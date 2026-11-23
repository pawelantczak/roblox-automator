use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClientRect, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow,
    SW_RESTORE, SW_SHOW,
};

#[derive(Debug, Clone)]
pub struct RobloxWindowInfo {
    pub hwnd: HWND,
    pub pid: u32,
    pub title: String,
    pub bounds: RECT,
    pub client_width: i32,
    pub client_height: i32,
}

unsafe impl Send for RobloxWindowInfo {}
unsafe impl Sync for RobloxWindowInfo {}

pub struct WindowService;

impl WindowService {
    /// Returns a list of all running Roblox process IDs
    pub fn find_roblox_pids() -> Vec<u32> {
        let mut pids = Vec::new();
        unsafe {
            if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                let mut entry = PROCESSENTRY32W::default();
                entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

                if Process32FirstW(snapshot, &mut entry).is_ok() {
                    loop {
                        let name_len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
                        let name = String::from_utf16_lossy(&entry.szExeFile[..name_len]);
                        if name.eq_ignore_ascii_case("RobloxPlayerBeta.exe") || name.eq_ignore_ascii_case("RobloxPlayer.exe") {
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

    /// Finds all visible top-level Roblox windows on the system
    pub fn find_roblox_windows() -> Vec<RobloxWindowInfo> {
        let pids = Self::find_roblox_pids();
        if pids.is_empty() {
            return Vec::new();
        }

        struct EnumContext {
            pids: Vec<u32>,
            windows: Vec<RobloxWindowInfo>,
        }

        let mut context = EnumContext {
            pids,
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
                let mut rect = RECT::default();
                let mut client_rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);
                let _ = GetClientRect(hwnd, &mut client_rect);

                let w = client_rect.right - client_rect.left;
                let h = client_rect.bottom - client_rect.top;

                // Only consider full game windows (ignore small crash dialogs/helpers)
                if w >= 200 && h >= 200 {
                    let len = GetWindowTextLengthW(hwnd);
                    let title = if len > 0 {
                        let mut buf = vec![0u16; (len + 1) as usize];
                        GetWindowTextW(hwnd, &mut buf);
                        String::from_utf16_lossy(&buf[..len as usize])
                    } else {
                        "Roblox".to_string()
                    };

                    ctx.windows.push(RobloxWindowInfo {
                        hwnd,
                        pid,
                        title,
                        bounds: rect,
                        client_width: w,
                        client_height: h,
                    });
                }
            }
            BOOL(1)
        }

        unsafe {
            let _ = EnumWindows(Some(enum_window_proc), LPARAM(&mut context as *mut _ as isize));
        }

        context.windows
    }

    /// Automatically restores a minimized Roblox window
    pub fn restore_if_minimized(hwnd: HWND) {
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    }

    /// Brings the Roblox window to the foreground
    pub fn activate_window(hwnd: HWND) {
        Self::restore_if_minimized(hwnd);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
}
