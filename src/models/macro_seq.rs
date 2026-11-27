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
