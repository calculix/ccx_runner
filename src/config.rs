use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs::{create_dir_all, File};
use std::io::{Read, Write};
use dirs::config_dir;

pub fn default_num_cores() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UserSetup {
    pub calculix_bin_path: PathBuf,
    pub project_dir_path: PathBuf,
    #[serde(default = "default_num_cores")]
    pub num_cores: usize,
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

pub fn load() -> UserSetup {
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

pub fn save(user_setup: &UserSetup) -> Result<(), std::io::Error> {
    let config_dir = config_dir().unwrap().join("ccx_runner_rs");
    let config_file = config_dir.join("config.json");
    let json = serde_json::to_string_pretty(user_setup).unwrap();
    let mut file = File::create(config_file)?;
    file.write_all(json.as_bytes())?;

    Ok(())
}
