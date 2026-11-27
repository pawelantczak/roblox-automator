use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateFontW, CreatePen, CreateSolidBrush, DeleteObject, DrawTextW, Ellipse,
    FillRect, RoundRect, ScreenToClient, SelectObject, SetBkMode, SetTextColor, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, FW_BOLD, FW_SEMIBOLD, HBRUSH, HFONT, HPEN, PS_SOLID, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GetClientRect, GetForegroundWindow, PeekMessageW, PostThreadMessageW,
    RegisterClassW, SetLayeredWindowAttributes, SetWindowsHookExW, ShowWindow, TranslateMessage,
    UnhookWindowsHookEx, CS_HREDRAW, CS_VREDRAW, HHOOK, KBDLLHOOKSTRUCT, LLKHF_EXTENDED,
    LWA_ALPHA, LWA_COLORKEY, MSG, MSLLHOOKSTRUCT, PM_REMOVE, SW_SHOWNOACTIVATE, WH_KEYBOARD_LL,
    WH_MOUSE_LL,
    WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
    WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_QUIT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::models::{MacroStep, MouseBtn};
use crate::window::{Win32WindowService, WindowDiscovery};

struct RecorderState {
    target_hwnd: HWND,
    thread_id: u32,
    stop_flag: Arc<AtomicBool>,
    last_event_at: Instant,
    last_move_at: Instant,
    last_move_pt: Option<POINT>,
    steps: Vec<MacroStep>,
    kb_hook: HHOOK,
    ms_hook: HHOOK,
}

thread_local! {
    static REC_STATE: RefCell<Option<RecorderState>> = const { RefCell::new(None) };
    static OVERLAY_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

pub struct MacroRecorderHandle {
    stop_flag: Arc<AtomicBool>,
    countdown: Arc<AtomicU8>,
    thread_id: Arc<std::sync::atomic::AtomicU32>,
    join_handle: Option<JoinHandle<Vec<MacroStep>>>,
}

impl MacroRecorderHandle {
    pub fn is_finished(&self) -> bool {
        self.join_handle
            .as_ref()
            .map(|h| h.is_finished())
            .unwrap_or(true)
    }

    pub fn countdown_secs(&self) -> Option<u8> {
        let v = self.countdown.load(Ordering::Relaxed);
        if v == 0 {
            None
        } else {
            Some(v)
        }
    }

    pub fn request_stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        let tid = self.thread_id.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }

    pub fn join(mut self) -> Vec<MacroStep> {
        self.request_stop();
        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap_or_default()
        } else {
            Vec::new()
        }
    }
}

impl Drop for MacroRecorderHandle {
    fn drop(&mut self) {
        self.request_stop();
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn start_macro_recording(
    target_hwnd: HWND,
    countdown_seconds: u8,
) -> Result<MacroRecorderHandle, String> {
    let hwnd_raw = target_hwnd.0 as isize;
    let stop_flag = Arc::new(AtomicBool::new(false));
    let countdown = Arc::new(AtomicU8::new(countdown_seconds));
    let thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let stop_c = Arc::clone(&stop_flag);
    let cd_c = Arc::clone(&countdown);
    let tid_c = Arc::clone(&thread_id);

    let handle = thread::spawn(move || {
        let hwnd = HWND(hwnd_raw as *mut std::ffi::c_void);
        let tid = unsafe { GetCurrentThreadId() };
        tid_c.store(tid, Ordering::SeqCst);

        // 1. Focus target Roblox window & center cursor
        let win_svc = Win32WindowService;
        if win_svc.activate_window(hwnd).is_err() {
            cd_c.store(0, Ordering::SeqCst);
            return Vec::new();
        }
        let _ = Win32WindowService::center_cursor_in_window(hwnd);

        // 2. Show non-activating click-through 3..2..1 countdown overlay over target window
        if countdown_seconds > 0 {
            run_countdown_overlay(hwnd, countdown_seconds, &stop_c, &cd_c);
        }
        cd_c.store(0, Ordering::SeqCst);

        if stop_c.load(Ordering::SeqCst) {
            return Vec::new();
        }

        // Re-focus and re-center right as recording begins
        let _ = win_svc.activate_window(hwnd);
        let _ = Win32WindowService::center_cursor_in_window(hwnd);

        // 3. Install low-level keyboard & mouse hooks
        unsafe {
            let kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), None, 0);
            let ms_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(ll_mouse_proc), None, 0);

            let (kb_h, ms_h) = match (kb_hook, ms_hook) {
                (Ok(kb), Ok(ms)) => (kb, ms),
                (kb_res, ms_res) => {
                    if let Ok(h) = kb_res {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    if let Ok(h) = ms_res {
                        let _ = UnhookWindowsHookEx(h);
                    }
                    return Vec::new();
                }
            };

            let now = Instant::now();
            REC_STATE.with(|cell| {
                *cell.borrow_mut() = Some(RecorderState {
                    target_hwnd: hwnd,
                    thread_id: tid,
                    stop_flag: Arc::clone(&stop_c),
                    last_event_at: now,
                    last_move_at: now,
                    last_move_pt: None,
                    steps: Vec::new(),
                    kb_hook: kb_h,
                    ms_hook: ms_h,
                });
            });

            let mut msg = MSG::default();
            while !stop_c.load(Ordering::SeqCst) {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        stop_c.store(true, Ordering::SeqCst);
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                if stop_c.load(Ordering::SeqCst) {
                    break;
                }
                thread::sleep(Duration::from_millis(8));
            }

            let steps = REC_STATE.with(|cell| {
                if let Some(st) = cell.borrow_mut().take() {
                    let _ = UnhookWindowsHookEx(st.kb_hook);
                    let _ = UnhookWindowsHookEx(st.ms_hook);
                    st.steps
                } else {
                    Vec::new()
                }
            });

            steps
        }
    });

    Ok(MacroRecorderHandle {
        stop_flag,
        countdown,
        thread_id,
        join_handle: Some(handle),
    })
}

fn run_countdown_overlay(
    target_hwnd: HWND,
    seconds: u8,
    stop_flag: &AtomicBool,
    countdown: &AtomicU8,
) {
    unsafe {
        let mut client = RECT::default();
        if GetClientRect(target_hwnd, &mut client).is_err() {
            return;
        }
        let mut top_left = POINT { x: 0, y: 0 };
        let _ = ClientToScreen(target_hwnd, &mut top_left);
        let cw = (client.right - client.left).max(100);
        let ch = (client.bottom - client.top).max(100);

        let box_w = 540;
        let box_h = 260;
        let ox = top_left.x + (cw - box_w) / 2;
        let oy = top_left.y + (ch - box_h) / 2;

        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("RobloxAutomatorCountdownOverlay");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let overlay_hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!(""),
            WS_POPUP,
            ox,
            oy,
            box_w,
            box_h,
            None,
            None,
            hinstance,
            None,
        );

        let Ok(ov) = overlay_hwnd else {
            return;
        };

        let _ = SetLayeredWindowAttributes(ov, COLORREF(0x00FF00FF), 230, LWA_COLORKEY | LWA_ALPHA);
        let _ = ShowWindow(ov, SW_SHOWNOACTIVATE);

        for digit in (1..=seconds).rev() {
            if stop_flag.load(Ordering::SeqCst) {
                break;
            }
            countdown.store(digit, Ordering::SeqCst);
            OVERLAY_TEXT.with(|t| *t.borrow_mut() = digit.to_string());
            let _ = windows::Win32::Graphics::Gdi::InvalidateRect(ov, None, true);

            let step_end = Instant::now() + Duration::from_millis(1500);
            let mut msg = MSG::default();
            while Instant::now() < step_end && !stop_flag.load(Ordering::SeqCst) {
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                thread::sleep(Duration::from_millis(16));
            }
        }

        let _ = DestroyWindow(ov);
    }
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        let mut ps = windows::Win32::Graphics::Gdi::PAINTSTRUCT::default();
        let hdc = windows::Win32::Graphics::Gdi::BeginPaint(hwnd, &mut ps);
        let mut rect = RECT::default();
        let _ = GetClientRect(hwnd, &mut rect);

        // Transparent colorkey background
        let key_brush: HBRUSH = CreateSolidBrush(COLORREF(0x00FF00FF));
        FillRect(hdc, &rect, key_brush);
        let _ = DeleteObject(key_brush);

        let dark_brush: HBRUSH = CreateSolidBrush(COLORREF(0x00170F0B));
        let cyan_pen: HPEN = CreatePen(PS_SOLID, 3, COLORREF(0x00F8BD38));
        let old_brush = SelectObject(hdc, dark_brush);
        let old_pen = SelectObject(hdc, cyan_pen);

        // Circular countdown badge centered in top portion
        let _ = Ellipse(hdc, 190, 8, 350, 168);

        // Rounded hint pill below the circle
        let border_pen: HPEN = CreatePen(PS_SOLID, 2, COLORREF(0x00554133));
        SelectObject(hdc, border_pen);
        let _ = RoundRect(hdc, 10, 184, 530, 244, 18, 18);

        SelectObject(hdc, old_pen);
        SelectObject(hdc, old_brush);
        let _ = DeleteObject(cyan_pen);
        let _ = DeleteObject(border_pen);
        let _ = DeleteObject(dark_brush);

        SetBkMode(hdc, TRANSPARENT);

        // Countdown digit inside circle
        let digit_font: HFONT = CreateFontW(
            110,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            w!("Segoe UI"),
        );
        let old_font = SelectObject(hdc, digit_font);
        SetTextColor(hdc, COLORREF(0x00F8BD38));
        let mut circle_rect = RECT {
            left: 190,
            top: 8,
            right: 350,
            bottom: 168,
        };
        let text_str = OVERLAY_TEXT.with(|t| t.borrow().clone());
        let mut wide: Vec<u16> = text_str.encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut wide,
            &mut circle_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        // Hint text below circle
        let hint_font: HFONT = CreateFontW(
            26,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            w!("Segoe UI"),
        );
        SelectObject(hdc, hint_font);
        SetTextColor(hdc, COLORREF(0x00F9F5F1));
        let mut hint_rect = RECT {
            left: 10,
            top: 184,
            right: 530,
            bottom: 244,
        };
        let mut hint_wide: Vec<u16> = "Move mouse outside window to stop".encode_utf16().collect();
        DrawTextW(
            hdc,
            &mut hint_wide,
            &mut hint_rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        SelectObject(hdc, old_font);
        let _ = DeleteObject(digit_font);
        let _ = DeleteObject(hint_font);
        let _ = windows::Win32::Graphics::Gdi::EndPaint(hwnd, &ps);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn normalize_client_point(hwnd: HWND, screen_pt: POINT) -> Option<(f32, f32)> {
    unsafe {
        let mut client = RECT::default();
        GetClientRect(hwnd, &mut client).ok()?;
        let cw = client.right - client.left;
        let ch = client.bottom - client.top;
        if cw <= 0 || ch <= 0 {
            return None;
        }
        let mut pt = screen_pt;
        if !ScreenToClient(hwnd, &mut pt).as_bool() {
            return None;
        }
        if pt.x < 0 || pt.y < 0 || pt.x >= cw || pt.y >= ch {
            return None;
        }
        Some((
            (pt.x as f32 / cw as f32).clamp(0.0, 1.0),
            (pt.y as f32 / ch as f32).clamp(0.0, 1.0),
        ))
    }
}

unsafe extern "system" fn ll_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let msg = wparam.0 as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        if is_down || is_up {
            REC_STATE.with(|cell| {
                if let Some(ref mut st) = *cell.borrow_mut() {
                    if !st.stop_flag.load(Ordering::Relaxed)
                        && GetForegroundWindow() == st.target_hwnd
                        && st.steps.len() < 5000
                    {
                        let now = Instant::now();
                        let delay_ms = now.duration_since(st.last_event_at).as_millis() as u64;
                        st.last_event_at = now;
                        st.steps.push(MacroStep::Key {
                            vk: kb.vkCode as u16,
                            scan_code: kb.scanCode as u16,
                            extended: (kb.flags.0 & LLKHF_EXTENDED.0) != 0,
                            down: is_down,
                            delay_ms,
                        });
                    }
                }
            });
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn ll_mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let msg = wparam.0 as u32;

        REC_STATE.with(|cell| {
            if let Some(ref mut st) = *cell.borrow_mut() {
                if st.stop_flag.load(Ordering::Relaxed) {
                    return;
                }

                // Auto-stop recording the moment the cursor leaves the target Roblox window
                let Some((nx, ny)) = normalize_client_point(st.target_hwnd, ms.pt) else {
                    st.stop_flag.store(true, Ordering::SeqCst);
                    let _ = PostThreadMessageW(st.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
                    return;
                };

                if GetForegroundWindow() != st.target_hwnd || st.steps.len() >= 5000 {
                    return;
                }

                let now = Instant::now();
                match msg {
                    WM_MOUSEMOVE => {
                        let elapsed_move = now.duration_since(st.last_move_at).as_millis();
                        let moved_enough = st
                            .last_move_pt
                            .map(|p| (p.x - ms.pt.x).abs() >= 4 || (p.y - ms.pt.y).abs() >= 4)
                            .unwrap_or(true);
                        if elapsed_move >= 25 && moved_enough {
                            let delay_ms = now.duration_since(st.last_event_at).as_millis() as u64;
                            st.last_event_at = now;
                            st.last_move_at = now;
                            st.last_move_pt = Some(ms.pt);
                            st.steps.push(MacroStep::MouseMove { nx, ny, delay_ms });
                        }
                    }
                    WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP
                    | WM_MBUTTONDOWN | WM_MBUTTONUP => {
                        let (button, down) = match msg {
                            WM_LBUTTONDOWN => (MouseBtn::Left, true),
                            WM_LBUTTONUP => (MouseBtn::Left, false),
                            WM_RBUTTONDOWN => (MouseBtn::Right, true),
                            WM_RBUTTONUP => (MouseBtn::Right, false),
                            WM_MBUTTONDOWN => (MouseBtn::Middle, true),
                            _ => (MouseBtn::Middle, false),
                        };
                        let delay_ms = now.duration_since(st.last_event_at).as_millis() as u64;
                        st.last_event_at = now;
                        st.steps.push(MacroStep::MouseButton {
                            button,
                            down,
                            nx,
                            ny,
                            delay_ms,
                        });
                    }
                    WM_MOUSEWHEEL => {
                        let delta = ((ms.mouseData >> 16) & 0xFFFF) as i16;
                        let delay_ms = now.duration_since(st.last_event_at).as_millis() as u64;
                        st.last_event_at = now;
                        st.steps.push(MacroStep::MouseWheel {
                            delta,
                            nx,
                            ny,
                            delay_ms,
                        });
                    }
                    _ => {}
                }
            }
        });
    }
    CallNextHookEx(None, code, wparam, lparam)
}
