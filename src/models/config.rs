use serde::{Deserialize, Serialize};

use crate::models::macro_seq::MacroStep;

pub const CURRENT_CONFIG_SCHEMA_VERSION: u32 = 2;

pub fn default_target_process_names() -> Vec<String> {
    vec![
        "RobloxPlayerBeta.exe".to_string(),
        "RobloxPlayer.exe".to_string(),
    ]
}

pub fn default_fallback_window_title() -> String {
    "Roblox".to_string()
}

pub fn default_reconnect_button_phrases() -> Vec<String> {
    vec![
        "Reconnect".to_string(),
        "Retry".to_string(),
        "Reconectar".to_string(),
        "Połącz ponownie".to_string(),
        "Ponów".to_string(),
        "Erneut verbinden".to_string(),
        "Erneut versuchen".to_string(),
        "Se reconnecter".to_string(),
        "Reconnecter".to_string(),
        "Réessayer".to_string(),
        "Riconnetti".to_string(),
        "Переподключиться".to_string(),
        "Повторить".to_string(),
        "Yeniden Bağlan".to_string(),
        "Sambung Ulang".to_string(),
        "Kết nối lại".to_string(),
        "再接続".to_string(),
        "다시 연결".to_string(),
        "重新连接".to_string(),
        "重新連接".to_string(),
    ]
}

pub fn default_disconnect_keywords() -> Vec<String> {
    vec![
        "disconnected".to_string(),
        "lost connection".to_string(),
        "connection lost".to_string(),
        "connection closed".to_string(),
        "please reconnect".to_string(),
        "server was shut down".to_string(),
        "server has closed".to_string(),
        "join error".to_string(),
        "http error".to_string(),
        "error code:".to_string(),
        "error code".to_string(),
        "rozłączono".to_string(),
        "utracono połączenie".to_string(),
        "kod błędu:".to_string(),
        "kod błędu".to_string(),
        "desconectado".to_string(),
        "código de error:".to_string(),
        "verbindung getrennt".to_string(),
        "fehlercode:".to_string(),
        "déconnecté".to_string(),
        "code d'erreur:".to_string(),
    ]
}

pub fn default_ignore_phrases() -> Vec<String> {
    vec!["voice chat".to_string(), "voice".to_string()]
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct UiStringsConfig {
    pub app_title: String,
    pub status_idle: String,
    pub status_active: String,
    pub status_standby: String,
    pub status_healthy: String,
    pub status_traced: String,
    pub status_no_event: String,
}

impl Default for UiStringsConfig {
    fn default() -> Self {
        Self {
            app_title: "Roblox Automator".to_string(),
            status_idle: "Idle".to_string(),
            status_active: "Active".to_string(),
            status_standby: "Standby".to_string(),
            status_healthy: "Healthy".to_string(),
            status_traced: "Traced ✓".to_string(),
            status_no_event: "None".to_string(),
        }
    }
}

pub fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> serde::de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let list: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(list)
        }

        fn visit_seq<S>(self, mut visitor: S) -> Result<Self::Value, S::Error>
        where
            S: serde::de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(elem) = visitor.next_element::<String>()? {
                let trimmed = elem.trim().to_string();
                if !trimmed.is_empty() {
                    vec.push(trimmed);
                }
            }
            Ok(vec)
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub schema_version: u32,
    pub check_interval_seconds: f64,
    pub ui_refresh_interval_ms: u64,
    pub reconnect_cooldown_seconds: f64,
    pub ocr_timeout_ms: u64,
    pub thumbnail_max_width: u32,
    pub thumbnail_max_height: u32,
    pub thumbnail_jpeg_quality: u8,
    pub vip_server_url: Option<String>,
    pub auto_click_reconnect: bool,
    pub auto_relaunch_on_kick: bool,
    pub auto_teleport_to_event: bool,
    pub anti_afk_enabled: bool,
    pub anti_afk_interval_seconds: f64,
    #[serde(
        default = "default_target_process_names",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub target_process_names: Vec<String>,
    #[serde(default = "default_fallback_window_title")]
    pub fallback_window_title: String,
    #[serde(
        default = "default_reconnect_button_phrases",
        alias = "reconnectButtonText",
        alias = "reconnectButtonPhrases",
        alias = "disconnectButtonPhrases",
        alias = "disconnectButtonKeywords",
        alias = "reconnectButtonKeywords",
        alias = "reconnect_button_text",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub reconnect_button_phrases: Vec<String>,
    #[serde(
        default = "default_disconnect_keywords",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub disconnect_keywords: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub event_target_keywords: Vec<String>,
    #[serde(
        default = "default_ignore_phrases",
        deserialize_with = "deserialize_string_or_vec"
    )]
    pub ignore_phrases: Vec<String>,
    pub ui_strings: UiStringsConfig,
    #[serde(default)]
    pub macro_enabled: bool,
    #[serde(default = "default_true")]
    pub macro_replay_on_reconnect: bool,
    #[serde(default = "default_true")]
    pub macro_replay_on_event: bool,
    #[serde(default = "default_true")]
    pub uniform_window_size: bool,
    #[serde(default = "default_uniform_width")]
    pub uniform_window_width: i32,
    #[serde(default = "default_uniform_height")]
    pub uniform_window_height: i32,
    #[serde(default)]
    pub macro_steps: Vec<MacroStep>,
}

fn default_true() -> bool {
    true
}

fn default_uniform_width() -> i32 {
    800
}

fn default_uniform_height() -> i32 {
    600
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_CONFIG_SCHEMA_VERSION,
            check_interval_seconds: 15.0,
            ui_refresh_interval_ms: 2000,
            reconnect_cooldown_seconds: 30.0,
            ocr_timeout_ms: 3000,
            thumbnail_max_width: 320,
            thumbnail_max_height: 180,
            thumbnail_jpeg_quality: 75,
            vip_server_url: None,
            auto_click_reconnect: true,
            auto_relaunch_on_kick: true,
            auto_teleport_to_event: false,
            anti_afk_enabled: true,
            anti_afk_interval_seconds: 600.0,
            target_process_names: default_target_process_names(),
            fallback_window_title: default_fallback_window_title(),
            reconnect_button_phrases: default_reconnect_button_phrases(),
            disconnect_keywords: default_disconnect_keywords(),
            event_target_keywords: Vec::new(),
            ignore_phrases: default_ignore_phrases(),
            ui_strings: UiStringsConfig::default(),
            macro_enabled: false,
            macro_replay_on_reconnect: true,
            macro_replay_on_event: true,
            uniform_window_size: true,
            uniform_window_width: 800,
            uniform_window_height: 600,
            macro_steps: Vec::new(),
        }
    }
}

impl Config {
    /// Validates all numeric ranges, paths/URLs, and externalized string lists.
    pub fn validate(&mut self) -> Result<(), String> {
        if !self.check_interval_seconds.is_finite()
            || self.check_interval_seconds < 1.0
            || self.check_interval_seconds > 60.0
        {
            return Err("Check interval must be between 1 and 60 seconds.".to_string());
        }
        if self.ui_refresh_interval_ms < 250 || self.ui_refresh_interval_ms > 10000 {
            return Err("UI refresh interval must be between 0.25 and 10 seconds.".to_string());
        }
        if !self.reconnect_cooldown_seconds.is_finite()
            || self.reconnect_cooldown_seconds < 5.0
            || self.reconnect_cooldown_seconds > 600.0
        {
            return Err("Reconnect cooldown must be between 5 and 600 seconds.".to_string());
        }
        if self.ocr_timeout_ms < 500 || self.ocr_timeout_ms > 15000 {
            return Err("OCR timeout must be between 500 and 15000 ms.".to_string());
        }
        if self.thumbnail_max_width < 160 || self.thumbnail_max_width > 640 {
            return Err("Thumbnail max width must be between 160 and 640 pixels.".to_string());
        }
        if self.thumbnail_max_height < 90 || self.thumbnail_max_height > 360 {
            return Err("Thumbnail max height must be between 90 and 360 pixels.".to_string());
        }
        if self.thumbnail_jpeg_quality < 30 || self.thumbnail_jpeg_quality > 95 {
            return Err("Thumbnail JPEG quality must be between 30 and 95.".to_string());
        }
        if !self.anti_afk_interval_seconds.is_finite()
            || self.anti_afk_interval_seconds < 60.0
            || self.anti_afk_interval_seconds > 1140.0
        {
            return Err("Anti-AFK interval must be between 1 and 19 minutes.".to_string());
        }

        if !(640..=3840).contains(&self.uniform_window_width) {
            return Err("Uniform window width must be between 640 and 3840 pixels.".to_string());
        }
        if !(480..=2160).contains(&self.uniform_window_height) {
            return Err("Uniform window height must be between 480 and 2160 pixels.".to_string());
        }
        if self.macro_steps.len() > 5000 {
            self.macro_steps.truncate(5000);
        }

        if let Some(ref url) = self.vip_server_url {
            let trimmed = url.trim();
            if trimmed.is_empty() {
                self.vip_server_url = None;
            } else if !trimmed.starts_with("https://") && !trimmed.starts_with("roblox://") {
                return Err("VIP Server URL must start with https:// or roblox://.".to_string());
            } else {
                self.vip_server_url = Some(trimmed.to_string());
            }
        }

        Self::clean_list(&mut self.target_process_names);
        Self::clean_list(&mut self.reconnect_button_phrases);
        Self::clean_list(&mut self.disconnect_keywords);
        Self::clean_list(&mut self.event_target_keywords);
        Self::clean_list(&mut self.ignore_phrases);

        if self.target_process_names.is_empty() {
            return Err("At least one target process name must be configured.".to_string());
        }
        if self.reconnect_button_phrases.is_empty() {
            return Err("At least one reconnect button phrase must be configured.".to_string());
        }
        if self.fallback_window_title.trim().is_empty() {
            self.fallback_window_title = default_fallback_window_title();
        } else {
            self.fallback_window_title = self.fallback_window_title.trim().to_string();
        }
        if self.ui_strings.app_title.trim().is_empty() {
            self.ui_strings.app_title = UiStringsConfig::default().app_title;
        }

        self.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;
        Ok(())
    }

    fn clean_list(list: &mut Vec<String>) {
        let mut seen = std::collections::HashSet::new();
        list.retain(|item| {
            let trimmed = item.trim();
            !trimmed.is_empty() && seen.insert(trimmed.to_lowercase())
        });
        for item in list.iter_mut() {
            *item = item.trim().to_string();
        }
    }

    /// Migrates raw persisted JSON from older schema versions to CURRENT_CONFIG_SCHEMA_VERSION.
    pub fn from_json_with_migration(raw: &str) -> Result<(Self, bool), String> {
        let val: serde_json::Value =
            serde_json::from_str(raw).map_err(|e| format!("Invalid JSON: {}", e))?;
        let version = val
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        let mut cfg: Config =
            serde_json::from_value(val).map_err(|e| format!("Config decode error: {}", e))?;
        let migrated = version < CURRENT_CONFIG_SCHEMA_VERSION;
        if migrated {
            cfg.schema_version = CURRENT_CONFIG_SCHEMA_VERSION;
        }
        cfg.validate()?;
        Ok((cfg, migrated))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_is_valid() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.schema_version, CURRENT_CONFIG_SCHEMA_VERSION);
        assert!(config
            .target_process_names
            .contains(&"RobloxPlayerBeta.exe".to_string()));
        assert!(config
            .reconnect_button_phrases
            .contains(&"Reconnect".to_string()));
        assert!(config
            .reconnect_button_phrases
            .contains(&"Połącz ponownie".to_string()));
    }

    #[test]
    fn test_config_validation_rejects_bad_ranges_and_urls() {
        let mut cfg = Config::default();
        cfg.check_interval_seconds = 0.1;
        assert!(cfg.validate().is_err());

        let mut cfg = Config::default();
        cfg.vip_server_url = Some("http://insecure.example.com".to_string());
        assert!(cfg.validate().is_err());

        let mut cfg = Config::default();
        cfg.reconnect_button_phrases = vec![];
        assert!(cfg.validate().is_err());

        let mut cfg = Config::default();
        cfg.target_process_names = vec![];
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_config_migration_from_legacy_v0() {
        let json = r#"{
            "checkIntervalSeconds": 4.0,
            "reconnectButtonText": "Reconnect, Połącz ponownie"
        }"#;
        let (cfg, migrated) = Config::from_json_with_migration(json).expect("migration failed");
        assert!(migrated);
        assert_eq!(cfg.schema_version, CURRENT_CONFIG_SCHEMA_VERSION);
        assert_eq!(
            cfg.reconnect_button_phrases,
            vec!["Reconnect".to_string(), "Połącz ponownie".to_string()]
        );
    }

    #[test]
    fn test_config_deserialize_from_json_array() {
        let json = r#"{"schemaVersion": 2, "reconnectButtonPhrases": ["Reconnect", "Wiederverbinden"], "targetProcessNames": ["CustomRoblox.exe"]}"#;
        let (cfg, migrated) = Config::from_json_with_migration(json).expect("decode failed");
        assert!(!migrated);
        assert_eq!(
            cfg.reconnect_button_phrases,
            vec!["Reconnect".to_string(), "Wiederverbinden".to_string()]
        );
        assert_eq!(
            cfg.target_process_names,
            vec!["CustomRoblox.exe".to_string()]
        );
    }
}
