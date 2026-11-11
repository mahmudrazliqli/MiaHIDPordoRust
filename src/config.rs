use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct Config {
    #[serde(default)]
    pub hex_mode: bool,
    #[serde(default)]
    pub autoconnect: bool,
    #[serde(default)]
    pub window_width: i32,
    #[serde(default)]
    pub window_height: i32,
    #[serde(default)]
    pub window_x: i32,
    #[serde(default)]
    pub window_y: i32,
    #[serde(default = "default_font")]
    pub log_font_size: i32,
    #[serde(default)]
    pub last_device: String,
}

fn default_font() -> i32 {
    10
}

pub fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = PathBuf::from(home).join(".config");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("miahidpordo.toml")
}

pub fn load() -> Config {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(cfg: &Config) {
    let path = config_path();
    match toml::to_string_pretty(cfg) {
        Ok(s) => {
            if let Err(e) = std::fs::write(&path, s) {
                eprintln!("save_config failed: {}", e);
            }
        }
        Err(e) => eprintln!("toml serialize failed: {}", e),
    }
}
