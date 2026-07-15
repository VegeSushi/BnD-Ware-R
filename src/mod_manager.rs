// Hide the console window when running the release build on Windows,
// since this is a windowed egui app, not a console app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use directories::UserDirs;
use eframe::egui;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

struct ModManagerApp {
    mods_dir: PathBuf,
    config_path: PathBuf,
    available_mods: Vec<String>,
    enabled_mods: HashSet<String>,
    status: Option<String>,
}

impl ModManagerApp {
    fn new() -> Self {
        let user_dirs = UserDirs::new().expect("could not determine the user's home directory");
        let bnd_home = user_dirs.home_dir().join("BnD");
        let mods_dir = bnd_home.join("Mods");
        let config_dir = bnd_home.join("config");
        let config_path = config_dir.join("selected_mods.json");

        if !mods_dir.exists() {
            fs::create_dir_all(&mods_dir).unwrap();
        }
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).unwrap();
        }

        let mut available_mods = Vec::new();
        if let Ok(entries) = fs::read_dir(&mods_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("bnd") {
                    if let Ok(name) = entry.file_name().into_string() {
                        available_mods.push(name);
                    }
                }
            }
        }
        available_mods.sort();

        // selected_mods.json is a plain JSON array of enabled filenames,
        // e.g. ["cool_mod.bnd", "other_mod.bnd"] - this is also exactly
        // the format bnd_game.exe reads at startup when launched with
        // --modded, so save/load here must stay in lockstep with it.
        let enabled_mods: HashSet<String> = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .map(|v| v.into_iter().collect())
            .unwrap_or_default();

        Self {
            mods_dir,
            config_path,
            available_mods,
            enabled_mods,
            status: None,
        }
    }

    fn save_config(&mut self) {
        let list: Vec<&String> = self.enabled_mods.iter().collect();
        match serde_json::to_string_pretty(&list) {
            Ok(json) => {
                if let Err(e) = fs::write(&self.config_path, json) {
                    self.status = Some(format!("Failed to save config: {}", e));
                }
            }
            Err(e) => {
                self.status = Some(format!("Failed to serialize config: {}", e));
            }
        }
    }

    /// Saves the current selection, then launches bnd_game.exe (expected
    /// to sit right next to this exe, same as the installer lays them
    /// out) with --modded so it picks up the enabled mods.
    fn play_game(&mut self) {
        self.save_config();

        let exe_dir = match std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())) {
            Some(dir) => dir,
            None => {
                self.status = Some("Could not determine mod manager's own directory.".to_string());
                return;
            }
        };

        let game_exe = if cfg!(windows) {
            exe_dir.join("bnd_game.exe")
        } else {
            exe_dir.join("bnd_game")
        };

        if !game_exe.exists() {
            self.status = Some(format!("Game executable not found at {}", game_exe.display()));
            return;
        }

        match Command::new(&game_exe).arg("--modded").spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                self.status = Some(format!("Failed to launch game: {}", e));
            }
        }
    }
}

impl eframe::App for ModManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("BnD-Ware Mod Manager");
            ui.separator();

            if self.available_mods.is_empty() {
                ui.label("No mods found. Place .bnd files in:");
                ui.monospace(self.mods_dir.display().to_string());
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for mod_file in &self.available_mods {
                    let mut is_enabled = self.enabled_mods.contains(mod_file);
                    if ui.checkbox(&mut is_enabled, mod_file).changed() {
                        if is_enabled {
                            self.enabled_mods.insert(mod_file.clone());
                        } else {
                            self.enabled_mods.remove(mod_file);
                        }
                    }
                }
            });

            if let Some(status) = &self.status {
                ui.separator();
                ui.colored_label(egui::Color32::LIGHT_RED, status);
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Save & Exit").clicked() {
                        self.save_config();
                        std::process::exit(0);
                    }
                    if ui.button("Play Game").clicked() {
                        self.play_game();
                    }
                });
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(
        "BnD Mod Manager",
        options,
        Box::new(|_cc| Box::new(ModManagerApp::new())),
    )
}