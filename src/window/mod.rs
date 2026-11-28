use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClientRect, GetForegroundWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SetCursorPos, SetForegroundWindow, SetWindowPos, ShowWindow, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOZORDER, SW_RESTORE, SW_SHOW,
};

use crate::models::InstanceId;

#[derive(Debug, Clone)]
pub struct RobloxWindowInfo {
    pub instance_id: InstanceId,
    pub hwnd: HWND,
    pub pid: u32,
    pub title: String,
    pub window_left: i32,
    pub window_top: i32,
    pub window_width: i32,
    pub window_height: i32,
}

unsafe impl Send for RobloxWindowInfo {}
unsafe impl Sync for RobloxWindowInfo {}

pub trait WindowDiscovery: Send + Sync {
    fn discover_windows(&self, process_names: &[String], fallback_title: &str) -> Vec<RobloxWindowInfo>;
    fn is_window_alive(&self, hwnd: HWND) -> bool;
    fn activate_window(&self, hwnd: HWND) -> Result<(), String>;
    fn resize_window_in_place(&self, _hwnd: HWND, _width: i32, _height: i32) -> Result<(), String> {
        Ok(())
    }
}

pub struct Win32WindowService;

impl Win32WindowService {
    pub fn find_pids_by_names(process_names: &[String]) -> Vec<u32> {
        let mut pids = Vec::new();
        if process_names.is_empty() {
            return pids;
        }
        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        let name_len = entry
                            .szExeFile
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);
                        if process_names.iter().any(|p| name.eq_ignore_ascii_case(p)) {
                            pids.push(entry.th32ProcessID);
                        }
                        if Process32NextW(snapshot, &mut entry).is_err() {
                            break;
                        }
                    }
                }
            }
        }
        pids
    }

    pub fn restore_if_minimized(hwnd: HWND) {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
        }
    }

    pub fn center_cursor_in_window(hwnd: HWND) -> Result<(), String> {
        unsafe {
            let mut client = RECT::default();
            GetClientRect(hwnd, &mut client)
                .map_err(|e| format!("GetClientRect failed: {}", e))?;
            let mut pt = POINT {
                x: (client.right - client.left) / 2,
                y: (client.bottom - client.top) / 2,
            };
            if !ClientToScreen(hwnd, &mut pt).as_bool() {
                return Err("ClientToScreen failed".to_string());
            }
            SetCursorPos(pt.x, pt.y)
                .map_err(|e| format!("SetCursorPos failed: {}", e))?;
            Ok(())
        }
    }
}

impl WindowDiscovery for Win32WindowService {
    fn discover_windows(&self, process_names: &[String], fallback_title: &str) -> Vec<RobloxWindowInfo> {
        let pids = Self::find_pids_by_names(process_names);
        if pids.is_empty() {
            return Vec::new();
        }

        struct EnumContext<'a> {
            pids: Vec<u32>,
            fallback_title: &'a str,
            windows: Vec<RobloxWindowInfo>,
        }

        let mut context = EnumContext {
            pids,
            fallback_title,
            windows: Vec::new(),
        };

        unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
            let ctx = &mut *(lparam.0 as *mut EnumContext);
            if !IsWindowVisible(hwnd).as_bool() {
                return BOOL(1);
            }

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));

            if ctx.pids.contains(&pid) {
                let mut win_rect = RECT::default();
                let mut client_rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut win_rect);
                let _ = GetClientRect(hwnd, &mut client_rect);

                let cw = client_rect.right - client_rect.left;
                let ch = client_rect.bottom - client_rect.top;
                let ww = win_rect.right - win_rect.left;
                let wh = win_rect.bottom - win_rect.top;

                if is_discoverable_rect(cw, ch, ww, wh) {
                    let len = GetWindowTextLengthW(hwnd);
                    let title = if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        GetWindowTextW(hwnd, &mut buf);
                        String::from_utf16_lossy(&buf[..len as usize])
                    } else {
                        ctx.fallback_title.to_string()
                    };

                    // Stable instance ID combining PID and HWND handle value (never exposed as raw HWND to JS)
                    let instance_id = compute_instance_id(pid, hwnd.0 as usize);

                    ctx.windows.push(RobloxWindowInfo {
                        instance_id,
                        hwnd,
                        pid,
                        title,
                        window_left: win_rect.left,
                        window_top: win_rect.top,
                        window_width: ww,
                        window_height: wh,
                    });
                }
            }
            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(Some(enum_window_proc), LPARAM(&mut context as *mut _ as isize));
        }

        context.windows.sort_by_key(|w| w.instance_id);
        context.windows
    }

    fn is_window_alive(&self, hwnd: HWND) -> bool {
        unsafe { IsWindow(hwnd).as_bool() }
    }

    fn activate_window(&self, hwnd: HWND) -> Result<(), String> {
        if !self.is_window_alive(hwnd) {
            return Err("Target window no longer exists".to_string());
        }
        Self::restore_if_minimized(hwnd);
        unsafe {
            if GetForegroundWindow() == hwnd {
                return Ok(());
            }
            let fg_hwnd = GetForegroundWindow();
            let fg_tid = if !fg_hwnd.0.is_null() {
                GetWindowThreadProcessId(fg_hwnd, None)
            } else {
                0
            };
            let cur_tid = GetCurrentThreadId();
            let attached = fg_tid != 0 && fg_tid != cur_tid && AttachThreadInput(cur_tid, fg_tid, true).as_bool();

            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);

            if attached {
                let _ = AttachThreadInput(cur_tid, fg_tid, false);
            }
            std::thread::sleep(std::time::Duration::from_millis(120));
        }
        Ok(())
    }

    fn resize_window_in_place(&self, hwnd: HWND, width: i32, height: i32) -> Result<(), String> {
        if !self.is_window_alive(hwnd) {
            return Err("Target window no longer exists".to_string());
        }
        Self::restore_if_minimized(hwnd);
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .map_err(|e| format!("SetWindowPos failed: {}", e))?;
        }
        Ok(())
    }
}

pub fn is_discoverable_rect(cw: i32, ch: i32, ww: i32, wh: i32) -> bool {
    cw >= 200 && ch >= 200 && ww >= 200 && wh >= 200
}

pub fn compute_instance_id(pid: u32, hwnd_raw: usize) -> u64 {
    ((pid as u64) << 32) ^ (hwnd_raw as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_discovery_helpers() {
        assert!(is_discoverable_rect(800, 600, 816, 639));
        assert!(!is_discoverable_rect(199, 600, 800, 600));
        assert!(!is_discoverable_rect(800, 150, 800, 600));

        let id1 = compute_instance_id(1234, 0xABCD);
        let id2 = compute_instance_id(1234, 0xABCE);
        let id3 = compute_instance_id(1235, 0xABCD);
        assert_ne!(id1, id2);
        assert_ne!(id1, id3);
    }
}
