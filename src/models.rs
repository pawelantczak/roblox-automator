use serde::{Deserialize, Serialize};

// MARK: - Configuration Model

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
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
    pub reconnect_button_text: String,
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
            reconnect_button_text: "Reconnect".to_string(),
            disconnect_keywords: vec![
                "disconnected".to_string(),
                "lost connection".to_string(),
                "error code: 277".to_string(),
                "error code: 279".to_string(),
                "error code: 267".to_string(),
                "error code: 271".to_string(),
                "error code: 268".to_string(),
                "error code: 273".to_string(),
                "server was shut down".to_string(),
                "server has closed".to_string(),
                "please reconnect".to_string(),
                "connection lost".to_string(),
                "connection closed".to_string(),
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
