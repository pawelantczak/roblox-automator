use serde::{Deserialize, Serialize};

use crate::models::config::Config;
use crate::models::state::UIStateSnapshot;

/// Typed commands sent from the WebView UI to the Rust backend.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UiCommand {
    StartAutomator,
    StopAutomator,
    TriggerTestJump,
    ClearLogs,
    GetConfig,
    SaveConfig { config: Config },
    SetAntiAfkInterval { minutes: u32 },
    SetScanInterval { seconds: u32 },
    SetEventTracking { enabled: bool, keywords: Vec<String> },
    ExportLogsJsonl,
    StartMacroRecording {
        #[serde(rename = "windowIndex")]
        window_index: usize,
    },
    StopMacroRecording,
    TestMacro {
        #[serde(rename = "windowIndex")]
        window_index: usize,
    },
    StopMacroPlayback,
    ClearMacro,
    SetMacroEnabled { enabled: bool },
}

impl UiCommand {
    /// Defensively parses and validates an incoming JSON command from the WebView.
    pub fn parse_and_validate(raw_json: &str) -> Result<Self, String> {
        if raw_json.len() > 64 * 1024 {
            return Err("IPC payload exceeds maximum allowed size.".to_string());
        }
        let mut cmd: UiCommand =
            serde_json::from_str(raw_json).map_err(|e| format!("Invalid command payload: {}", e))?;
        match cmd {
            UiCommand::SaveConfig { ref mut config } => {
                config.validate()?;
            }
            UiCommand::SetAntiAfkInterval { minutes } => {
                if !(1..=19).contains(&minutes) {
                    return Err("Anti-AFK interval must be between 1 and 19 minutes.".to_string());
                }
            }
            UiCommand::SetScanInterval { seconds } => {
                if !(1..=60).contains(&seconds) {
                    return Err("Scan interval must be between 1 and 60 seconds.".to_string());
                }
            }
            _ => {}
        }
        Ok(cmd)
    }
}

/// Typed events sent from Rust backend/workers to the WebView UI via EventLoopProxy.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum UiEvent {
    TelemetrySnapshot(UIStateSnapshot),
    ConfigLoaded(Config),
    ConfigSaved(Config),
    LogsJsonlExported { filename: String, content: String },
    ActionError { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_command_defensive_parsing() {
        let valid = r#"{"type": "startAutomator"}"#;
        assert_eq!(
            UiCommand::parse_and_validate(valid).unwrap(),
            UiCommand::StartAutomator
        );

        let rec = r#"{"type": "startMacroRecording", "windowIndex": 1}"#;
        assert_eq!(
            UiCommand::parse_and_validate(rec).unwrap(),
            UiCommand::StartMacroRecording { window_index: 1 }
        );

        let test_m = r#"{"type": "testMacro", "windowIndex": 0}"#;
        assert_eq!(
            UiCommand::parse_and_validate(test_m).unwrap(),
            UiCommand::TestMacro { window_index: 0 }
        );

        let unknown = r#"{"type": "execShell", "cmd": "whoami"}"#;
        assert!(UiCommand::parse_and_validate(unknown).is_err());

        let invalid_cfg = r#"{"type": "saveConfig", "config": {"checkIntervalSeconds": 0.01}}"#;
        assert!(UiCommand::parse_and_validate(invalid_cfg).is_err());

        let oversized = "a".repeat(70 * 1024);
        assert!(UiCommand::parse_and_validate(&oversized).is_err());
    }

    #[test]
    fn test_ui_event_serialization() {
        let ev = UiEvent::ActionError {
            message: "Invalid setting".to_string(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"type\":\"actionError\""));
        assert!(json.contains("Invalid setting"));
    }
}
