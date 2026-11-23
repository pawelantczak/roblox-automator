use std::sync::Mutex;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEINPUT, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, GetForegroundWindow, PostMessageW, SetCursorPos,
    SetForegroundWindow, WM_KEYDOWN, WM_KEYUP,
};

use crate::window::WindowService;

static INPUT_LOCK: Mutex<()> = Mutex::new(());

pub struct InputService;

impl InputService {
    /// Dispatches an atomic two-click sequence to target coordinates on a specific window
    pub fn click_at(hwnd: HWND, screen_x: i32, screen_y: i32) {
        let _guard = INPUT_LOCK.lock().unwrap();

        unsafe {
            let mut saved_cursor = POINT::default();
            let _ = GetCursorPos(&mut saved_cursor);
            let prev_foreground = GetForegroundWindow();

            // 1. Activate window and wait until focused
            WindowService::activate_window(hwnd);
            let start = std::time::Instant::now();
            while GetForegroundWindow() != hwnd && start.elapsed().as_millis() < 300 {
                std::thread::sleep(Duration::from_millis(20));
            }

            // 2. Warp cursor to target
            let _ = SetCursorPos(screen_x, screen_y);
            std::thread::sleep(Duration::from_millis(150));

            // 3. Click 1: Activate client area
            Self::send_mouse_click();
            std::thread::sleep(Duration::from_millis(200));

            // 4. Click 2: Trigger in-game button
            Self::send_mouse_click();
            std::thread::sleep(Duration::from_millis(150));

            // 5. Restore previous cursor position
            let _ = SetCursorPos(saved_cursor.x, saved_cursor.y);
            if prev_foreground != hwnd && !prev_foreground.0.is_null() {
                let _ = SetForegroundWindow(prev_foreground);
            }
        }
    }

    /// Performs a bulletproof Focus + Hardware Scancode Spacebar Jump + Restore Focus sequence
    pub fn anti_afk_jump(hwnd: HWND) {
        let _guard = INPUT_LOCK.lock().unwrap();

        unsafe {
            let mut client_rect = windows::Win32::Foundation::RECT::default();
            if GetClientRect(hwnd, &mut client_rect).is_err() {
                return;
            }

            let width = client_rect.right - client_rect.left;
            let height = client_rect.bottom - client_rect.top;
            if width <= 0 || height <= 0 {
                return;
            }

            // Target safe upper-middle 3D viewport area (avoid bottom HUD icons)
            let mut pt = POINT {
                x: width / 2,
                y: (height as f32 * 0.45) as i32,
            };

            // Map client point to screen point
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut pt);

            // Add randomized jitter +/- 6px
            let jitter_x = (rand_simple() % 13) - 6;
            let jitter_y = (rand_simple() % 13) - 6;
            let target_x = pt.x + jitter_x;
            let target_y = pt.y + jitter_y;

            let mut saved_cursor = POINT::default();
            let _ = GetCursorPos(&mut saved_cursor);
            let prev_foreground = GetForegroundWindow();

            // 1. Activate window and wait until confirmed active
            WindowService::activate_window(hwnd);
            let start = std::time::Instant::now();
            while GetForegroundWindow() != hwnd && start.elapsed().as_millis() < 350 {
                std::thread::sleep(Duration::from_millis(20));
            }

            // 2. Warp cursor to viewport with jitter
            let _ = SetCursorPos(target_x, target_y);
            std::thread::sleep(Duration::from_millis(100));

            // 3. Gentle focus click to confirm 3D viewport keyboard focus
            Self::send_mouse_click();
            std::thread::sleep(Duration::from_millis(120));

            // 4. Send Hardware Scancode Spacebar Jump via SendInput (DirectX / DirectInput compliant)
            // Spacebar hardware scancode = 0x39 (57)
            let scancode = MapVirtualKeyW(VK_SPACE.0 as u32, MAPVK_VK_TO_VSC) as u16;
            let scancode = if scancode == 0 { 0x39 } else { scancode };

            // Dual dispatch: SendInput hardware scan code + PostMessage fallback
            Self::send_hardware_key(scancode, false);
            let _ = PostMessageW(hwnd, WM_KEYDOWN, WPARAM(VK_SPACE.0 as usize), LPARAM(0x00390001));

            // Hold key down for 140ms so the game engine physics step captures the jump
            std::thread::sleep(Duration::from_millis(140));

            Self::send_hardware_key(scancode, true);
            let _ = PostMessageW(hwnd, WM_KEYUP, WPARAM(VK_SPACE.0 as usize), LPARAM(0xC0390001));

            // Allow Roblox engine frame to process jump before restoring focus
            std::thread::sleep(Duration::from_millis(150));

            // 5. Restore previous hardware cursor position and foreground focus
            let _ = SetCursorPos(saved_cursor.x, saved_cursor.y);
            if prev_foreground != hwnd && !prev_foreground.0.is_null() {
                let _ = SetForegroundWindow(prev_foreground);
            }
        }
    }

    unsafe fn send_mouse_click() {
        let mut inputs = [
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTDOWN,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_LEFTUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];
        SendInput(&mut inputs, std::mem::size_of::<INPUT>() as i32);
    }

    /// Sends a hardware-level keyboard event using KEYEVENTF_SCANCODE for DirectX / RawInput games
    unsafe fn send_hardware_key(scancode: u16, key_up: bool) {
        let flags = if key_up {
            KEYEVENTF_SCANCODE | KEYEVENTF_KEYUP
        } else {
            KEYEVENTF_SCANCODE
        };

        let mut input = [INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(0),
                    wScan: scancode,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }];
        SendInput(&mut input, std::mem::size_of::<INPUT>() as i32);
    }
}

// Simple fast pseudo-random number generator for spatial jitter without heavy dependencies
fn rand_simple() -> i32 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (now % 1000) as i32
}
