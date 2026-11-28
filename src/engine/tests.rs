use super::*;
use crate::models::{CapturedFrame, DetectedText, MacroStep, NormalizedPoint, NormalizedRect};
use std::sync::Mutex;
use windows::Win32::Foundation::HWND;

struct MockWindowSvc {
    windows: Vec<RobloxWindowInfo>,
}
impl WindowDiscovery for MockWindowSvc {
    fn discover_windows(
        &self,
        _process_names: &[String],
        _fallback_title: &str,
    ) -> Vec<RobloxWindowInfo> {
        self.windows.clone()
    }
    fn is_window_alive(&self, _hwnd: HWND) -> bool {
        true
    }
    fn activate_window(&self, _hwnd: HWND) -> Result<(), String> {
        Ok(())
    }
}

struct MockCaptureSvc {
    frame: CapturedFrame,
}
impl ScreenCapture for MockCaptureSvc {
    fn capture_window(&self, _hwnd: HWND) -> Result<CapturedFrame, String> {
        Ok(self.frame.clone())
    }
    fn encode_thumbnail_base64(
        &self,
        _frame: &CapturedFrame,
        _max_w: u32,
        _max_h: u32,
        _quality: u8,
    ) -> Option<String> {
        Some("mock_b64".to_string())
    }
    fn encode_point_crop_base64(
        &self,
        _frame: &CapturedFrame,
        _center: &NormalizedPoint,
        _radius_px: u32,
        _quality: u8,
    ) -> Option<String> {
        Some("mock_crop_b64".to_string())
    }
}

struct MockOcrSvc {
    items: Vec<DetectedText>,
    sidebar_items: Vec<DetectedText>,
    scan_calls: std::sync::atomic::AtomicUsize,
}
impl MockOcrSvc {
    fn with_items(items: Vec<DetectedText>) -> Self {
        Self {
            items,
            sidebar_items: Vec::new(),
            scan_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    fn with_sidebar(sidebar_items: Vec<DetectedText>) -> Self {
        Self {
            items: Vec::new(),
            sidebar_items,
            scan_calls: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
impl OcrProvider for MockOcrSvc {
    fn scan_frame(
        &self,
        _frame: &CapturedFrame,
        _timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        self.scan_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.items.clone())
    }
    fn scan_left_sidebar(
        &self,
        _frame: &CapturedFrame,
        _sidebar_ratio: f32,
        _timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        Ok(self.sidebar_items.clone())
    }
    fn scan_left_sidebar_high_contrast(
        &self,
        _frame: &CapturedFrame,
        _sidebar_ratio: f32,
        _timeout: Duration,
    ) -> Result<Vec<DetectedText>, String> {
        Ok(Vec::new())
    }
}

struct MockInputSvc {
    clicks: Mutex<Vec<(i32, i32)>>,
    jumps: std::sync::atomic::AtomicUsize,
    macro_plays: std::sync::atomic::AtomicUsize,
}
impl MockInputSvc {
    fn new() -> Self {
        Self {
            clicks: Mutex::new(Vec::new()),
            jumps: std::sync::atomic::AtomicUsize::new(0),
            macro_plays: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
impl InputAutomation for MockInputSvc {
    fn click_at(&self, _hwnd: HWND, screen_x: i32, screen_y: i32) -> Result<(), FailureReason> {
        self.clicks.lock().unwrap().push((screen_x, screen_y));
        Ok(())
    }
    fn anti_afk_jump(&self, _hwnd: HWND) -> Result<(), FailureReason> {
        self.jumps.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn play_macro(
        &self,
        _hwnd: HWND,
        steps: &[MacroStep],
        _stop_flag: &AtomicBool,
    ) -> Result<(), FailureReason> {
        if !steps.is_empty() {
            self.macro_plays.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

struct MockUiSink {
    events: Mutex<Vec<UiEvent>>,
}
impl UiEventSink for MockUiSink {
    fn emit(&self, event: UiEvent) {
        self.events.lock().unwrap().push(event);
    }
}

fn mock_win(id: u64) -> RobloxWindowInfo {
    RobloxWindowInfo {
        instance_id: id,
        hwnd: HWND(std::ptr::null_mut()),
        pid: 9000 + id as u32,
        title: "Roblox".to_string(),
        window_left: 0,
        window_top: 0,
        window_width: 800,
        window_height: 600,
    }
}

#[test]
fn test_worker_detects_disconnect_and_clicks_reconnect_with_mock_services() {
    let win = RobloxWindowInfo {
        instance_id: 101,
        hwnd: HWND(std::ptr::null_mut()),
        pid: 9999,
        title: "Roblox".to_string(),
        window_left: 0,
        window_top: 0,
        window_width: 800,
        window_height: 600,
    };

    let frame = CapturedFrame::new(4, 4, vec![255; 64]);
    let items = vec![DetectedText {
        text: "Połącz ponownie".to_string(),
        bounding_box: NormalizedRect::new(0.4, 0.5, 0.6, 0.6),
        target_point: Some(NormalizedPoint { x: 0.5, y: 0.55 }),
        whole_icon_box: None,
        word_boxes: vec![],
    }];

    let input_svc = Arc::new(MockInputSvc::new());
    let ui_sink = Arc::new(MockUiSink {
        events: Mutex::new(Vec::new()),
    });

    let (_tx, rx) = mpsc::sync_channel(4);
    let mut worker = EngineWorker::new(
        Config::default(),
        Arc::new(MockWindowSvc { windows: vec![win] }),
        Arc::new(MockCaptureSvc { frame }),
        Arc::new(MockOcrSvc::with_items(items)),
        input_svc.clone(),
        ui_sink.clone(),
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(false)),
        rx,
    );

    worker.run_monitoring_cycle(Instant::now());

    let clicks = input_svc.clicks.lock().unwrap();
    assert_eq!(clicks.len(), 1);
    assert_eq!(clicks[0], (400, 330));
    assert_eq!(worker.metrics.captures, 1);
    assert_eq!(worker.metrics.ocr_runs, 1);
    assert_eq!(worker.metrics.reconnect_attempts, 1);
    assert_eq!(worker.trackers.get(&101).unwrap().restarts, 1);
    assert_eq!(
        worker.trackers.get(&101).unwrap().state,
        InstanceState::Reconnecting
    );
}

#[test]
fn test_worker_start_emits_snapshot_immediately_and_stop_halts() {
    let input_svc = Arc::new(MockInputSvc::new());
    let ui_sink = Arc::new(MockUiSink {
        events: Mutex::new(Vec::new()),
    });
    let (_tx, rx) = mpsc::sync_channel(4);
    let mut worker = EngineWorker::new(
        Config::default(),
        Arc::new(MockWindowSvc { windows: vec![mock_win(1)] }),
        Arc::new(MockCaptureSvc { frame: CapturedFrame::new(4, 4, vec![255; 64]) }),
        Arc::new(MockOcrSvc::with_items(vec![])),
        input_svc,
        ui_sink.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        rx,
    );

    worker.handle_command(UiCommand::StartAutomator);
    assert!(worker.is_running_flag.load(Ordering::SeqCst));
    let events = ui_sink.events.lock().unwrap();
    assert!(matches!(
        events.last(),
        Some(UiEvent::TelemetrySnapshot(s)) if s.running
    ));
    drop(events);

    worker.handle_command(UiCommand::StopAutomator);
    assert!(!worker.is_running_flag.load(Ordering::SeqCst));
}

#[test]
fn test_worker_caches_ocr_on_unchanged_frame_and_respects_reconnect_cooldown() {
    let mut cfg = Config::default();
    cfg.macro_enabled = true;
    cfg.macro_replay_on_reconnect = true;
    cfg.macro_steps = vec![MacroStep::Key {
        vk: 0x20,
        scan_code: 0x39,
        extended: false,
        down: true,
        delay_ms: 10,
    }];

    let ocr_svc = Arc::new(MockOcrSvc::with_items(vec![DetectedText {
        text: "Reconnect".to_string(),
        bounding_box: NormalizedRect::new(0.4, 0.5, 0.6, 0.6),
        target_point: Some(NormalizedPoint { x: 0.5, y: 0.55 }),
        whole_icon_box: None,
        word_boxes: vec![],
    }]));
    let input_svc = Arc::new(MockInputSvc::new());
    let ui_sink = Arc::new(MockUiSink {
        events: Mutex::new(Vec::new()),
    });
    let (_tx, rx) = mpsc::sync_channel(4);
    let mut worker = EngineWorker::new(
        cfg,
        Arc::new(MockWindowSvc { windows: vec![mock_win(202)] }),
        Arc::new(MockCaptureSvc { frame: CapturedFrame::new(4, 4, vec![255; 64]) }),
        ocr_svc.clone(),
        input_svc.clone(),
        ui_sink,
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(false)),
        rx,
    );

    let t0 = Instant::now();
    worker.run_monitoring_cycle(t0);
    assert_eq!(ocr_svc.scan_calls.load(Ordering::SeqCst), 1);
    assert_eq!(input_svc.clicks.lock().unwrap().len(), 1);
    assert!(worker.trackers.get(&202).unwrap().pending_macro_replay_at.is_some());

    // Second cycle immediately with identical frame: reconnect click invalidated cached_ocr so OCR re-runs,
    // while reconnect cooldown still blocks a second click.
    worker.run_monitoring_cycle(t0 + Duration::from_secs(1));
    assert_eq!(ocr_svc.scan_calls.load(Ordering::SeqCst), 2);
    assert_eq!(input_svc.clicks.lock().unwrap().len(), 1);
}

#[test]
fn test_worker_smart_click_event_cooldown_and_macro_replay() {
    let mut cfg = Config::default();
    cfg.auto_teleport_to_event = true;
    cfg.event_target_keywords = vec!["turkey hunt".to_string()];
    cfg.macro_enabled = true;
    cfg.macro_replay_on_event = true;
    cfg.macro_steps = vec![MacroStep::Key {
        vk: 0x20,
        scan_code: 0x39,
        extended: false,
        down: true,
        delay_ms: 10,
    }];

    let sidebar_item = DetectedText {
        text: "Turkey Hunt".to_string(),
        bounding_box: NormalizedRect::new(0.06, 0.66, 0.14, 0.70),
        target_point: Some(NormalizedPoint { x: 0.10, y: 0.62 }),
        whole_icon_box: None,
        word_boxes: vec![],
    };
    let input_svc = Arc::new(MockInputSvc::new());
    let ui_sink = Arc::new(MockUiSink {
        events: Mutex::new(Vec::new()),
    });
    let (_tx, rx) = mpsc::sync_channel(4);
    let mut worker = EngineWorker::new(
        cfg,
        Arc::new(MockWindowSvc { windows: vec![mock_win(303)] }),
        Arc::new(MockCaptureSvc { frame: CapturedFrame::new(4, 4, vec![255; 64]) }),
        Arc::new(MockOcrSvc::with_sidebar(vec![sidebar_item])),
        input_svc.clone(),
        ui_sink,
        Arc::new(AtomicBool::new(true)),
        Arc::new(AtomicBool::new(false)),
        rx,
    );

    let t0 = Instant::now();
    worker.run_monitoring_cycle(t0);
    assert_eq!(input_svc.clicks.lock().unwrap().len(), 1);
    assert_eq!(input_svc.macro_plays.load(Ordering::SeqCst), 1);

    // Immediate second cycle is blocked by event cooldown
    worker.run_monitoring_cycle(t0 + Duration::from_secs(1));
    assert_eq!(input_svc.clicks.lock().unwrap().len(), 1);
}

#[test]
fn test_worker_commands_config_logging_redaction_and_export() {
    let mut cfg = Config::default();
    cfg.vip_server_url = Some("https://www.roblox.com/games/8737899170?privateServerLinkCode=secret".to_string());
    let input_svc = Arc::new(MockInputSvc::new());
    let ui_sink = Arc::new(MockUiSink {
        events: Mutex::new(Vec::new()),
    });
    let (_tx, rx) = mpsc::sync_channel(4);
    let mut worker = EngineWorker::new(
        cfg,
        Arc::new(MockWindowSvc { windows: vec![mock_win(404)] }),
        Arc::new(MockCaptureSvc { frame: CapturedFrame::new(4, 4, vec![255; 64]) }),
        Arc::new(MockOcrSvc::with_items(vec![])),
        input_svc.clone(),
        ui_sink.clone(),
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
        rx,
    );

    worker.handle_command(UiCommand::TriggerTestJump);
    assert_eq!(input_svc.jumps.load(Ordering::SeqCst), 1);
    assert_eq!(worker.metrics.anti_afk_jumps, 1);

    worker.handle_command(UiCommand::SetAntiAfkInterval { minutes: 5 });
    assert_eq!(worker.config.anti_afk_interval_seconds, 300.0);

    worker.handle_command(UiCommand::ExportLogsJsonl);
    let events = ui_sink.events.lock().unwrap();
    let exported = events.iter().find_map(|e| match e {
        UiEvent::LogsJsonlExported { content, .. } => Some(content.clone()),
        _ => None,
    }).expect("should emit LogsJsonlExported");
    assert!(exported.contains("\"hasVipServerUrl\":true"));
    assert!(!exported.contains("secret"));
}

#[test]
#[ignore = "Multi-stage live diagnostic test against open Roblox windows (run on interactive desktop)"]
fn test_multistage_live_event_pipeline() {
    use crate::capture::Win32CaptureService;
    use crate::ocr::WindowsMediaOcrService;
    use crate::window::Win32WindowService;

    let win_svc = Win32WindowService;
    let cap_svc = Win32CaptureService;
    let ocr_svc = WindowsMediaOcrService;
    let config = Config::default();

    // Stage 1: Window discovery
    let windows = win_svc.discover_windows(
        &config.target_process_names,
        &config.fallback_window_title,
    );
    println!("[Stage 1] Discovered {} Roblox windows", windows.len());
    assert!(!windows.is_empty(), "No live Roblox windows found");

    for (i, win) in windows.iter().enumerate() {
        let num = i + 1;
        // Stage 2: Frame capture
        let t0 = Instant::now();
        let frame = cap_svc.capture_window(win.hwnd).expect("capture failed");
        println!(
            "[Stage 2] Window #{} (PID {}) captured {}x{} in {:?}",
            num,
            win.pid,
            frame.width,
            frame.height,
            t0.elapsed()
        );

        // Stage 3: Full-frame OCR
        let t1 = Instant::now();
        let full_items = ocr_svc
            .scan_frame(&frame, Duration::from_secs(5))
            .expect("full OCR failed");
        println!(
            "[Stage 3] Full-frame OCR ({:?}): {} lines",
            t1.elapsed(),
            full_items.len()
        );

        // Stage 4: Standard sidebar OCR + High-Contrast sidebar OCR
        let t2 = Instant::now();
        let sb_items = ocr_svc
            .scan_left_sidebar(&frame, 0.35, Duration::from_secs(5))
            .expect("sidebar OCR failed");
        println!(
            "[Stage 4a] Sidebar OCR ({:?}): {} lines -> {:?}",
            t2.elapsed(),
            sb_items.len(),
            sb_items.iter().map(|x| &x.text).collect::<Vec<_>>()
        );

        let t3 = Instant::now();
        let hc_items = ocr_svc
            .scan_left_sidebar_high_contrast(&frame, 0.35, Duration::from_secs(5))
            .expect("high-contrast OCR failed");
        println!(
            "[Stage 4b] High-Contrast Sidebar OCR ({:?}): {} lines -> {:?}",
            t3.elapsed(),
            hc_items.len(),
            hc_items.iter().map(|x| &x.text).collect::<Vec<_>>()
        );

        // Stage 5: Classifier + IconTracer
        let kw = if config.event_target_keywords.is_empty() {
            vec!["turkey hunt".to_string()]
        } else {
            config.event_target_keywords.clone()
        };
        let matched = Classifier::find_event_target(&sb_items, &kw, 0.40)
            .or_else(|| Classifier::find_event_target(&full_items, &kw, 0.40))
            .or_else(|| Classifier::find_event_target(&hc_items, &kw, 0.40));
        println!(
            "[Stage 5] Window #{} match: {:?}",
            num,
            matched.as_ref().map(|m| &m.text)
        );
    }
}
