use std::fs;
use std::path::{Path, PathBuf};

use crate::models::Config;

pub struct StorageService;

impl StorageService {
    pub fn config_path() -> PathBuf {
        PathBuf::from("config.json")
    }

    pub fn load_config() -> Config {
        Self::load_from_path(&Self::config_path())
    }

    pub fn load_from_path(path: &Path) -> Config {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok((cfg, migrated)) = Config::from_json_with_migration(&content) {
                    if migrated {
                        let _ = Self::save_to_path(path, &cfg);
                    }
                    return cfg;
                }
            }
        }
        let default_cfg = Config::default();
        let _ = Self::save_to_path(path, &default_cfg);
        default_cfg
    }

    pub fn save_config(config: &Config) -> Result<(), String> {
        Self::save_to_path(&Self::config_path(), config)
    }

    /// Writes configuration atomically via a temporary file + rename.
    pub fn save_to_path(path: &Path, config: &Config) -> Result<(), String> {
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json).map_err(|e| format!("Failed to write temp config: {}", e))?;

        if path.exists() {
            // On Windows, fs::rename can fail if destination exists on older filesystems; remove first or replace
            let _ = fs::remove_file(path);
        }
        fs::rename(&tmp_path, path)
            .map_err(|e| format!("Failed to finalize atomic config write: {}", e))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_save_and_migration() {
        let dir = std::env::temp_dir().join("roblox_watcher_storage_test");
        let _ = fs::create_dir_all(&dir);
        let cfg_path = dir.join("config.json");
        let _ = fs::remove_file(&cfg_path);

        // Write legacy v0 file
        let legacy = r#"{"checkIntervalSeconds": 6.0, "reconnectButtonText": "Reconnect"}"#;
        fs::write(&cfg_path, legacy).unwrap();

        let loaded = StorageService::load_from_path(&cfg_path);
        assert_eq!(loaded.schema_version, crate::models::CURRENT_CONFIG_SCHEMA_VERSION);
        assert_eq!(loaded.check_interval_seconds, 6.0);
        assert_eq!(loaded.reconnect_button_phrases, vec!["Reconnect".to_string()]);

        let _ = fs::remove_dir_all(&dir);
    }
}
