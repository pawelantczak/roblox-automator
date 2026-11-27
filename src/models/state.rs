use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use crate::models::{Config, MacroStep};

pub type InstanceId = u64;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InstanceState {
    Idle,
    Discovering,
    Healthy,
    Disconnected,
    Reconnecting,
    Navigating,
    Error,
}

impl std::fmt::Display for InstanceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstanceState::Idle => write!(f, "IDLE"),
            InstanceState::Discovering => write!(f, "DISCOVERING"),
            InstanceState::Healthy => write!(f, "HEALTHY"),
            InstanceState::Disconnected => write!(f, "DISCONNECTED"),
            InstanceState::Reconnecting => write!(f, "RECONNECTING"),
            InstanceState::Navigating => write!(f, "NAVIGATING"),
            InstanceState::Error => write!(f, "ERROR"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FailureReason {
    CaptureFailed(String),
    OcrTimeout,
    OcrFailed(String),
    FocusFailed(String),
    InputFailed(String),
    ReconnectCooldownActive,
    ReconnectButtonNotFound,
}

impl std::fmt::Display for FailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureReason::CaptureFailed(e) => write!(f, "Capture failed: {}", e),
            FailureReason::OcrTimeout => write!(f, "OCR timed out"),
            FailureReason::OcrFailed(e) => write!(f, "OCR failed: {}", e),
            FailureReason::FocusFailed(e) => write!(f, "Focus failed: {}", e),
            FailureReason::InputFailed(e) => write!(f, "Input failed: {}", e),
            FailureReason::ReconnectCooldownActive => write!(f, "Reconnect cooldown active"),
            FailureReason::ReconnectButtonNotFound => write!(f, "Reconnect button not found"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Action,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigLogSnapshot {
    pub schema_version: u32,
    pub check_interval_seconds: f64,
    pub ui_refresh_interval_ms: u64,
    pub reconnect_cooldown_seconds: f64,
    pub ocr_timeout_ms: u64,
    pub auto_click_reconnect: bool,
    pub auto_relaunch_on_kick: bool,
    pub auto_teleport_to_event: bool,
    pub anti_afk_enabled: bool,
    pub anti_afk_interval_seconds: f64,
    pub has_vip_server_url: bool,
    pub target_process_names: Vec<String>,
    pub reconnect_button_phrases: Vec<String>,
    pub disconnect_keywords: Vec<String>,
    pub event_target_keywords: Vec<String>,
    pub ignore_phrases: Vec<String>,
    pub uniform_window_size: bool,
    pub uniform_window_width: i32,
    pub uniform_window_height: i32,
    pub macro_enabled: bool,
    pub macro_replay_on_reconnect: bool,
    pub macro_replay_on_event: bool,
    pub macro_step_count: usize,
    pub macro_duration_ms: u64,
}

impl From<&Config> for ConfigLogSnapshot {
    fn from(cfg: &Config) -> Self {
        Self {
            schema_version: cfg.schema_version,
            check_interval_seconds: cfg.check_interval_seconds,
            ui_refresh_interval_ms: cfg.ui_refresh_interval_ms,
            reconnect_cooldown_seconds: cfg.reconnect_cooldown_seconds,
            ocr_timeout_ms: cfg.ocr_timeout_ms,
            auto_click_reconnect: cfg.auto_click_reconnect,
            auto_relaunch_on_kick: cfg.auto_relaunch_on_kick,
            auto_teleport_to_event: cfg.auto_teleport_to_event,
            anti_afk_enabled: cfg.anti_afk_enabled,
            anti_afk_interval_seconds: cfg.anti_afk_interval_seconds,
            has_vip_server_url: cfg.vip_server_url.is_some(),
            target_process_names: cfg.target_process_names.clone(),
            reconnect_button_phrases: cfg.reconnect_button_phrases.clone(),
            disconnect_keywords: cfg.disconnect_keywords.clone(),
            event_target_keywords: cfg.event_target_keywords.clone(),
            ignore_phrases: cfg.ignore_phrases.clone(),
            uniform_window_size: cfg.uniform_window_size,
            uniform_window_width: cfg.uniform_window_width,
            uniform_window_height: cfg.uniform_window_height,
            macro_enabled: cfg.macro_enabled,
            macro_replay_on_reconnect: cfg.macro_replay_on_reconnect,
            macro_replay_on_event: cfg.macro_replay_on_event,
            macro_step_count: cfg.macro_steps.len(),
            macro_duration_ms: MacroStep::total_duration_ms(&cfg.macro_steps),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub version: String,
    pub timestamp: String,
    pub level: LogLevel,
    pub instance_id: Option<InstanceId>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ConfigLogSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EngineMetrics {
    pub captures: u64,
    pub capture_failures: u64,
    pub ocr_runs: u64,
    pub ocr_failures: u64,
    pub ocr_skipped_unchanged: u64,
    pub reconnect_attempts: u64,
    pub reconnect_failures: u64,
    pub anti_afk_jumps: u64,
    pub input_failures: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InstanceCard {
    pub instance_id: InstanceId,
    pub pid: u32,
    pub title: String,
    pub state: InstanceState,
    pub failure_reason: Option<String>,
    pub afk_remaining_secs: Option<u64>,
    pub cooldown_remaining_secs: Option<u64>,
    pub event_status: String,
    pub restarts: u32,
    pub ocr_failures: u32,
    pub screenshot_base64: Option<String>,
    pub event_crop_base64: Option<String>,
    pub reconnect_crop_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UIStateSnapshot {
    pub app_version: String,
    pub running: bool,
    pub instances: Vec<InstanceCard>,
    pub queue_status: String,
    pub event_keyword: String,
    pub auto_teleport_to_event: bool,
    pub event_target_keywords: Vec<String>,
    pub scan_interval_seconds: u32,
    pub next_scan_remaining_secs: Option<u64>,
    pub session_uptime_secs: Option<u64>,
    pub total_restarts: u32,
    pub anti_afk_enabled: bool,
    pub anti_afk_interval_minutes: u32,
    pub next_afk_remaining_secs: Option<u64>,
    pub macro_enabled: bool,
    pub macro_recording: bool,
    pub macro_playing: bool,
    pub macro_countdown_secs: Option<u8>,
    pub macro_step_count: usize,
    pub macro_duration_ms: u64,
    pub logs: Vec<LogEntry>,
    pub metrics: EngineMetrics,
}

pub struct BoundedLogQueue {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    jsonl_path: Option<PathBuf>,
}

impl BoundedLogQueue {
    #[cfg(test)]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
            jsonl_path: None,
        }
    }

    pub fn new_with_file(capacity: usize, jsonl_path: PathBuf) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
            jsonl_path: Some(jsonl_path),
        }
    }

    pub fn push(&mut self, level: LogLevel, instance_id: Option<InstanceId>, message: impl Into<String>) {
        self.push_with_icon(level, instance_id, message, None);
    }

    pub fn push_with_icon(
        &mut self,
        level: LogLevel,
        instance_id: Option<InstanceId>,
        message: impl Into<String>,
        icon_base64: Option<String>,
    ) {
        self.push_entry(level, instance_id, message, icon_base64, None);
    }

    pub fn push_with_config(
        &mut self,
        level: LogLevel,
        instance_id: Option<InstanceId>,
        message: impl Into<String>,
        config: &Config,
    ) {
        self.push_entry(
            level,
            instance_id,
            message,
            None,
            Some(ConfigLogSnapshot::from(config)),
        );
    }

    fn push_entry(
        &mut self,
        level: LogLevel,
        instance_id: Option<InstanceId>,
        message: impl Into<String>,
        icon_base64: Option<String>,
        config: Option<ConfigLogSnapshot>,
    ) {
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        let entry = LogEntry {
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: current_timestamp_hms(),
            level,
            instance_id,
            message: message.into(),
            icon_base64,
            config,
        };
        if let Some(ref path) = self.jsonl_path {
            if let Ok(line) = serde_json::to_string(&entry) {
                if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                    let _ = writeln!(file, "{}", line);
                }
            }
        }
        self.entries.push_back(entry);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn snapshot(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    pub fn export_jsonl(&self) -> String {
        if let Some(ref path) = self.jsonl_path {
            if let Ok(content) = std::fs::read_to_string(path) {
                if !content.is_empty() {
                    return content;
                }
            }
        }
        self.entries
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn current_timestamp_hms() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_log_queue_evicts_oldest() {
        let mut q = BoundedLogQueue::new(3);
        q.push(LogLevel::Info, None, "one");
        q.push(LogLevel::Info, Some(10), "two");
        q.push(LogLevel::Warn, Some(20), "three");
        q.push(LogLevel::Error, None, "four");

        let snap = q.snapshot();
        assert_eq!(snap.len(), 3);
        assert_eq!(snap[0].message, "two");
        assert_eq!(snap[0].instance_id, Some(10));
        assert_eq!(snap[2].message, "four");
    }

    #[test]
    fn test_jsonl_logging_with_icon_base64() {
        let dir = std::env::temp_dir().join("roblox_watcher_jsonl_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_logs.jsonl");
        let _ = std::fs::remove_file(&path);

        let mut q = BoundedLogQueue::new_with_file(5, path.clone());
        q.push(LogLevel::Info, None, "startup");
        q.push_with_icon(
            LogLevel::Action,
            Some(42),
            "Event found",
            Some("BASE64ICON".to_string()),
        );

        let jsonl = q.export_jsonl();
        let lines: Vec<&str> = jsonl.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(!lines[0].contains("iconBase64"));
        assert!(lines[1].contains("\"iconBase64\":\"BASE64ICON\""));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
