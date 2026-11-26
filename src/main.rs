#![windows_subsystem = "windows"]

mod capture;
mod engine;
mod input;
mod models;
mod ocr;
mod power;
mod storage;
mod ui;
mod window;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

use crate::storage::StorageService;

struct SingleInstanceGuard {
    handle: HANDLE,
}

impl SingleInstanceGuard {
    fn acquire() -> Option<Self> {
        unsafe {
            let handle =
                CreateMutexW(None, true, windows::core::w!("Global\\RobloxWatcher_Mutex"))
                    .ok()?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return None;
            }
            Some(Self { handle })
        }
    }
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn main() -> anyhow::Result<()> {
    // 0. Enforce Per-Monitor V2 DPI awareness so capture, ClientToScreen, and SetCursorPos share physical pixels
    unsafe {
        let _ = windows::Win32::UI::HiDpi::SetProcessDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        );
    }

    // 1. Runtime architecture & WebView2 availability check before launch
    if let Err(msg) = ui::webview::check_webview2_runtime() {
        ui::show_fatal_error_box("Roblox Watcher - Missing Runtime", &msg);
        return Ok(());
    }

    // 2. Single-instance guard via Win32 Named Mutex (released cleanly on drop)
    let Some(_instance_guard) = SingleInstanceGuard::acquire() else {
        return Ok(());
    };

    // 3. Load and migrate persisted configuration
    let config = StorageService::load_config();

    // 4. Launch main-thread UI & worker engine
    ui::run_app(config)
}
