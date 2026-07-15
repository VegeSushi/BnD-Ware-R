use mlua::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;
use sdl2::event::Event;
use sdl2::image::LoadTexture;
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

// Helper to convert "rom:/" paths to our local filesystem
fn resolve_path(vfs_root: &Path, path: &str) -> PathBuf {
    let mut clean_path = path.replace("rom:/", "assets/");
    if clean_path.ends_with(".sprite") {
        clean_path = clean_path.replace(".sprite", ".png");
    }
    vfs_root.join(clean_path)
}

// ---------------------------------------------------------
// Main Entry Point
// ---------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let _image_context = sdl2::image::init(sdl2::image::InitFlag::PNG)?;
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string())?;

    let window = video_subsystem
        .window("BnD-Ware Engine", 320, 240)
        .position_centered()
        .build()?;
    
    let mut canvas = window.into_canvas().build()?;
    let mut event_pump = sdl_context.event_pump()?;
    let texture_creator = canvas.texture_creator();

    // We assume fetch.bat created the "core" folder at the root.
    let vfs_root = Path::new("core");
    let font_path = resolve_path(&vfs_root, "rom:/font.ttf");
    let font = ttf_context.load_font(&font_path, 16).ok();

    // Loading Screens[cite: 1]
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

    // Logo Animation Loop[cite: 1]
    for i in 0..=59 {
        let filename = format!("rom:/logo/logo_{:04}.sprite", i);
        let path = resolve_path(&vfs_root, &filename);
        if let Ok(texture) = texture_creator.load_texture(&path) {
            canvas.clear();
            canvas.copy(&texture, None, None)?;
            canvas.present();
            std::thread::sleep(Duration::from_millis(16)); // ~60fps instead of 1ms to actually see it
        }
    }

    let mut g_total_score = 0; //[cite: 1]

    // Main Engine Loop
    'engine_loop: loop {
        // Main Menu Loop[cite: 1]
        let mut selected = 0;
        let mut menu_frame = 0;
        let mut menu_active = true;

        while menu_active {
            // Pump Events
            for event in event_pump.poll_iter() {
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
                                    show_about_screen(&mut canvas, &mut event_pump, &texture_creator, &font, &vfs_root)?;
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
            let bg_path = resolve_path(&vfs_root, &bg_filename);
            
            if let Ok(texture) = texture_creator.load_texture(&bg_path) {
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

        // Gather and shuffle minigames[cite: 1]
        let minigames_dir = vfs_root.join("minigames");
        let mut minigames = Vec::new();
        if let Ok(entries) = fs::read_dir(minigames_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("lua") {
                    minigames.push(entry.path());
                }
            }
        }
        let mut rng = thread_rng();
        minigames.shuffle(&mut rng);

        // Run Minigames[cite: 1]
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
                for event in event_pump.poll_iter() {
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
                            let disk_path = resolve_path(&vfs_root, &path);
                            if let Ok(texture) = texture_creator.load_texture(&disk_path) {
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

                // 4. Frame Capping (~60 FPS)[cite: 1]
                let elapsed = frame_start.elapsed();
                if elapsed < Duration::from_millis(16) {
                    std::thread::sleep(Duration::from_millis(16) - elapsed);
                }
            }

            // Intermediate Score Screen[cite: 1]
            if let Some(score) = game_score {
                g_total_score += score;
                
                let stars_path = resolve_path(&vfs_root, "rom:/sprites/stars.sprite");
                if let Ok(stars) = texture_creator.load_texture(&stars_path) {
                    canvas.copy(&stars, None, None)?;
                }
                
                let score_text = format!("Game Score: {}\nTotal Score: {}", score, g_total_score);
                draw_native_text(&mut canvas, &texture_creator, &font, &score_text, 20, 20)?;
                canvas.present();
                std::thread::sleep(Duration::from_millis(2500));
            }
        }

        // Final Score Screen[cite: 1]
        let stars_path = resolve_path(&vfs_root, "rom:/sprites/stars.sprite");
        if let Ok(stars) = texture_creator.load_texture(&stars_path) {
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
    font: &Option<sdl2::ttf::Font>,
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
    font: &Option<sdl2::ttf::Font>,
    vfs_root: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let stars_path = resolve_path(vfs_root, "rom:/sprites/stars.sprite");
    if let Ok(stars) = tc.load_texture(&stars_path) {
        canvas.copy(&stars, None, None)?;
    }

    let qr_path = resolve_path(vfs_root, "rom:/sprites/qr.sprite");
    if let Ok(qr) = tc.load_texture(&qr_path) {
        let q = qr.query();
        canvas.copy(&qr, None, Some(sdl2::rect::Rect::new(100, 45, q.width as u32, q.height as u32)))?;
    }

    draw_native_text(canvas, tc, font, "https://github.com/VegeSushi/BnD-Ware", 15, 20)?;
    canvas.present();

    loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::KeyDown { scancode: Some(key), .. } => {
                    if key == Scancode::Return || key == Scancode::Z || key == Scancode::X {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

fn draw_native_text(
    canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
    tc: &sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    font: &Option<sdl2::ttf::Font>,
    text: &str,
    x: i32,
    y: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(f) = font {
        for (i, line) in text.split('\n').enumerate() {
            if let Ok(surface) = f.render(line).blended(sdl2::pixels::Color::WHITE) {
                if let Ok(texture) = tc.create_texture_from_surface(&surface) {
                    let target = sdl2::rect::Rect::new(x, y + (i as i32 * 20), surface.width(), surface.height());
                    canvas.copy(&texture, None, Some(target))?;
                }
            }
        }
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

    // Inject our coroutine logic to suspend execution when a frame ends[cite: 1]
    lua.load(
        r#"
        function begin_frame() end
        function end_frame() coroutine.yield() end
        "#
    ).exec()?;

    Ok(())
}