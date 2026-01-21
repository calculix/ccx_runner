use crate::config::{self, default_num_cores, UserSetup};
use crate::solver::{ResidualData, SolverMessage, StepInfo};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::{
    fs,
    path::PathBuf,
    process::Child,
    sync::{
        mpsc::{self, Receiver},
        Arc, Mutex,
    },
    time::Instant,
};

#[derive(PartialEq)]
pub enum Ansicht {
    SolverOutput,
    Overview,
}

pub struct MainApp {
    user_setup: UserSetup,
    ansicht: Ansicht,
    solver_process: Option<Arc<Mutex<Child>>>,
    line_receiver: Option<Receiver<SolverMessage>>,
    is_running: bool,
    solver_output_buffer: Vec<String>,
    residual_data: Vec<ResidualData>,
    step_info: Vec<StepInfo>,
    available_inp_files: Vec<PathBuf>,
    selected_inp_file: Option<PathBuf>,
    start_time: Option<Instant>,
    filter_query: String,
}

impl MainApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let mut app = Self {
            user_setup: config::load(),
            ansicht: Ansicht::SolverOutput,
            solver_process: None,
            line_receiver: None,
            is_running: false,
            solver_output_buffer: Vec::new(),
            residual_data: Vec::new(),
            step_info: Vec::new(),
            available_inp_files: Vec::new(),
            selected_inp_file: None,
            start_time: None,
            filter_query: String::new(),
        };
        app.refresh_inp_files();
        app
    }

    fn refresh_inp_files(&mut self) {
        self.available_inp_files.clear();
        if let Ok(entries) = fs::read_dir(&self.user_setup.project_dir_path) {
            self.available_inp_files = entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("inp"))
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
                            self.solver_output_buffer.push(line);
                        }
                        SolverMessage::Residual(data) => self.residual_data.push(data),
                        SolverMessage::ResetResiduals => self.residual_data.clear(),
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
                        self.start_time = None;
                        break;
                    }
                }
            }
            ctx.request_repaint(); // Request a repaint to show new data
        }

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.hyperlink_to("GitHub", "https://github.com/calculix/ccx_runner");
                egui::warn_if_debug_build(ui);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_dark_light_mode_switch(ui);
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Settings");
            {
                ui.label("Path to Calculix Binary");
                ui.horizontal(|ui| {
                    let mut ccx_path_str = self.user_setup.calculix_bin_path.display().to_string();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut ccx_path_str)
                            .desired_width(ui.available_width() - 50.0),
                    );
                    if response.changed() {
                        self.user_setup.calculix_bin_path = PathBuf::from(ccx_path_str);
                    }

                    if ui.button("…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            self.user_setup.calculix_bin_path = path;
                        }
                    }
                });
            }
            {
                ui.label("Path to project directory");
                ui.horizontal(|ui| {
                    let mut project_dir_str =
                        self.user_setup.project_dir_path.display().to_string();
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut project_dir_str)
                            .desired_width(ui.available_width() - 50.0),
                    );
                    if response.changed() {
                        self.user_setup.project_dir_path = PathBuf::from(project_dir_str);
                        self.refresh_inp_files();
                    }

                    if ui.button("…").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.user_setup.project_dir_path = path;
                            self.refresh_inp_files();
                        }
                    }
                });
            }

            if !self.is_running {
                ui.horizontal(|ui| {
                    let max_cores = default_num_cores();
                    ui.label("Number of Cores:");
                    ui.add(
                        egui::DragValue::new(&mut self.user_setup.num_cores).range(1..=max_cores),
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

                ui.label("Input file");
                egui::ComboBox::from_id_source("inp_file_selector")
                    .selected_text(selected_file_name)
                    .show_ui(ui, |ui| {
                        self.refresh_inp_files();

                        if self.available_inp_files.is_empty() {
                            ui.label("No .inp files found.");
                        } else {
                            // Use a scroll area in case there are many files.
                            egui::ScrollArea::vertical()
                                .max_height(200.0)
                                .show(ui, |ui| {
                                    for f in &self.available_inp_files {
                                        let file_name =
                                            f.file_name().unwrap().to_str().unwrap().to_string();
                                        ui.selectable_value(
                                            &mut self.selected_inp_file,
                                            Some(f.clone()),
                                            file_name,
                                        );
                                    }
                                });
                        }
                    });
            }

            ui.add_space(5.0);

            if self.is_running {
                ui.horizontal(|ui| {
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
                        self.start_time = None;
                    }

                    if let Some(start_time) = self.start_time {
                        let elapsed = start_time.elapsed();
                        ui.label(format!("Running for: {:.1}s", elapsed.as_secs_f32()));
                        ctx.request_repaint();
                    }
                });
            } else if ui.button("Run Analysis").clicked() {
                match config::save(&self.user_setup) {
                    Ok(_) => {} // No-op
                    Err(e) => panic!("{}", e),
                }
                if let Some(inp_path) = self.selected_inp_file.clone() {
                    let job_name = inp_path.file_stem().unwrap().to_str().unwrap();
                    let (sender, receiver) = mpsc::channel::<SolverMessage>();
                    self.line_receiver = Some(receiver);
                    self.is_running = true;
                    self.start_time = Some(Instant::now());
                    self.solver_output_buffer.clear();
                    self.residual_data.clear();
                    self.step_info.clear();

                    let child = crate::solver::spawn_process(
                        &self.user_setup.calculix_bin_path,
                        &self.user_setup.project_dir_path,
                        job_name,
                        self.user_setup.num_cores,
                    );

                    match child {
                        Ok(mut child) => {
                            crate::solver::spawn_reader_thread(&mut child, sender);
                            self.solver_process = Some(Arc::new(Mutex::new(child)));
                        }
                        Err(e) => {
                            self.solver_output_buffer
                                .push(format!("Failed to start process: {}", e));
                            self.is_running = false;
                        }
                    }
                } else {
                    self.solver_output_buffer
                        .push("No '.inp' file selected.".to_string());
                }
            }

            // Tabs
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.ansicht, Ansicht::SolverOutput, "Solver Output");
                ui.selectable_value(&mut self.ansicht, Ansicht::Overview, "Overview");
            });
            ui.separator();

            match self.ansicht {
                Ansicht::SolverOutput => {
                    ui.heading("Solver Output");

                    let hint =
                        "Filter with AND (&) and OR (|). E.g. 'force & iteration | convergence'";
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter_query)
                            .hint_text(hint)
                            .desired_width(f32::INFINITY),
                    );

                    let query = self.filter_query.trim();
                    let filtered_lines: Vec<_> = if query.is_empty() {
                        self.solver_output_buffer.iter().collect()
                    } else {
                        // DNF parsing: OR of ANDs
                        // "a & b | c" -> OR clauses: [["a", "b"], ["c"]]
                        let or_clauses: Vec<Vec<String>> = query
                            .split('|')
                            .map(|or_part| {
                                or_part
                                    .split('&')
                                    .map(|s| s.trim().to_lowercase())
                                    .filter(|s| !s.is_empty())
                                    .collect()
                            })
                            .filter(|and_terms: &Vec<String>| !and_terms.is_empty())
                            .collect();

                        self.solver_output_buffer
                            .iter()
                            .filter(|line| {
                                let lower_line = line.to_lowercase();
                                // A line matches if it matches ANY of the OR clauses
                                or_clauses.iter().any(|and_terms| {
                                    // An OR clause matches if the line contains ALL of its AND terms
                                    and_terms.iter().all(|term| lower_line.contains(term))
                                })
                            })
                            .collect()
                    };

                    let row_height = ui.text_style_height(&egui::TextStyle::Monospace);
                    let num_rows = filtered_lines.len();

                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show_rows(ui, row_height, num_rows, |ui, row_range| {
                            for i in row_range {
                                if let Some(line) = filtered_lines.get(i) {
                                    ui.label(egui::RichText::new(*line).monospace());
                                }
                            }
                        });
                }

                Ansicht::Overview => {
                    ui.heading("Residual Plot");
                    let points: PlotPoints = self
                        .residual_data
                        .iter()
                        .map(|d| [d.total_iteration as f64, d.residual])
                        .collect();
                    let line = Line::new(points);

                    Plot::new("residual_plot")
                        .height(250.0)
                        .legend(egui_plot::Legend::default())
                        .x_axis_label("Total Iterations")
                        .show(ui, |plot_ui| {
                            plot_ui.line(line.name("Largest Residual"));
                        });

                    ui.add_space(10.0);

                    // Step Table
                    ui.heading("Step Information");
                    egui::Grid::new("step_grid").striped(true).show(ui, |ui| {
                        ui.label("Step");
                        ui.label("Increment");
                        ui.label("Attempt");
                        ui.label("Iterations");
                        ui.label("Total Time");
                        ui.end_row();

                        for data in &self.step_info {
                            ui.label(data.step.to_string());
                            ui.label(data.increment.to_string());
                            ui.label(data.attempt.to_string());
                            ui.label(data.iterations.to_string());
                            ui.label(format!("{:.4e}", data.total_time));
                            ui.end_row();
                        }
                    });
                }
            }
        });
    }
}
