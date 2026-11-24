use serde::{Deserialize, Serialize};

// MARK: - Configuration Model

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
            if list.is_empty() {
                Ok(vec![value.trim().to_string()])
            } else {
                Ok(list)
            }
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
    pub check_interval_seconds: f64,
    pub reconnect_cooldown_seconds: f64,
    pub place_id: Option<String>,
    pub vip_server_url: Option<String>,
    pub auto_click_reconnect: bool,
    pub auto_relaunch_on_kick: bool,
    pub auto_teleport_to_event: bool,
    pub anti_afk_enabled: bool,
    pub anti_afk_interval_seconds: f64,
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
    pub disconnect_keywords: Vec<String>,
    pub event_target_keywords: Vec<String>,
    pub ignore_phrases: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            check_interval_seconds: 5.0,
            reconnect_cooldown_seconds: 30.0,
            place_id: Some("8737899170".to_string()),
            vip_server_url: None,
            auto_click_reconnect: true,
            auto_relaunch_on_kick: true,
            auto_teleport_to_event: true,
            anti_afk_enabled: true,
            anti_afk_interval_seconds: 600.0, // 10 minutes
            reconnect_button_phrases: default_reconnect_button_phrases(),
            disconnect_keywords: vec![
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
            ],
            event_target_keywords: vec!["turkey hunt".to_string()],
            ignore_phrases: vec!["voice chat".to_string(), "voice".to_string()],
        }
    }
}

// MARK: - UI & Instance Telemetry Models

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceCard {
    pub id: usize,
    pub pid: u32,
    pub hwnd: isize,
    pub title: String,
    pub place_id: Option<String>,
    pub status: String,
    pub afk_countdown: String,
    pub event_status: String,
    pub restarts: u32,
    pub screenshot_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UIStateBroadcast {
    pub running: bool,
    pub instances: Vec<InstanceCard>,
    pub queue_status: String,
    pub event_keyword: String,
    pub next_afk_countdown: String,
    pub logs: Vec<String>,
}

// MARK: - IPC Action Deserialization

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase")]
pub enum IPCAction {
    Start,
    Stop,
    Rescan,
    TestJump,
    ClearLogs,
    GetConfig,
    SaveConfig { config: Config },
}

// MARK: - Vision & OCR Entities

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedRect {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl NormalizedRect {
    pub fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self { min_x, min_y, max_x, max_y }
    }

    pub fn mid_x(&self) -> f32 {
        (self.min_x + self.max_x) * 0.5
    }

    pub fn mid_y(&self) -> f32 {
        (self.min_y + self.max_y) * 0.5
    }

    pub fn width(&self) -> f32 {
        (self.max_x - self.min_x).abs()
    }

    pub fn height(&self) -> f32 {
        (self.max_y - self.min_y).abs()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedWord {
    pub word: String,
    pub bounding_box: NormalizedRect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetectedText {
    pub text: String,
    pub bounding_box: NormalizedRect,
    pub target_point: Option<NormalizedPoint>,
    pub whole_icon_box: Option<NormalizedRect>,
    pub word_boxes: Vec<DetectedWord>,
}

impl DetectedText {
    pub fn click_point(&self) -> NormalizedPoint {
        if let Some(ref pt) = self.target_point {
            pt.clone()
        } else {
            NormalizedPoint {
                x: self.bounding_box.mid_x(),
                y: self.bounding_box.mid_y(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_has_multilingual_phrases() {
        let config = Config::default();
        assert!(config.reconnect_button_phrases.contains(&"Reconnect".to_string()));
        assert!(config.reconnect_button_phrases.contains(&"Połącz ponownie".to_string()));
        assert!(config.reconnect_button_phrases.contains(&"Reconectar".to_string()));
        assert!(config.reconnect_button_phrases.contains(&"Erneut verbinden".to_string()));
    }

    #[test]
    fn test_config_deserialize_from_legacy_single_string() {
        let json = r#"{"reconnectButtonText": "Custom Reconnect"}"#;
        let config: Config = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(config.reconnect_button_phrases, vec!["Custom Reconnect".to_string()]);
    }

    #[test]
    fn test_config_deserialize_from_comma_separated_string() {
        let json = r#"{"reconnectButtonPhrases": "Reconnect, Połącz ponownie, Reconectar"}"#;
        let config: Config = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(
            config.reconnect_button_phrases,
            vec![
                "Reconnect".to_string(),
                "Połącz ponownie".to_string(),
                "Reconectar".to_string()
            ]
        );
    }

    #[test]
    fn test_config_deserialize_from_json_array() {
        let json = r#"{"reconnectButtonPhrases": ["Reconnect", "Wiederverbinden"]}"#;
        let config: Config = serde_json::from_str(json).expect("deserialization failed");
        assert_eq!(
            config.reconnect_button_phrases,
            vec!["Reconnect".to_string(), "Wiederverbinden".to_string()]
        );
    }
}
