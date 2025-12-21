use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::thread;

#[derive(Debug, Clone, Default)]
pub struct StepInfo {
    pub step: u32,
    pub increment: u32,
    pub attempt: u32,
    pub iterations: u32,
    pub total_time: f64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ResidualData {
    pub step: u32,
    pub total_iteration: u32,
    pub residual: f64,
}

pub enum SolverMessage {
    Line(String),
    NewStepInfo(StepInfo),
    UpdateStepInfo(StepInfo),
    Residual(ResidualData),
    ResetResiduals,
}

pub fn spawn_process(
    ccx_path: &std::path::Path,
    project_dir: &std::path::Path,
    job_name: &str,
    num_cores: usize,
) -> Result<Child, std::io::Error> {
    let num_cores = num_cores.to_string();
    Command::new(ccx_path)
        .arg("-i")
        .arg(job_name)
        .env("OMP_NUM_THREADS", &num_cores)
        .env("CCX_NPROC", &num_cores)
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
}

pub fn spawn_reader_thread(child: &mut Child, sender: Sender<SolverMessage>) {
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);

    thread::spawn(move || {
        let sender_clone = sender; // The move closure takes ownership of sender.
        let mut current_step_info: Option<StepInfo> = None;
        let mut total_iterations_for_residual = 0;

        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    if line.trim().starts_with("STEP") {
                        if let Some(step_str) = line.split_whitespace().last() {
                            if let Ok(step_num) = step_str.parse::<u32>() {
                                let new_info = StepInfo {
                                    step: step_num,
                                    ..Default::default()
                                };
                                current_step_info = Some(new_info.clone());
                                if sender_clone.send(SolverMessage::NewStepInfo(new_info)).is_err()
                                {
                                    break;
                                }
                            }
                        }
                    } else if let Some(info) = current_step_info.as_mut() {
                        let mut updated = false;
                        if line.trim().starts_with("increment ") {
                            if sender_clone.send(SolverMessage::ResetResiduals).is_err() {
                                break;
                            }
                            total_iterations_for_residual = 0;
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 4 {
                                if let (Ok(inc), Ok(att)) =
                                    (parts[1].parse::<u32>(), parts[3].parse::<u32>())
                                {
                                    info.increment = inc;
                                    info.attempt = att;
                                    info.iterations = 0; // Reset for new attempt
                                    updated = true;
                                }
                            }
                        } else if line.trim().starts_with("iteration ") {
                            info.iterations += 1;
                            updated = true;
                        } else if line.starts_with(" actual total time=") {
                            if let Some(val_str) = line.split('=').nth(1) {
                                if let Ok(val) = val_str.trim().parse::<f64>() {
                                    info.total_time = val;
                                    updated = true;
                                }
                            }
                        } else if line.trim().starts_with("largest residual force=") {
                            if let Some(val_str) = line.split('=').nth(1) {
                                if let Some(residual_str) = val_str.trim().split_whitespace().next()
                                {
                                    if let Ok(residual) = residual_str.parse::<f64>() {
                                        total_iterations_for_residual += 1;
                                        let residual_data = ResidualData {
                                            step: info.step,
                                            total_iteration: total_iterations_for_residual,
                                            residual,
                                        };
                                        if sender_clone
                                            .send(SolverMessage::Residual(residual_data))
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        if updated {
                            if sender_clone
                                .send(SolverMessage::UpdateStepInfo(info.clone()))
                                .is_err()
                            {
                                break;
                            }
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
}