use crate::models::Config;
use std::fs;
use std::path::PathBuf;

pub struct StorageService;

impl StorageService {
    pub fn config_path() -> PathBuf {
        PathBuf::from("config.json")
    }

    pub fn load_config() -> Config {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<Config>(&content) {
                    return cfg;
                }
            }
        }
        let default_cfg = Config::default();
        Self::save_config(&default_cfg);
        default_cfg
    }

    pub fn save_config(config: &Config) {
        let path = Self::config_path();
        if let Ok(json) = serde_json::to_string_pretty(config) {
            let _ = fs::write(path, json);
        }
    }
}
