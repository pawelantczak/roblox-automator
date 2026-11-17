# Specification & Agent Implementation Prompt: Roblox Automator (Windows)
# Multi-Instance, System Tray Daemon & ps99clan.xyz UI Architecture

> **Instructions for the AI Agent on Windows**:
> You are tasked with implementing the **Roblox Automator** on Windows.
> **Key Architecture Decisions**:
> - **Window Recovery Scope**: Strictly rely on existing open windows. Do **NOT** attempt external process relaunching.
> - **High-DPI Awareness**: Set Per-Monitor V2 DPI awareness to prevent coordinate drift across scaling (125%, 150%).
> - **Screen Saver / Sleep Prevention**: Use `SetThreadExecutionState` to prevent Windows from locking or sleeping.
> - **Anti-AFK Action**: Configurable between background `Spacebar` (jump) and `LeftClick`.
> - **Discord Webhook**: Simple incoming webhook HTTP POST (zero credentials / OAuth required).

---

## 1. System Tray & Window Lifecycle

1. **Minimize to Tray Behavior**:
   - When the user clicks the minimize button or the window close (`X`) button:
     - `window.set_visible(false)`
     - App continues running background anti-AFK, monitoring, and recovery.
2. **Tray Icon Near Clock (`tray-icon` crate)**:
   - Lives in the taskbar notification area.
   - Dynamic icon state:
     - **Cyan/Green icon**: Actively monitoring.
     - **Amber icon**: Reconnecting / recovering an account.
     - **Slate/Grey icon**: Paused.
3. **Tray Interactions**:
   - **Left Click**: Restores and focuses the dashboard window (`window.set_visible(true); window.set_focus()`).
   - **Right Click**: Context menu:
     - `Show Dashboard`
     - `Pause Monitoring` / `Resume Monitoring`
     - `---` (separator)
     - `Exit Automator` (graceful shutdown)

---

## 2. Core Functional Specifications

| Feature | Execution Mode | Mechanism | User Impact |
|---|---|---|---|
| **System Tray Hide** | Taskbar notification area near clock | `tray-icon` crate + `tao` window events | Keeps taskbar clean, runs in background |
| **Sleep / Lock Prevention** | OS Keep-Alive | Win32 `SetThreadExecutionState` | Prevents screensaver / lock screen |
| **High-DPI Scaling Guard** | Per-Monitor V2 DPI | `SetProcessDpiAwarenessContext` | Accurate click coordinates on 125%/150% displays |
| **Buried Anti-AFK** | Background Spacebar or Click every 5 min | `PostMessageW(hwnd, WM_KEYDOWN VK_SPACE)` | **Zero cursor movement**, windows stay buried |
| **Discord Notifications** | Incoming Webhook POST (JSON) | Direct HTTP request via `reqwest` | Instant pings on disconnect/recovery, 0 login |
| **"Ok!" Notice Dismiss** | Detect `"Ok!"` button after key actions and click | OCR search for `"Ok!"` -> `SendInput` | Dismisses blocking in-game dialogs |
| **Two-Stage Recovery** | Closed-loop Reconnect -> Event text-to-icon teleport | `SendInput` loop with OCR verification | Full recovery + instant re-bury |

---

## 3. Technology Stack: Rust (2021 Edition)

```toml
[package]
name = "roblox-automator"
version = "0.9.0"
edition = "2021"

[dependencies]
windows = { version = "0.58", features = [
    "Win32_Foundation",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_HiDpi",
    "Win32_Graphics_Gdi",
    "Win32_System_Threading",
    "Win32_System_Power",
    "Win32_System_ProcessStatus",
    "Win32_Security",
    "Win32_System_Diagnostics_ToolHelp",
    "Media_Ocr",
    "Graphics_Capture",
    "Graphics_DirectX_Direct3D11",
    "Storage_Streams"
] }

# Direct3D 11 Capture
windows-capture = "1.2"

# Native WebView2 GUI & System Tray
wry = "0.41"
tao = "0.28"
tray-icon = "0.14"

# HTTP Client for Discord Webhook
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }

# Concurrency & Config
tokio = { version = "1.38", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
chrono = "0.4"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## 4. Key Implementation Details

### 1. High-DPI & Sleep Prevention (`src/main.rs`)
```rust
use windows::Win32::UI::HiDpi::*;
use windows::Win32::System::Power::*;

pub fn initialize_os_environment() {
    unsafe {
        // Prevent coordinate drift on displays with 125%, 150%, or 200% scaling
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);

        // Keep system awake and prevent screen saver / lock screen from pausing DWM
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED);
    }
}
```

### 2. Configurable Anti-AFK (`src/input.rs`)
```rust
pub fn send_background_anti_afk(hwnd: HWND, mode: &str) {
    unsafe {
        match mode {
            "spacebar" => {
                // Post Jump key without moving cursor or stealing focus
                PostMessageW(hwnd, WM_KEYDOWN, WPARAM(0x20), LPARAM(0)); // VK_SPACE
                std::thread::sleep(Duration::from_millis(50));
                PostMessageW(hwnd, WM_KEYUP, WPARAM(0x20), LPARAM(0));
            }
            _ => {
                // Default: background center click
                let lparam = ((300 as u32) << 16) | (400 as u32 & 0xFFFF);
                PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(lparam as isize));
                std::thread::sleep(Duration::from_millis(50));
                PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
            }
        }
    }
}
```

### 3. Discord Webhook Notifications (`src/notifications.rs`)
No authentication or credentials required. The user creates a webhook in their Discord channel and pastes the URL in `config.toml`.
```rust
pub async fn send_discord_alert(webhook_url: &str, message: &str) {
    if webhook_url.is_empty() { return; }
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "username": "Roblox Automator",
        "avatar_url": "https://ps99clan.xyz/favicon.ico",
        "content": message
    });
    let _ = client.post(webhook_url).json(&payload).send().await;
}
```

---

## 5. Configuration (`config.toml`)
```toml
[general]
scan_interval_seconds = 5
anti_afk_interval_seconds = 300
anti_afk_mode = "spacebar"       # "spacebar" (jump) or "left_click"
prevent_system_sleep = true

[ui]
theme = "dark-cyan"
window_width = 1040
window_height = 720
close_to_tray = true
start_minimized_to_tray = false

[notifications]
# Paste your Discord channel incoming webhook URL here (optional)
discord_webhook_url = ""
notify_on_disconnect = true
notify_on_recovery = true

[reconnect_loop]
max_click_attempts = 15
retry_interval_ms = 1000
activation_delay_ms = 300

[event_teleport]
enabled = true
max_wait_seconds = 45
text_label = "Pumpkin"
click_offset_y = -40

[[actions.keys]]
key = "1"
virtual_key_code = 0x31
cooldown_seconds = 60
description = "Consumable Potion"

[[actions.keys]]
key = "E"
virtual_key_code = 0x45
cooldown_seconds = 0
description = "Interact / Ability (Every cycle)"

[actions.notice_dismiss]
auto_dismiss_enabled = true
target_button_text = "Ok!"
penalty_cooldown_seconds = 10

[vision]
reconnect_button_text = "Reconnect"
disconnect_phrases = [
    "disconnected",
    "lost connection",
    "error code",
    "server was shut down",
    "please reconnect"
]
```

---

## 6. Implementation Milestones for the Agent on Windows

1. **Step 1: Multi-HWND Scanner & OS Environment** (`src/instances.rs`, `src/main.rs`)
   - Initialize Per-Monitor V2 DPI awareness and `SetThreadExecutionState`.
   - Enumerate all running Roblox windows using `EnumWindows`.
2. **Step 2: Full-Window Buried Capture & Vision** (`src/vision.rs`)
   - Capture full frame via `windows-capture` and locate "Reconnect", event label, and "Ok!" button via WinRT OCR.
3. **Step 3: Two-Stage Recovery Pipeline** (`src/input.rs`)
   - Reconnect loop -> Event teleport -> Re-bury window to `HWND_BOTTOM`.
4. **Step 4: Buried Anti-AFK (Spacebar/Click) & Actions** (`src/input.rs`, `src/actions.rs`)
   - Test background spacebar/jump without cursor movement; handle key cooldowns and "Ok!" dismiss.
5. **Step 5: System Tray & Discord Webhook** (`src/tray.rs`, `src/notifications.rs`)
   - Implement tray icon near clock and HTTP webhook dispatch for alerts.
6. **Step 6: WebView2 Dark Dashboard & IPC** (`src/ui.rs`, `ui/index.html`)
   - Implement `wry` + `tao` window hosting the ps99clan.xyz dark cyberpunk dashboard.
7. **Step 7: Release Build**
   - Run `cargo build --release` to produce a single self-contained `roblox-automator.exe`.
