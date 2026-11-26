pub mod config;
pub mod ipc;
pub mod state;
pub mod vision;

pub use config::Config;
#[cfg(test)]
pub use config::CURRENT_CONFIG_SCHEMA_VERSION;
pub use ipc::{UiCommand, UiEvent};
pub use state::{
    BoundedLogQueue, EngineMetrics, FailureReason, InstanceCard, InstanceId, InstanceState,
    LogLevel, UIStateSnapshot,
};
pub use vision::{CapturedFrame, DetectedText, DetectedWord, NormalizedPoint, NormalizedRect};
