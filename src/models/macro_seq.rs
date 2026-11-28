use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MouseBtn {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MacroStep {
    Key {
        vk: u16,
        scan_code: u16,
        extended: bool,
        down: bool,
        delay_ms: u64,
    },
    MouseMove {
        nx: f32,
        ny: f32,
        delay_ms: u64,
    },
    MouseButton {
        button: MouseBtn,
        down: bool,
        nx: f32,
        ny: f32,
        delay_ms: u64,
    },
    MouseWheel {
        delta: i16,
        nx: f32,
        ny: f32,
        delay_ms: u64,
    },
}

impl MacroStep {
    pub fn delay_ms(&self) -> u64 {
        match self {
            MacroStep::Key { delay_ms, .. }
            | MacroStep::MouseMove { delay_ms, .. }
            | MacroStep::MouseButton { delay_ms, .. }
            | MacroStep::MouseWheel { delay_ms, .. } => *delay_ms,
        }
    }

    pub fn total_duration_ms(steps: &[MacroStep]) -> u64 {
        steps.iter().map(|s| s.delay_ms()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_step_serde_roundtrip_and_total_duration() {
        let steps = vec![
            MacroStep::Key {
                vk: 0x20,
                scan_code: 0x39,
                extended: false,
                down: true,
                delay_ms: 50,
            },
            MacroStep::MouseMove {
                nx: 0.25,
                ny: 0.75,
                delay_ms: 120,
            },
            MacroStep::MouseButton {
                button: MouseBtn::Right,
                down: true,
                nx: 0.5,
                ny: 0.5,
                delay_ms: 30,
            },
            MacroStep::MouseWheel {
                delta: -120,
                nx: 0.5,
                ny: 0.5,
                delay_ms: 100,
            },
        ];

        assert_eq!(MacroStep::total_duration_ms(&steps), 300);
        let json = serde_json::to_string(&steps).expect("serialize macro");
        let parsed: Vec<MacroStep> = serde_json::from_str(&json).expect("deserialize macro");
        assert_eq!(parsed, steps);
        assert_eq!(MacroStep::total_duration_ms(&[]), 0);
    }
}
