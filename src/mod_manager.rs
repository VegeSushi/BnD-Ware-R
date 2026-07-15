use directories::UserDirs;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default)]
struct ModConfig {
    enabled_mods: HashSet<String>,
}

struct ModManagerApp {
    mod_dir: PathBuf,
    available_mods: Vec<String>,
    config: ModConfig,
}

impl ModManagerApp {
    fn new() -> Self {
        let user_dirs = UserDirs::new().unwrap();
        let mod_dir = user_dirs.home_dir().join("BnD").join("Mods");
        
        if !mod_dir.exists() {
            fs::create_dir_all(&mod_dir).unwrap();
        }

        let mut available_mods = Vec::new();
        if let Ok(entries) = fs::read_dir(&mod_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|e| e.to_str()) == Some("bnd") {
                    available_mods.push(entry.file_name().into_string().unwrap());
                }
            }
        }

        let config_path = mod_dir.join("mod_config.json");
        let config: ModConfig = fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();

        Self { mod_dir, available_mods, config }
    }

    fn save_config(&self) {
        let config_path = self.mod_dir.join("mod_config.json");
        if let Ok(json) = serde_json::to_string_pretty(&self.config.enabled_mods) {
            let _ = fs::write(config_path, json);
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
                ui.monospace(self.mod_dir.display().to_string());
            }

            let mut config_changed = false;

            egui::ScrollArea::vertical().show(ui, |ui| {
                for mod_file in &self.available_mods {
                    let mut is_enabled = self.config.enabled_mods.contains(mod_file);
                    
                    if ui.checkbox(&mut is_enabled, mod_file).changed() {
                        config_changed = true;
                        if is_enabled {
                            self.config.enabled_mods.insert(mod_file.clone());
                        } else {
                            self.config.enabled_mods.remove(mod_file);
                        }
                    }
                }
            });

            if config_changed {
                self.save_config();
            }

            ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                if ui.button("Save & Exit").clicked() {
                    self.save_config();
                    std::process::exit(0);
                }
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(400.0, 300.0)),
        ..Default::default()
    };
    eframe::run_native(
        "BnD Mod Manager",
        options,
        Box::new(|_cc| Box::new(ModManagerApp::new())),
    )
}