use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub idle_threshold_mins: u32,
    pub start_minimized: bool,
    pub show_seconds: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            idle_threshold_mins: 5,
            start_minimized: false,
            show_seconds: false,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}
