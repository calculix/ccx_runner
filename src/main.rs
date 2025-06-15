#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use dirs::config_dir;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use std::path::PathBuf;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CalculiX Solution Monitor",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<MainApp>::default())
        }),
    )
}

#[derive(Serialize, Deserialize, Debug)]
struct UserSetup {
    calculix_bin_path: PathBuf,
    project_dir_path: PathBuf,
}

impl Default for UserSetup {
    fn default() -> Self {
        Self {
            calculix_bin_path: PathBuf::from(""),
            project_dir_path: PathBuf::from(""),
        }
    }
}

#[derive(PartialEq)]
enum Ansicht {
    SolverOutput,
    Overview,
}

struct MainApp {
    user_setup: UserSetup,
    solver_output: String,
    ansicht: Ansicht,
}

impl Default for MainApp {
    fn default() -> Self {
        Self {
            user_setup: Self::load_config(),
            solver_output: String::from(
                "TEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST",
            ),
            ansicht: Ansicht::SolverOutput,
        }
    }
}

impl MainApp {
    fn load_config() -> UserSetup {
        let config_dir = config_dir().unwrap().join("ccx_runner_rs");

        if !config_dir.exists() {
            create_dir_all(&config_dir).unwrap();
        };

        let config_file = config_dir.join("config.json");

        if config_file.exists() {
            let mut file = File::open(config_file).unwrap();
            let mut contents = String::new();
            file.read_to_string(&mut contents).unwrap();
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            UserSetup::default()
        }
    }

    fn save_config(&self) -> Result<(), std::io::Error> {
        let config_dir = config_dir().unwrap().join("ccx_runner_rs");
        let config_file = config_dir.join("config.json");
        let json = serde_json::to_string_pretty(&self.user_setup).unwrap();
        let mut file = File::create(config_file)?;
        file.write_all(json.as_bytes())?;

        Ok(())
    }
}

impl eframe::App for MainApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Settings");
            {
                ui.label("Path to Calculix Binary");
                let mut ccx_path_str = self.user_setup.calculix_bin_path.display().to_string();
                let ccx_path_input = ui.text_edit_singleline(&mut ccx_path_str);
                if ccx_path_input.changed() {
                    self.user_setup.calculix_bin_path = PathBuf::from(ccx_path_str);
                }
            }
            {
                ui.label("Path to project directory");
                let mut project_dir_str = self.user_setup.project_dir_path.display().to_string();
                let project_dir_input = ui.text_edit_singleline(&mut project_dir_str);
                if project_dir_input.changed() {
                    self.user_setup.project_dir_path = PathBuf::from(project_dir_str);
                }
            }

            if ui.button("Run Analysis").clicked() {
                match self.save_config() {
                    Ok(_) => println!("config saved!"),
                    Err(e) => println!("{}", e),
                }
            };

            // Tabs
            ui.add_space(10.);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ansicht, Ansicht::SolverOutput, "Solver Output");
                ui.selectable_value(&mut self.ansicht, Ansicht::Overview, "Overview");
            });
            ui.separator();

            match self.ansicht {
                Ansicht::SolverOutput => {
                    ui.heading("Solver Output");
                }

                Ansicht::Overview => {
                    ui.heading("Solution Overview");
                }
            }
        });
    }
}
