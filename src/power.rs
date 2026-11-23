use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

pub struct PowerService;

impl PowerService {
    /// Prevents Windows from turning off the display, dimming, or entering idle sleep
    pub fn enable_sleep_prevention() {
        unsafe {
            let _ = SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
        }
    }

    /// Restores default Windows idle power management timers
    pub fn disable_sleep_prevention() {
        unsafe {
            let _ = SetThreadExecutionState(ES_CONTINUOUS);
        }
    }
}
