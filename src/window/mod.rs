use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetClientRect, GetForegroundWindow, GetWindowRect,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE, SW_SHOW,
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

                if cw >= 200 && ch >= 200 && ww >= 200 && wh >= 200 {
                    let len = GetWindowTextLengthW(hwnd);
                    let title = if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        GetWindowTextW(hwnd, &mut buf);
                        String::from_utf16_lossy(&buf[..len as usize])
                    } else {
                        ctx.fallback_title.to_string()
                    };

                    // Stable instance ID combining PID and HWND handle value (never exposed as raw HWND to JS)
                    let instance_id = ((pid as u64) << 32) ^ (hwnd.0 as usize as u64);

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
}
