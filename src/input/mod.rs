pub mod recorder;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{ClientToScreen, ScreenToClient};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC,
    MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetCursorPos, GetForegroundWindow, PostMessageW, SetCursorPos,
    SetForegroundWindow, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
};

use crate::models::{FailureReason, MacroStep, MouseBtn};
use crate::window::{Win32WindowService, WindowDiscovery};

static INPUT_LOCK: Mutex<()> = Mutex::new(());

pub trait InputAutomation: Send + Sync {
    fn click_at(&self, hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(), FailureReason>;
    fn anti_afk_jump(&self, hwnd: HWND) -> Result<(), FailureReason>;
    fn play_macro(
        &self,
        _hwnd: HWND,
        _steps: &[MacroStep],
        _stop_flag: &AtomicBool,
    ) -> Result<(), FailureReason> {
        Ok(())
    }
}

/// RAII guard that always restores cursor position and previous foreground window,
/// even if an automation step returns early or panics.
struct ForegroundAndCursorGuard {
    target_hwnd: HWND,
    saved_cursor: POINT,
    prev_foreground: HWND,
}

impl ForegroundAndCursorGuard {
    fn capture(target_hwnd: HWND) -> Self {
        unsafe {
            let mut saved_cursor = POINT::default();
            let _ = GetCursorPos(&mut saved_cursor);
            let prev_foreground = GetForegroundWindow();
            Self {
                target_hwnd,
                saved_cursor,
                prev_foreground,
            }
        }
    }
}

impl Drop for ForegroundAndCursorGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = SetCursorPos(self.saved_cursor.x, self.saved_cursor.y);
            if self.prev_foreground != self.target_hwnd && !self.prev_foreground.0.is_null() {
                let _ = SetForegroundWindow(self.prev_foreground);
            }
        }
    }
}

pub struct Win32InputService;

impl Win32InputService {
    fn focus_window(hwnd: HWND, timeout_ms: u128) -> Result<(), FailureReason> {
        let win_svc = Win32WindowService;
        win_svc
            .activate_window(hwnd)
            .map_err(FailureReason::FocusFailed)?;

        let start = std::time::Instant::now();
        unsafe {
            while GetForegroundWindow() != hwnd && start.elapsed().as_millis() < timeout_ms {
                std::thread::sleep(Duration::from_millis(20));
            }
            if GetForegroundWindow() != hwnd {
                return Err(FailureReason::FocusFailed(
                    "Timed out waiting for window foreground focus".to_string(),
                ));
            }
        }
        Ok(())
    }

    unsafe fn move_mouse_to(screen_x: i32, screen_y: i32) {
        let _ = SetCursorPos(screen_x, screen_y);

        // Nudge by +1/-1 relative pixel via SendInput so Roblox UI hover/RawInput registers at exact pixel
        let mut nudge_right = [INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 1,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }];
        SendInput(&mut nudge_right, std::mem::size_of::<INPUT>() as i32);
        std::thread::sleep(Duration::from_millis(15));

        let mut nudge_left = [INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: -1,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }];
        SendInput(&mut nudge_left, std::mem::size_of::<INPUT>() as i32);
        let _ = SetCursorPos(screen_x, screen_y);
    }

    unsafe fn send_mouse_click() -> Result<(), FailureReason> {
        let mut down_input = [INPUT {
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
        }];
        if SendInput(&mut down_input, std::mem::size_of::<INPUT>() as i32) == 0 {
            return Err(FailureReason::InputFailed(
                "SendInput mouse down failed (0 events dispatched)".to_string(),
            ));
        }

        // Hold button down across at least 2 game frames so Roblox UI registers the press
        std::thread::sleep(Duration::from_millis(65));

        let mut up_input = [INPUT {
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
        }];
        if SendInput(&mut up_input, std::mem::size_of::<INPUT>() as i32) == 0 {
            return Err(FailureReason::InputFailed(
                "SendInput mouse up failed (0 events dispatched)".to_string(),
            ));
        }
        Ok(())
    }

    unsafe fn send_hardware_key(scancode: u16, key_up: bool) -> Result<(), FailureReason> {
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
        let sent = SendInput(&mut input, std::mem::size_of::<INPUT>() as i32);
        if sent == 0 {
            return Err(FailureReason::InputFailed(
                "SendInput keyboard event failed (0 events dispatched)".to_string(),
            ));
        }
        Ok(())
    }
}

impl InputAutomation for Win32InputService {
    fn play_macro(
        &self,
        hwnd: HWND,
        steps: &[MacroStep],
        stop_flag: &AtomicBool,
    ) -> Result<(), FailureReason> {
        if steps.is_empty() || stop_flag.load(Ordering::Relaxed) {
            return Ok(());
        }
        let _guard = INPUT_LOCK.lock().unwrap();
        let _restore = ForegroundAndCursorGuard::capture(hwnd);
        Self::focus_window(hwnd, 450)?;

        unsafe {
            let mut client = RECT::default();
            if GetClientRect(hwnd, &mut client).is_err() {
                return Err(FailureReason::InputFailed(
                    "GetClientRect failed for macro playback".to_string(),
                ));
            }
            let cw = (client.right - client.left).max(1);
            let ch = (client.bottom - client.top).max(1);

            let to_screen = |nx: f32, ny: f32| -> POINT {
                let mut pt = POINT {
                    x: (nx.clamp(0.0, 1.0) * cw as f32).round() as i32,
                    y: (ny.clamp(0.0, 1.0) * ch as f32).round() as i32,
                };
                let _ = ClientToScreen(hwnd, &mut pt);
                pt
            };

            for step in steps {
                if stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                let mut remaining = step.delay_ms().min(10_000);
                while remaining > 0 {
                    if stop_flag.load(Ordering::Relaxed) {
                        return Ok(());
                    }
                    let slice = remaining.min(40);
                    std::thread::sleep(Duration::from_millis(slice));
                    remaining -= slice;
                }

                match step {
                    MacroStep::Key {
                        vk,
                        scan_code,
                        extended,
                        down,
                        ..
                    } => {
                        let sc = if *scan_code != 0 {
                            *scan_code
                        } else {
                            MapVirtualKeyW(*vk as u32, MAPVK_VK_TO_VSC) as u16
                        };
                        let mut flags = KEYEVENTF_SCANCODE;
                        if *extended {
                            flags |= KEYEVENTF_EXTENDEDKEY;
                        }
                        if !*down {
                            flags |= KEYEVENTF_KEYUP;
                        }
                        let mut input = [INPUT {
                            r#type: INPUT_KEYBOARD,
                            Anonymous: INPUT_0 {
                                ki: KEYBDINPUT {
                                    wVk: Default::default(),
                                    wScan: sc,
                                    dwFlags: flags,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        }];
                        let _ = SendInput(&mut input, std::mem::size_of::<INPUT>() as i32);
                        let msg = if *down { WM_KEYDOWN } else { WM_KEYUP };
                        let _ = PostMessageW(hwnd, msg, WPARAM(*vk as usize), LPARAM(0));
                    }
                    MacroStep::MouseMove { nx, ny, .. } => {
                        let pt = to_screen(*nx, *ny);
                        let _ = SetCursorPos(pt.x, pt.y);
                        let mut c_pt = pt;
                        let _ = ScreenToClient(hwnd, &mut c_pt);
                        let lp = LPARAM(pack_client_lparam(c_pt.x, c_pt.y));
                        let _ = PostMessageW(hwnd, WM_MOUSEMOVE, WPARAM(0), lp);
                    }
                    MacroStep::MouseButton {
                        button,
                        down,
                        nx,
                        ny,
                        ..
                    } => {
                        let pt = to_screen(*nx, *ny);
                        let _ = SetCursorPos(pt.x, pt.y);
                        std::thread::sleep(Duration::from_millis(12));
                        let flag = match (button, down) {
                            (MouseBtn::Left, true) => MOUSEEVENTF_LEFTDOWN,
                            (MouseBtn::Left, false) => MOUSEEVENTF_LEFTUP,
                            (MouseBtn::Right, true) => MOUSEEVENTF_RIGHTDOWN,
                            (MouseBtn::Right, false) => MOUSEEVENTF_RIGHTUP,
                            (MouseBtn::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
                            (MouseBtn::Middle, false) => MOUSEEVENTF_MIDDLEUP,
                        };
                        let mut input = [INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: 0,
                                    dwFlags: flag,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        }];
                        let _ = SendInput(&mut input, std::mem::size_of::<INPUT>() as i32);
                    }
                    MacroStep::MouseWheel { delta, nx, ny, .. } => {
                        let pt = to_screen(*nx, *ny);
                        let _ = SetCursorPos(pt.x, pt.y);
                        let mut input = [INPUT {
                            r#type: INPUT_MOUSE,
                            Anonymous: INPUT_0 {
                                mi: MOUSEINPUT {
                                    dx: 0,
                                    dy: 0,
                                    mouseData: (*delta as i32) as u32,
                                    dwFlags: MOUSEEVENTF_WHEEL,
                                    time: 0,
                                    dwExtraInfo: 0,
                                },
                            },
                        }];
                        let _ = SendInput(&mut input, std::mem::size_of::<INPUT>() as i32);
                    }
                }
            }
        }

        Ok(())
    }

    fn click_at(&self, hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(), FailureReason> {
        let _guard = INPUT_LOCK.lock().unwrap();
        let _restore = ForegroundAndCursorGuard::capture(hwnd);

        Self::focus_window(hwnd, 300)?;

        unsafe {
            let mut client_pt = POINT {
                x: screen_x,
                y: screen_y,
            };
            let _ = ScreenToClient(hwnd, &mut client_pt);
            let client_lparam = LPARAM(pack_client_lparam(client_pt.x, client_pt.y));

            Self::move_mouse_to(screen_x, screen_y);
            let _ = PostMessageW(hwnd, WM_MOUSEMOVE, WPARAM(0), client_lparam);
            std::thread::sleep(Duration::from_millis(90));

            Self::send_mouse_click()?;
            std::thread::sleep(Duration::from_millis(140));

            Self::send_mouse_click()?;
            let _ = PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), client_lparam);
            std::thread::sleep(Duration::from_millis(50));
            let _ = PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), client_lparam);
            std::thread::sleep(Duration::from_millis(120));
        }
        Ok(())
    }

    fn anti_afk_jump(&self, hwnd: HWND) -> Result<(), FailureReason> {
        let _guard = INPUT_LOCK.lock().unwrap();

        unsafe {
            let mut client_rect = windows::Win32::Foundation::RECT::default();
            if GetClientRect(hwnd, &mut client_rect).is_err() {
                return Err(FailureReason::InputFailed(
                    "GetClientRect failed for anti-AFK jump".to_string(),
                ));
            }

            let width = client_rect.right - client_rect.left;
            let height = client_rect.bottom - client_rect.top;
            if width <= 0 || height <= 0 {
                return Err(FailureReason::InputFailed(
                    "Invalid window dimensions for anti-AFK jump".to_string(),
                ));
            }

            let mut pt = POINT {
                x: width / 2,
                y: (height as f32 * 0.45) as i32,
            };
            let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut pt);

            let _restore = ForegroundAndCursorGuard::capture(hwnd);
            Self::focus_window(hwnd, 350)?;

            Self::move_mouse_to(pt.x, pt.y);
            std::thread::sleep(Duration::from_millis(100));

            Self::send_mouse_click()?;
            std::thread::sleep(Duration::from_millis(120));

            let scancode = MapVirtualKeyW(VK_SPACE.0 as u32, MAPVK_VK_TO_VSC) as u16;
            let scancode = if scancode == 0 { 0x39 } else { scancode };

            Self::send_hardware_key(scancode, false)?;
            let _ = PostMessageW(
                hwnd,
                WM_KEYDOWN,
                WPARAM(VK_SPACE.0 as usize),
                LPARAM(0x00390001),
            );

            std::thread::sleep(Duration::from_millis(140));

            Self::send_hardware_key(scancode, true)?;
            let _ = PostMessageW(
                hwnd,
                WM_KEYUP,
                WPARAM(VK_SPACE.0 as usize),
                LPARAM(0xC0390001),
            );

            std::thread::sleep(Duration::from_millis(150));
        }

        Ok(())
    }
}

pub fn pack_client_lparam(cx: i32, cy: i32) -> isize {
    (((cy as isize) & 0xFFFF) << 16) | ((cx as isize) & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_lparam_helper() {
        assert_eq!(pack_client_lparam(400, 300), (300isize << 16) | 400isize);
    }
}
