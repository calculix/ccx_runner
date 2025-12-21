#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use dirs::config_dir;
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, create_dir_all, File},
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    thread,
};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CalculiX Solution Monitor",
        options,
        Box::new(|cc| Box::new(MainApp::new(cc))),
    )
}

fn default_num_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[derive(Serialize, Deserialize, Debug)]
struct UserSetup {
    calculix_bin_path: PathBuf,
    project_dir_path: PathBuf,
    #[serde(default = "default_num_cores")]
    num_cores: usize,
}

impl Default for UserSetup {
    fn default() -> Self {
        Self {
            calculix_bin_path: PathBuf::from(""),
            project_dir_path: PathBuf::from(""),
            num_cores: default_num_cores(),
        }
    }
}

#[derive(PartialEq)]
enum Ansicht {
    SolverOutput,
    Overview,
}

#[derive(Debug, Clone, Default)]
struct StepInfo {
    step: u32,
    increment: u32,
    attempt: u32,
    iterations: u32,
    step_time: f64,
    total_time: f64,
}

#[derive(Debug, Clone)]
struct ResidualData {
    step: u32,
    total_iteration: u32,
    residual: f64,
}

enum SolverMessage {
    Line(String),
    NewStepInfo(StepInfo),
    UpdateStepInfo(StepInfo),
    Residual(ResidualData),
}

struct MainApp {
    user_setup: UserSetup,
    ansicht: Ansicht,
    solver_process: Option<Arc<Mutex<Child>>>,
    line_receiver: Option<Receiver<SolverMessage>>,
    is_running: bool,
    solver_output_buffer: String,
    residual_data: Vec<ResidualData>,
    step_info: Vec<StepInfo>,
    available_inp_files: Vec<PathBuf>,
    selected_inp_file: Option<PathBuf>,
}

impl MainApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This gives us image support:
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut app = Self {
            user_setup: Self::load_config(),
            ansicht: Ansicht::SolverOutput,
            solver_process: None,
            line_receiver: None,
            is_running: false,
            solver_output_buffer: String::new(),
            residual_data: Vec::new(),
            step_info: Vec::new(),
            available_inp_files: Vec::new(),
            selected_inp_file: None,
        };
        app.refresh_inp_files();
        app
    }

    fn refresh_inp_files(&mut self) {
        self.available_inp_files.clear();
        if let Ok(entries) = fs::read_dir(&self.user_setup.project_dir_path) {
            self.available_inp_files = entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().extension().and_then(|s| s.to_str()) == Some("inp")
                })
                .map(|entry| entry.path())
                .collect();
        }
        // If the selected file is no longer available, reset it.
        if let Some(selected) = &self.selected_inp_file {
            if !self.available_inp_files.contains(selected) {
                self.selected_inp_file = None;
            }
        }
        // If nothing is selected, and there are files, select the first one.
        if self.selected_inp_file.is_none() && !self.available_inp_files.is_empty() {
            self.selected_inp_file = self.available_inp_files.first().cloned();
        }
    }

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
        // Handle solver output and check for completion
        if let Some(receiver) = &self.line_receiver {
            // Use a loop to drain the channel on each frame.
            loop {
                match receiver.try_recv() {
                    Ok(message) => match message {
                        SolverMessage::Line(line) => {
                            self.solver_output_buffer.push_str(&line);
                            self.solver_output_buffer.push('\n');
                        }
                        SolverMessage::Residual(data) => self.residual_data.push(data),
                        SolverMessage::NewStepInfo(info) => self.step_info.push(info),
                        SolverMessage::UpdateStepInfo(info) => {
                            if let Some(last) = self.step_info.last_mut() {
                                *last = info;
                            }
                        }
                    },
                    Err(mpsc::TryRecvError::Empty) => {
                        // No more messages in the channel for now.
                        break;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // The sender has been dropped, meaning the reader thread and process are finished.
                        self.is_running = false;
                        self.line_receiver = None;
                        self.solver_process = None; // The Child process is dropped here, reaping it.
                        self.solver_output_buffer.push_str("\n--- Analysis Finished ---\n");
                        break;
                    }
                }
            }
            ctx.request_repaint(); // Request a repaint to show new data
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Settings");
            {
                ui.label("Path to Calculix Binary");
                let mut ccx_path_str = self.user_setup.calculix_bin_path.display().to_string();
                if ui.text_edit_singleline(&mut ccx_path_str).changed() {
                    self.user_setup.calculix_bin_path = PathBuf::from(ccx_path_str);
                }
            }
            {
                ui.label("Path to project directory");
                let mut project_dir_str = self.user_setup.project_dir_path.display().to_string();
                if ui.text_edit_singleline(&mut project_dir_str).changed() {
                    self.user_setup.project_dir_path = PathBuf::from(project_dir_str);
                    self.refresh_inp_files();
                }
            }

            if !self.is_running {
                ui.horizontal(|ui| {
                    let max_cores = default_num_cores();
                    ui.label("Number of Cores:");
                    ui.add(
                        egui::DragValue::new(&mut self.user_setup.num_cores)
                            .clamp_range(1..=max_cores),
                    );
                });
            }

            // Drop-down for .inp file
            if !self.is_running {
                let selected_file_name = self
                    .selected_inp_file
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Select a file".to_string());

                egui::ComboBox::from_label("Input file")
                    .selected_text(selected_file_name)
                    .show_ui(ui, |ui| {
                        for f in &self.available_inp_files {
                            let file_name = f.file_name().unwrap().to_str().unwrap().to_string();
                            ui.selectable_value(&mut self.selected_inp_file, Some(f.clone()), file_name);
                        }
                    });
            }

            if self.is_running {
                if ui.button("Stop Analysis").clicked() {
                    if let Some(process) = self.solver_process.take() {
                        let mut process = process.lock().unwrap();
                        match process.kill() {
                            Ok(_) => {
                                println!("Process killed");
                            }
                            Err(e) => println!("Failed to kill process: {}", e),
                        }
                    }
                    self.is_running = false;
                    self.line_receiver = None;
                }
            } else {
                if ui.button("Run Analysis").clicked() {
                    match self.save_config() {
                        Ok(_) => {},
                        Err(e) => panic!("{}", e),
                    }

                    if let Some(inp_path) = self.selected_inp_file.clone() {
                        let job_name = inp_path.file_stem().unwrap().to_str().unwrap();
                        let (sender, receiver) = mpsc::channel::<SolverMessage>();
                        self.line_receiver = Some(receiver);
                        self.is_running = true;
                        self.solver_output_buffer.clear();
                        self.residual_data.clear();
                        self.step_info.clear();

                        let ccx_path = self.user_setup.calculix_bin_path.clone();
                        let project_dir = self.user_setup.project_dir_path.clone();
                        let job_name = job_name.to_string();
                        let num_cores = self.user_setup.num_cores.to_string();

                        let child = Command::new(ccx_path)
                            .arg("-i")
                            .arg(&job_name)
                            .env("OMP_NUM_THREADS", &num_cores)
                            .env("CCX_NPROC", &num_cores)
                            .current_dir(project_dir)
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .spawn();

                        match child {
                            Ok(mut child) => {
                                let stdout = child.stdout.take().unwrap();
                                let reader = BufReader::new(stdout);
                                let sender_clone = sender.clone();

                                thread::spawn(move || {
                                   let mut current_step_info: Option<StepInfo> = None;
                                   let mut total_iterations_for_residual = 0;

                                   for line_result in reader.lines() {
                                       match line_result {
                                           Ok(line) => {
                                               if line.starts_with(" STEP") {
                                                   if let Some(step_str) = line.split_whitespace().last() {
                                                       if let Ok(step_num) = step_str.parse::<u32>() {
                                                            let new_info = StepInfo {
                                                                step: step_num,
                                                                ..Default::default()
                                                            };
                                                            current_step_info = Some(new_info.clone());
                                                            if sender_clone.send(SolverMessage::NewStepInfo(new_info)).is_err() { break; }
                                                       }
                                                   }
                                               } else if let Some(info) = current_step_info.as_mut() {
                                                    let mut updated = false;
                                                    if line.starts_with(" increment ") {
                                                        let parts: Vec<&str> = line.split_whitespace().collect();
                                                        if parts.len() >= 4 {
                                                            if let (Ok(inc), Ok(att)) = (parts[1].parse::<u32>(), parts[3].parse::<u32>()) {
                                                                info.increment = inc;
                                                                info.attempt = att;
                                                                info.iterations = 0; // Reset for new attempt
                                                                updated = true;
                                                           }
                                                       }
                                                    } else if line.trim().starts_with("iteration ") {
                                                        info.iterations += 1;
                                                        updated = true;
                                                    } else if line.starts_with(" actual step time=") {
                                                        if let Some(val_str) = line.split('=').nth(1) {
                                                            if let Ok(val) = val_str.trim().parse::<f64>() {
                                                                info.step_time = val;
                                                                updated = true;
                                                           }
                                                       }
                                                    } else if line.starts_with(" actual total time=") {
                                                        if let Some(val_str) = line.split('=').nth(1) {
                                                            if let Ok(val) = val_str.trim().parse::<f64>() {
                                                                info.total_time = val;
                                                                updated = true;
                                                           }
                                                       }
                                                    } else if line.trim().starts_with("largest residual force=") {
                                                        if let Some(val_str) = line.split('=').nth(1) {
                                                            if let Some(residual_str) = val_str.trim().split_whitespace().next() {
                                                                if let Ok(residual) = residual_str.parse::<f64>() {
                                                                    total_iterations_for_residual += 1;
                                                                    let residual_data = ResidualData {
                                                                        step: info.step,
                                                                        total_iteration: total_iterations_for_residual,
                                                                        residual,
                                                                    };
                                                                    if sender_clone.send(SolverMessage::Residual(residual_data)).is_err() { break; }
                                                                }
                                                           }
                                                       }
                                                   }

                                                    if updated {
                                                        if sender_clone.send(SolverMessage::UpdateStepInfo(info.clone())).is_err() { break; }
                                                   }
                                               }

                                               if sender_clone.send(SolverMessage::Line(line)).is_err() {
                                                   break; // Receiver has been dropped
                                               }
                                           }
                                           Err(e) => {
                                               eprintln!("Error reading line: {}", e);
                                               break;
                                           }
                                       }
                                   }
                               });
                                self.solver_process = Some(Arc::new(Mutex::new(child)));
                            }
                            Err(e) => {
                                self.solver_output_buffer =
                                    format!("Failed to start process: {}", e);
                                self.is_running = false;
                            }
                        }
                    } else {
                        self.solver_output_buffer = "No '.inp' file selected.".to_string();
                    }
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
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.label(&self.solver_output_buffer);
                        });
                }

                Ansicht::Overview => {
                    // Step Table
                    ui.heading("Step Information");
                    egui::Grid::new("step_grid").striped(true).show(ui, |ui| {
                        ui.label("Step");
                        ui.label("Increment");
                        ui.label("Attempt");
                        ui.label("Iterations");
                        ui.label("Step Time");
                        ui.label("Total Time");
                        ui.end_row();

                        for data in &self.step_info {
                            ui.label(data.step.to_string());
                            ui.label(data.increment.to_string());
                            ui.label(data.attempt.to_string());
                            ui.label(data.iterations.to_string());
                            ui.label(format!("{:.4e}", data.step_time));
                            ui.label(format!("{:.4e}", data.total_time));
                            ui.end_row();
                        }
                    });
                }
            }
        });
    }
}
