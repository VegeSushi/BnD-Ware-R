// Hide the console window when running the release build on Windows,
// since this is a windowed SDL2 game, not a console app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::UserDirs;
use mlua::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use sdl2::event::Event;
use sdl2::keyboard::Scancode;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------
// Engine State & Draw Queue
// ---------------------------------------------------------

enum DrawCmd {
    Clear(u8, u8, u8, u8),
    DrawSprite(String, i32, i32),
    DrawText(String, i32, i32),
}

struct EngineState {
    draw_queue: Vec<DrawCmd>,
    keyboard_state: HashSet<Scancode>,
    start_time: Instant,
}

// ---------------------------------------------------------
// Virtual filesystem: a small ordered list of "roots".
//
// vfs_roots[0] is highest priority. When running unmodded this is just
// [core_root]. When running with --modded (only ever passed by the mod
// manager), enabled mods are extracted and prepended in front of core, so
// a mod's files shadow core's files of the same relative path, and mods
// can also add brand new minigames/sprites alongside core's.
// ---------------------------------------------------------

/// Resolve a "rom:/" path against the layered vfs, returning the first
/// root that actually has the file, or falling back to the last
/// (lowest-priority / core) root if nothing has it, so callers can keep
/// their existing "attempt to load, ignore on failure" behavior.
fn resolve_path(vfs_roots: &[PathBuf], path: &str) -> PathBuf {
    let mut clean_path = path.replace("rom:/", "assets/");
    if clean_path.ends_with(".sprite") {
        clean_path = clean_path.replace(".sprite", ".png");
    }

    for root in vfs_roots {
        let candidate = root.join(&clean_path);
        if candidate.exists() {
            return candidate;
        }
    }

    // Nothing found; fall back to the lowest-priority root so the
    // subsequent load attempt fails the same way it always has.
    vfs_roots
        .last()
        .map(|root| root.join(&clean_path))
        .unwrap_or_else(|| PathBuf::from(clean_path))
}

/// Collects every minigame lua script visible across all vfs roots.
/// If two roots have a minigame with the same filename, the
/// higher-priority root (earlier in vfs_roots, i.e. a mod) wins.
fn gather_minigames(vfs_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for root in vfs_roots {
        let dir = root.join("minigames");
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("lua") {
                    continue;
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if seen.insert(name.to_string()) {
                        result.push(path);
                    }
                }
            }
        }
    }

    result
}

/// Extracts a zip archive (a .bnd file) to `dest`.
fn extract_zip(zip_path: &Path, dest: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    fs::create_dir_all(dest)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let outpath = match entry.enclosed_name() {
            Some(p) => dest.join(p),
            None => continue, // skip unsafe paths (e.g. containing "..")
        };

        if entry.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    Ok(())
}

/// Builds the list of vfs roots for this run: core, extracted once from
/// core.bnd the first time it's needed, plus any enabled mods layered on
/// top when launched with --modded.
fn build_vfs_roots(modded: bool) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));

    // Prefer an already-extracted "core" folder next to the exe (dev
    // workflow via fetch.bat, or a previous run's extraction). Otherwise
    // extract core.bnd into place the first time it's needed.
    let core_root = exe_dir.join("core");
    if !core_root.exists() {
        let core_bnd = exe_dir.join("core.bnd");
        if core_bnd.exists() {
            if let Err(e) = extract_zip(&core_bnd, &core_root) {
                eprintln!("Failed to extract core.bnd: {}", e);
            }
        }
    }
    // Dev fallback: also check a "core" folder relative to the current
    // working directory (e.g. `cargo run` straight after fetch.bat).
    let core_root = if core_root.exists() {
        core_root
    } else {
        PathBuf::from("core")
    };

    let mut vfs_roots: Vec<PathBuf> = Vec::new();

    if modded {
        if let Some(user_dirs) = UserDirs::new() {
            let bnd_home = user_dirs.home_dir().join("BnD");
            let mods_dir = bnd_home.join("Mods");
            let config_path = bnd_home.join("config").join("selected_mods.json");

            let enabled: Vec<String> = fs::read_to_string(&config_path)
                .ok()
                .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
                .unwrap_or_default();

            let extract_base = std::env::temp_dir().join("bnd_ware_mods");

            for mod_name in &enabled {
                let mod_bnd = mods_dir.join(mod_name);
                if !mod_bnd.exists() {
                    eprintln!("Enabled mod not found, skipping: {}", mod_name);
                    continue;
                }

                let target = extract_base.join(mod_name);
                // Re-extract every launch so edits to the .bnd are picked up.
                let _ = fs::remove_dir_all(&target);

                match extract_zip(&mod_bnd, &target) {
                    Ok(()) => vfs_roots.push(target),
                    Err(e) => eprintln!("Failed to load mod {}: {}", mod_name, e),
                }
            }
        } else {
            eprintln!("Could not determine home directory; skipping mods.");
        }
    }

    vfs_roots.push(core_root);
    vfs_roots
}

// ---------------------------------------------------------
// Main Entry Point
// ---------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The mod manager launches us with `--modded` after the user picks
    // "Play Game". Running the exe directly (double-click, shortcut)
    // never sets this, so mods are only ever loaded via the mod manager.
    let modded = std::env::args().any(|a| a == "--modded");
    let vfs_roots = build_vfs_roots(modded);

    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("BnD-Ware Engine", 320, 240)
        .position_centered()
        .build()?;

    let mut canvas = window.into_canvas().build()?;
    let mut event_pump = sdl_context.event_pump()?;
    let texture_creator = canvas.texture_creator();

    let font_path = resolve_path(&vfs_roots, "rom:/font.ttf");
    let font: Option<fontdue::Font> = std::fs::read(&font_path)
        .ok()
        .and_then(|bytes| fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).ok());

    // Loading Screens
    draw_loading_screen(&mut canvas, &texture_creator, &font, 1)?;
    std::thread::sleep(Duration::from_millis(300));
    draw_loading_screen(&mut canvas, &texture_creator, &font, 2)?;
    std::thread::sleep(Duration::from_millis(300));

    let lua = Lua::new();
    let state = Rc::new(RefCell::new(EngineState {
        draw_queue: Vec::new(),
        keyboard_state: HashSet::new(),
        start_time: Instant::now(),
    }));

    register_lua_hooks(&lua, state.clone())?;

    draw_loading_screen(&mut canvas, &texture_creator, &font, 3)?;
    std::thread::sleep(Duration::from_millis(500));

    // Logo Animation Loop
    for i in 0..=59 {
        let filename = format!("rom:/logo/logo_{:04}.sprite", i);
        let path = resolve_path(&vfs_roots, &filename);
        if let Ok(texture) = load_texture_from_file(&texture_creator, &path) {
            canvas.clear();
            canvas.copy(&texture, None, None)?;
            canvas.present();
            std::thread::sleep(Duration::from_millis(16)); // ~60fps instead of 1ms to actually see it
        }
    }

    let mut g_total_score = 0;

    // Main Engine Loop
    'engine_loop: loop {
        // Main Menu Loop
        let mut selected = 0;
        let mut menu_frame = 0;
        let mut menu_active = true;

        while menu_active {
            // Pump Events. Collected into a Vec first so the mutable
            // borrow of event_pump from poll_iter() ends before the loop
            // body runs — the body may need its own &mut event_pump (to
            // pass into show_about_screen), which would otherwise conflict
            // with poll_iter()'s still-live borrow.
            let events: Vec<Event> = event_pump.poll_iter().collect();
            for event in events {
                match event {
                    Event::Quit { .. } => break 'engine_loop,
                    Event::KeyDown { scancode: Some(key), .. } => {
                        match key {
                            Scancode::Down => {
                                selected += 1;
                                if selected > 1 { selected = 0; }
                            }
                            Scancode::Up => {
                                selected -= 1;
                                if selected < 0 { selected = 1; }
                            }
                            Scancode::Return | Scancode::Z | Scancode::X => {
                                if selected == 0 {
                                    g_total_score = 0;
                                    menu_active = false;
                                } else if selected == 1 {
                                    show_about_screen(&mut canvas, &mut event_pump, &texture_creator, &font, &vfs_roots)?;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }

            if !menu_active { break; }

            // Draw Menu Frame
            let bg_filename = format!("rom:/menu/menu_{:04}.sprite", menu_frame);
            let bg_path = resolve_path(&vfs_roots, &bg_filename);

            if let Ok(texture) = load_texture_from_file(&texture_creator, &bg_path) {
                canvas.copy(&texture, None, None)?;
            }

            draw_native_text(&mut canvas, &texture_creator, &font, "Bubbles And Doki Ware", 20, 20)?;

            if selected == 0 {
                draw_native_text(&mut canvas, &texture_creator, &font, "> Start", 20, 60)?;
                draw_native_text(&mut canvas, &texture_creator, &font, "  About", 20, 80)?;
            } else {
                draw_native_text(&mut canvas, &texture_creator, &font, "  Start", 20, 60)?;
                draw_native_text(&mut canvas, &texture_creator, &font, "> About", 20, 80)?;
            }

            canvas.present();
            menu_frame += 1;
            if menu_frame > 173 { menu_frame = 0; }
            std::thread::sleep(Duration::from_millis(16));
        }

        // Gather and shuffle minigames from every vfs root (core + mods)
        let mut minigames = gather_minigames(&vfs_roots);
        let mut rng = thread_rng();
        minigames.shuffle(&mut rng);

        // Run Minigames
        for minigame_path in minigames {
            let script = fs::read_to_string(&minigame_path)?;

            // Execute the Lua Minigame via Coroutine Resumption
            let chunk = lua.load(&script).into_function()?;
            let thread = lua.create_thread(chunk)?;

            let mut game_score: Option<i32> = None;

            'minigame_loop: loop {
                let frame_start = Instant::now();

                // 1. Update Input State
                let mut keys = HashSet::new();
                let events: Vec<Event> = event_pump.poll_iter().collect();
                for event in events {
                    match event {
                        Event::Quit { .. } => break 'engine_loop,
                        Event::KeyDown { scancode: Some(scan), .. } => { keys.insert(scan); },
                        _ => {}
                    }
                }

                // Add currently pressed keys to state
                let keyboard_state = event_pump.keyboard_state();
                for key in keyboard_state.pressed_scancodes() {
                    keys.insert(key);
                }
                state.borrow_mut().keyboard_state = keys;

                // 2. Resume Lua Coroutine
                // The lua script runs until it hits end_frame() (which we redefined to coroutine.yield())
                // OR until it returns a final score.
                match thread.resume::<_, Option<i32>>(()) {
                    Ok(Some(score)) => {
                        game_score = Some(score);
                        break 'minigame_loop;
                    }
                    Ok(None) => {
                        // Thread yielded (end_frame was called). Proceed to render.
                    }
                    Err(e) => {
                        println!("LUA ERROR: {}", e);
                        canvas.set_draw_color(sdl2::pixels::Color::BLACK);
                        canvas.clear();
                        draw_native_text(&mut canvas, &texture_creator, &font, "LUA ERROR", 10, 10)?;
                        canvas.present();
                        std::thread::sleep(Duration::from_millis(5000));
                        break 'minigame_loop;
                    }
                }

                // 3. Render Draw Queue populated by Lua
                let cmds: Vec<DrawCmd> = state.borrow_mut().draw_queue.drain(..).collect();
                for cmd in cmds {
                    match cmd {
                        DrawCmd::Clear(r, g, b, a) => {
                            canvas.set_draw_color(sdl2::pixels::Color::RGBA(r, g, b, a));
                            canvas.clear();
                        }
                        DrawCmd::DrawSprite(path, x, y) => {
                            let disk_path = resolve_path(&vfs_roots, &path);
                            if let Ok(texture) = load_texture_from_file(&texture_creator, &disk_path) {
                                let q = texture.query();
                                let target = sdl2::rect::Rect::new(x, y, q.width as u32, q.height as u32);
                                let _ = canvas.copy(&texture, None, Some(target));
                            }
                        }
                        DrawCmd::DrawText(text, x, y) => {
                            let _ = draw_native_text(&mut canvas, &texture_creator, &font, &text, x, y);
                        }
                    }
                }
                canvas.present();

                // 4. Frame Capping (~60 FPS)
                let elapsed = frame_start.elapsed();
                if elapsed < Duration::from_millis(16) {
                    std::thread::sleep(Duration::from_millis(16) - elapsed);
                }
            }

            // Intermediate Score Screen
            if let Some(score) = game_score {
                g_total_score += score;

                let stars_path = resolve_path(&vfs_roots, "rom:/sprites/stars.sprite");
                if let Ok(stars) = load_texture_from_file(&texture_creator, &stars_path) {
                    canvas.copy(&stars, None, None)?;
                }

                let score_text = format!("Game Score: {}\nTotal Score: {}", score, g_total_score);
                draw_native_text(&mut canvas, &texture_creator, &font, &score_text, 20, 20)?;
                canvas.present();
                std::thread::sleep(Duration::from_millis(2500));
            }
        }

        // Final Score Screen
        let stars_path = resolve_path(&vfs_roots, "rom:/sprites/stars.sprite");
        if let Ok(stars) = load_texture_from_file(&texture_creator, &stars_path) {
            canvas.copy(&stars, None, None)?;
        }
        let final_text = format!("Final Score: {}", g_total_score);
        draw_native_text(&mut canvas, &texture_creator, &font, &final_text, 20, 20)?;
        canvas.present();
        std::thread::sleep(Duration::from_millis(5000));
    }

    Ok(())
}

// ---------------------------------------------------------
// Helper Functions for Engine UI
// ---------------------------------------------------------

fn draw_loading_screen(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    tc: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    font: &Option<fontdue::Font>,
    progress: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    canvas.set_draw_color(sdl2::pixels::Color::BLACK);
    canvas.clear();

    draw_native_text(canvas, tc, font, "BNDWARE ENGINE INIT...", 20, 20)?;
    if progress >= 1 { draw_native_text(canvas, tc, font, "> Filesystem OK", 20, 40)?; }
    if progress >= 2 { draw_native_text(canvas, tc, font, "> Joypad OK", 20, 50)?; }
    if progress >= 3 { draw_native_text(canvas, tc, font, "> Lua VM OK", 20, 60)?; }

    canvas.present();
    Ok(())
}

fn show_about_screen(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    event_pump: &mut sdl2::EventPump,
    tc: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    font: &Option<fontdue::Font>,
    vfs_roots: &[PathBuf],
) -> Result<(), Box<dyn std::error::Error>> {
    let stars_path = resolve_path(vfs_roots, "rom:/sprites/stars.sprite");
    if let Ok(stars) = load_texture_from_file(tc, &stars_path) {
        canvas.copy(&stars, None, None)?;
    }

    let qr_path = resolve_path(vfs_roots, "rom:/sprites/qr.sprite");
    if let Ok(qr) = load_texture_from_file(tc, &qr_path) {
        let q = qr.query();
        canvas.copy(&qr, None, Some(sdl2::rect::Rect::new(100, 45, q.width as u32, q.height as u32)))?;
    }

    draw_native_text(canvas, tc, font, "https://github.com/VegeSushi/BnD-Ware", 15, 20)?;
    canvas.present();

    loop {
        let events: Vec<Event> = event_pump.poll_iter().collect();
        for event in events {
            if let Event::KeyDown { scancode: Some(key), .. } = event {
                if key == Scancode::Return || key == Scancode::Z || key == Scancode::X {
                    return Ok(());
                }
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// Loads an image file from disk and decodes it in pure Rust (via the
/// `image` crate) into an SDL2 texture, replacing sdl2::image::LoadTexture
/// / SDL2_image so the whole build stays free of extra native libs.
fn load_texture_from_file<'a>(
    tc: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    path: &Path,
) -> Result<sdl2::render::Texture<'a>, String> {
    let img = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("image has zero width/height".to_string());
    }

    let mut texture = tc
        .create_texture_static(sdl2::pixels::PixelFormatEnum::RGBA32, w, h)
        .map_err(|e| e.to_string())?;
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    texture
        .update(None, &img.into_raw(), (w * 4) as usize)
        .map_err(|e| e.to_string())?;
    Ok(texture)
}

/// Renders text into a texture using pure-Rust glyph rasterization (via
/// `fontdue`), replacing sdl2::ttf::Font::render(...).blended(...) /
/// SDL2_ttf so the whole build stays free of extra native libs.
fn draw_native_text(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    tc: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    font: &Option<fontdue::Font>,
    text: &str,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    const FONT_SIZE: f32 = 16.0;
    const LINE_HEIGHT: i32 = 20;

    let Some(f) = font else { return Ok(()) };

    for (i, line) in text.split('\n').enumerate() {
        if line.is_empty() {
            continue;
        }

        // First pass: rasterize every glyph and work out the line's
        // overall pixel width/height, plus where the baseline sits.
        let mut glyphs = Vec::with_capacity(line.len());
        let mut pen_x: i32 = 0;
        let mut max_ascent: i32 = 0; // highest point any glyph reaches above the baseline
        let mut min_descent: i32 = 0; // lowest point any glyph reaches below the baseline

        for ch in line.chars() {
            let (metrics, bitmap) = f.rasterize(ch, FONT_SIZE);
            max_ascent = max_ascent.max(metrics.ymin + metrics.height as i32);
            min_descent = min_descent.min(metrics.ymin);
            glyphs.push((pen_x, metrics, bitmap));
            pen_x += metrics.advance_width.round() as i32;
        }

        let width = pen_x.max(1) as u32;
        let height = (max_ascent - min_descent).max(1) as u32;
        let baseline = max_ascent;

        // Second pass: blit each glyph's coverage bitmap into an RGBA
        // buffer as white text with per-pixel alpha.
        let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
        for (gx, metrics, bitmap) in &glyphs {
            for gy in 0..metrics.height {
                for gxi in 0..metrics.width {
                    let coverage = bitmap[gy * metrics.width + gxi];
                    if coverage == 0 {
                        continue;
                    }
                    let px = gx + gxi as i32 + metrics.xmin;
                    let py = baseline - metrics.height as i32 + gy as i32 - metrics.ymin;
                    if px < 0 || py < 0 || px as u32 >= width || py as u32 >= height {
                        continue;
                    }
                    let idx = ((py as u32 * width + px as u32) * 4) as usize;
                    buffer[idx] = 255;
                    buffer[idx + 1] = 255;
                    buffer[idx + 2] = 255;
                    buffer[idx + 3] = coverage;
                }
            }
        }

        let mut texture = tc
            .create_texture_static(sdl2::pixels::PixelFormatEnum::RGBA32, width, height)
            .map_err(|e| e.to_string())?;
        texture.set_blend_mode(sdl2::render::BlendMode::Blend);
        texture
            .update(None, &buffer, (width * 4) as usize)
            .map_err(|e| e.to_string())?;

        let target = sdl2::rect::Rect::new(x, y + (i as i32 * LINE_HEIGHT), width, height);
        canvas.copy(&texture, None, Some(target))?;
    }

    Ok(())
}

// ---------------------------------------------------------
// Lua Bindings Configuration
// ---------------------------------------------------------

fn register_lua_hooks(lua: &Lua, state: Rc<RefCell<EngineState>>) -> mlua::Result<()> {
    let globals = lua.globals();

    let state_ref = state.clone();
    globals.set("clear_screen", lua.create_function(move |_, (r, g, b, a): (u8, u8, u8, Option<u8>)| {
        state_ref.borrow_mut().draw_queue.push(DrawCmd::Clear(r, g, b, a.unwrap_or(255)));
        Ok(())
    })?)?;

    let state_ref = state.clone();
    globals.set("draw_sprite", lua.create_function(move |_, (path, x, y): (String, i32, i32)| {
        state_ref.borrow_mut().draw_queue.push(DrawCmd::DrawSprite(path, x, y));
        Ok(())
    })?)?;

    let state_ref = state.clone();
    globals.set("draw_text", lua.create_function(move |_, (text, x, y): (String, i32, i32)| {
        state_ref.borrow_mut().draw_queue.push(DrawCmd::DrawText(text, x, y));
        Ok(())
    })?)?;

    let state_ref = state.clone();
    globals.set("get_time_ms", lua.create_function(move |_, ()| {
        let elapsed = state_ref.borrow().start_time.elapsed().as_millis() as u32;
        Ok(elapsed)
    })?)?;

    let state_ref = state.clone();
    globals.set("get_button", lua.create_function(move |_, btn: String| {
        let keys = &state_ref.borrow().keyboard_state;
        let pressed = match btn.as_str() {
            "UP" => keys.contains(&Scancode::Up),
            "DOWN" => keys.contains(&Scancode::Down),
            "LEFT" => keys.contains(&Scancode::Left),
            "RIGHT" => keys.contains(&Scancode::Right),
            "A" => keys.contains(&Scancode::Z),
            "B" => keys.contains(&Scancode::X),
            "START" => keys.contains(&Scancode::Return),
            _ => false,
        };
        Ok(pressed)
    })?)?;

    // Inject our coroutine logic to suspend execution when a frame ends
    lua.load(
        r#"
        function begin_frame() end
        function end_frame() coroutine.yield() end
        "#
    ).exec()?;

    Ok(())
}