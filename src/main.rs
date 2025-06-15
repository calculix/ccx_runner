#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release
#![allow(rustdoc::missing_crate_level_docs)] // it's an example

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            Ok(Box::<MainApp>::default())
        }),
    )
}

struct UserSetup {
    calculix_bin_path: PathBuf,
    project_dir_path: PathBuf,
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
            user_setup: UserSetup {
                calculix_bin_path: PathBuf::from(
                    "/media/qhuss/76a9dfaf-c78f-4c2f-a48c-5a6b936cdb8d/CalculiX/ccx_2.19_MT",
                ),
                project_dir_path: PathBuf::from(
                    "/media/qhuss/76a9dfaf-c78f-4c2f-a48c-5a6b936cdb8d/PrePoMax/PrePoMax v2.3.4 dev/Temp/",
                ),
            },
            solver_output: String::from(
                "TEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST\nTEST",
            ),
            ansicht: Ansicht::SolverOutput,
        }
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
