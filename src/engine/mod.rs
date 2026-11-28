pub mod classifier;
pub mod scheduler;
pub mod tracer;

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::capture::ScreenCapture;
use crate::engine::classifier::Classifier;
use crate::engine::scheduler::InstanceTracker;
use crate::engine::tracer::IconTracer;
use crate::input::InputAutomation;
use crate::models::{
    BoundedLogQueue, Config, EngineMetrics, FailureReason, InstanceCard, InstanceId,
    InstanceState, LogLevel, UIStateSnapshot, UiCommand, UiEvent,
};
use crate::ocr::OcrProvider;
use crate::power::PowerService;
use crate::storage::StorageService;
use crate::window::{RobloxWindowInfo, WindowDiscovery};

pub trait UiEventSink: Send + Sync {
    fn emit(&self, event: UiEvent);
}

#[derive(Clone)]
pub struct EngineHandle {
    cmd_tx: SyncSender<UiCommand>,
}

impl EngineHandle {
    pub fn send(&self, cmd: UiCommand) -> Result<(), String> {
        self.cmd_tx
            .try_send(cmd)
            .map_err(|e| format!("Engine command queue full or closed: {}", e))
    }
}

pub struct EngineController {
    handle: EngineHandle,
    shutdown: Arc<AtomicBool>,
    worker_handle: Option<JoinHandle<()>>,
}

impl EngineController {
    pub fn start(
        initial_config: Config,
        window_svc: Arc<dyn WindowDiscovery>,
        capture_svc: Arc<dyn ScreenCapture>,
        ocr_svc: Arc<dyn OcrProvider>,
        input_svc: Arc<dyn InputAutomation>,
        ui_sink: Arc<dyn UiEventSink>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::sync_channel::<UiCommand>(32);
        let is_running = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));

        let worker_running = Arc::clone(&is_running);
        let worker_shutdown = Arc::clone(&shutdown);

        let worker_handle = thread::Builder::new()
            .name("engine-worker".to_string())
            .spawn(move || {
                let mut worker = EngineWorker::new(
                    initial_config,
                    window_svc,
                    capture_svc,
                    ocr_svc,
                    input_svc,
                    ui_sink,
                    worker_running,
                    worker_shutdown,
                    cmd_rx,
                );
                worker.run();
            })
            .expect("failed to spawn engine-worker thread");

        Self {
            handle: EngineHandle { cmd_tx },
            shutdown,
            worker_handle: Some(worker_handle),
        }
    }

    pub fn handle(&self) -> EngineHandle {
        self.handle.clone()
    }

    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.worker_handle.take() {
            let _ = handle.join();
        }
        PowerService::disable_sleep_prevention();
    }
}

impl Drop for EngineController {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct EngineWorker {
    config: Config,
    window_svc: Arc<dyn WindowDiscovery>,
    capture_svc: Arc<dyn ScreenCapture>,
    ocr_svc: Arc<dyn OcrProvider>,
    input_svc: Arc<dyn InputAutomation>,
    ui_sink: Arc<dyn UiEventSink>,
    is_running_flag: Arc<AtomicBool>,
    shutdown_flag: Arc<AtomicBool>,
    cmd_rx: Receiver<UiCommand>,
    trackers: HashMap<InstanceId, InstanceTracker>,
    ordered_ids: Vec<InstanceId>,
    logs: BoundedLogQueue,
    metrics: EngineMetrics,
    global_afk_deadline: Option<Instant>,
    next_scan_deadline: Option<Instant>,
    session_started_at: Option<Instant>,
    macro_recorder: Option<crate::input::recorder::MacroRecorderHandle>,
    macro_player: Option<(Arc<AtomicBool>, thread::JoinHandle<()>)>,
    ui_dirty: bool,
}

impl EngineWorker {
    #[allow(clippy::too_many_arguments)]
    fn new(
        config: Config,
        window_svc: Arc<dyn WindowDiscovery>,
        capture_svc: Arc<dyn ScreenCapture>,
        ocr_svc: Arc<dyn OcrProvider>,
        input_svc: Arc<dyn InputAutomation>,
        ui_sink: Arc<dyn UiEventSink>,
        is_running_flag: Arc<AtomicBool>,
        shutdown_flag: Arc<AtomicBool>,
        cmd_rx: Receiver<UiCommand>,
    ) -> Self {
        #[cfg(not(test))]
        let mut logs = BoundedLogQueue::new_with_file(
            200,
            std::path::PathBuf::from("roblox-automator.jsonl"),
        );
        #[cfg(test)]
        let mut logs = BoundedLogQueue::new(200);
        logs.push_with_config(
            LogLevel::Info,
            None,
            "Initialized Roblox Automator worker engine.",
            &config,
        );
        Self {
            config,
            window_svc,
            capture_svc,
            ocr_svc,
            input_svc,
            ui_sink,
            is_running_flag,
            shutdown_flag,
            cmd_rx,
            trackers: HashMap::new(),
            ordered_ids: Vec::new(),
            logs,
            metrics: EngineMetrics::default(),
            global_afk_deadline: None,
            next_scan_deadline: None,
            session_started_at: None,
            macro_recorder: None,
            macro_player: None,
            ui_dirty: true,
        }
    }

    fn run(&mut self) {
        let now = Instant::now();
        // Perform initial passive discovery and thumbnail capture on startup so UI shows previews immediately.
        let initial_windows = self.discover_and_sync_windows(now, false);
        self.capture_idle_thumbnails(&initial_windows, false);
        self.emit_snapshot(now);

        let mut next_idle_discovery_deadline = now + Duration::from_secs(3);
        let mut next_idle_thumb_refresh_deadline = now + Duration::from_secs(10);
        let mut next_ui_deadline = now + Duration::from_millis(self.config.ui_refresh_interval_ms);

        while !self.shutdown_flag.load(Ordering::Relaxed) {
            self.drain_commands();
            if self.shutdown_flag.load(Ordering::Relaxed) {
                break;
            }

            let now = Instant::now();
            self.poll_macro_recorder(now);
            self.poll_macro_player(now);
            let running = self.is_running_flag.load(Ordering::SeqCst);

            if running {
                let scan_due = self
                    .next_scan_deadline
                    .map(|d| now >= d)
                    .unwrap_or(true);
                if scan_due {
                    self.run_monitoring_cycle(now);
                    self.next_scan_deadline = Some(
                        Instant::now()
                            + Duration::from_secs_f64(self.config.check_interval_seconds),
                    );
                    self.ui_dirty = true;
                }
            } else if now >= next_idle_discovery_deadline {
                // While stopped, skip OCR/automation completely; keep window list fresh and occasionally refresh thumbnails.
                let windows = self.discover_and_sync_windows(now, false);
                let refresh_thumbs = now >= next_idle_thumb_refresh_deadline;
                self.capture_idle_thumbnails(&windows, refresh_thumbs);
                next_idle_discovery_deadline = Instant::now() + Duration::from_secs(3);
                if refresh_thumbs {
                    next_idle_thumb_refresh_deadline = Instant::now() + Duration::from_secs(10);
                }
            }

            if now >= next_ui_deadline {
                if self.ui_dirty || running {
                    self.emit_snapshot(now);
                    self.ui_dirty = false;
                }
                next_ui_deadline =
                    Instant::now() + Duration::from_millis(self.config.ui_refresh_interval_ms);
            }

            thread::sleep(Duration::from_millis(50));
        }

        if self.is_running_flag.swap(false, Ordering::SeqCst) {
            PowerService::disable_sleep_prevention();
        }
    }

    fn drain_commands(&mut self) {
        loop {
            match self.cmd_rx.try_recv() {
                Ok(cmd) => self.handle_command(cmd),
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
    }

    fn handle_command(&mut self, cmd: UiCommand) {
        let now = Instant::now();
        match cmd {
            UiCommand::StartAutomator => {
                if !self.is_running_flag.swap(true, Ordering::SeqCst) {
                    PowerService::enable_sleep_prevention();
                    let afk_interval = Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
                    self.global_afk_deadline = if self.config.anti_afk_enabled {
                        Some(now + afk_interval)
                    } else {
                        None
                    };
                    self.session_started_at = Some(now);
                    self.logs.push(
                        LogLevel::Action,
                        None,
                        "Automator started. Windows sleep prevention active.",
                    );
                    self.discover_and_sync_windows(now, true);
                    self.next_scan_deadline = None;
                    self.ui_dirty = false;
                    self.emit_snapshot(now);
                }
            }
            UiCommand::StopAutomator => {
                if self.is_running_flag.swap(false, Ordering::SeqCst) {
                    PowerService::disable_sleep_prevention();
                    self.global_afk_deadline = None;
                    self.next_scan_deadline = None;
                    self.session_started_at = None;
                    let standby = self.config.ui_strings.status_standby.clone();
                    for tracker in self.trackers.values_mut() {
                        tracker.enter_idle(now, &standby);
                    }
                    self.logs.push(
                        LogLevel::Action,
                        None,
                        "Automator stopped. Anti-AFK timer halted and sleep prevention released.",
                    );
                    self.ui_dirty = true;
                    self.emit_snapshot(now);
                }
            }
            UiCommand::TriggerTestJump => {
                let windows = self.window_svc.discover_windows(
                    &self.config.target_process_names,
                    &self.config.fallback_window_title,
                );
                self.logs.push(
                    LogLevel::Action,
                    None,
                    format!(
                        "Triggering Anti-AFK focus jumps across {} windows...",
                        windows.len()
                    ),
                );
                let mut last_ui_emit = Instant::now();
                for win in windows {
                    self.drain_commands();
                    if self.shutdown_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    match self.input_svc.anti_afk_jump(win.hwnd) {
                        Ok(()) => {
                            self.metrics.anti_afk_jumps += 1;
                        }
                        Err(reason) => {
                            self.metrics.input_failures += 1;
                            self.logs.push(
                                LogLevel::Error,
                                Some(win.instance_id),
                                format!("Test jump failed: {}", reason),
                            );
                        }
                    }
                    if last_ui_emit.elapsed() >= Duration::from_millis(self.config.ui_refresh_interval_ms) {
                        let t_now = Instant::now();
                        self.emit_snapshot(t_now);
                        last_ui_emit = t_now;
                    }
                }
                if self.is_running_flag.load(Ordering::SeqCst) && self.config.anti_afk_enabled {
                    self.global_afk_deadline = Some(
                        Instant::now()
                            + Duration::from_secs_f64(self.config.anti_afk_interval_seconds),
                    );
                }
                self.ui_dirty = true;
                self.emit_snapshot(Instant::now());
            }
            UiCommand::ClearLogs => {
                self.logs.clear();
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::GetConfig => {
                self.ui_sink.emit(UiEvent::ConfigLoaded(self.config.clone()));
            }
            UiCommand::SaveConfig { mut config } => {
                if let Err(err) = config.validate() {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                if let Err(err) = StorageService::save_config(&config) {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                if self.config != config {
                    self.config = config.clone();
                    self.logs.push_with_config(
                        LogLevel::Action,
                        None,
                        format!(
                            "Configuration updated (scan={}s, ui={}ms, cooldown={}s, afk={}s).",
                            config.check_interval_seconds,
                            config.ui_refresh_interval_ms,
                            config.reconnect_cooldown_seconds,
                            config.anti_afk_interval_seconds
                        ),
                        &self.config,
                    );
                    if self.is_running_flag.load(Ordering::SeqCst) {
                        if self.config.anti_afk_enabled {
                            let new_interval =
                                Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
                            let capped = self
                                .global_afk_deadline
                                .map(|d| d.min(now + new_interval))
                                .unwrap_or(now + new_interval);
                            self.global_afk_deadline = Some(capped);
                        } else {
                            self.global_afk_deadline = None;
                        }
                    }
                }
                self.ui_sink.emit(UiEvent::ConfigSaved(config));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::SetAntiAfkInterval { minutes } => {
                let mut updated = self.config.clone();
                updated.anti_afk_interval_seconds = (minutes as f64) * 60.0;
                if let Err(err) = updated.validate() {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                if let Err(err) = StorageService::save_config(&updated) {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                self.config = updated.clone();
                if self.is_running_flag.load(Ordering::SeqCst) && self.config.anti_afk_enabled {
                    let new_interval =
                        Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
                    let capped = self
                        .global_afk_deadline
                        .map(|d| d.min(now + new_interval))
                        .unwrap_or(now + new_interval);
                    self.global_afk_deadline = Some(capped);
                }
                self.logs.push_with_config(
                    LogLevel::Action,
                    None,
                    format!("Anti-AFK interval updated to {} min.", minutes),
                    &self.config,
                );
                self.ui_sink.emit(UiEvent::ConfigLoaded(updated));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::SetScanInterval { seconds } => {
                let mut updated = self.config.clone();
                updated.check_interval_seconds = seconds as f64;
                if let Err(err) = updated.validate() {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                if let Err(err) = StorageService::save_config(&updated) {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                self.config = updated.clone();
                if self.is_running_flag.load(Ordering::SeqCst) {
                    let new_interval = Duration::from_secs_f64(self.config.check_interval_seconds);
                    let capped = self
                        .next_scan_deadline
                        .map(|d| d.min(now + new_interval))
                        .unwrap_or(now + new_interval);
                    self.next_scan_deadline = Some(capped);
                }
                self.logs.push_with_config(
                    LogLevel::Action,
                    None,
                    format!("Scan interval updated to {}s.", seconds),
                    &self.config,
                );
                self.ui_sink.emit(UiEvent::ConfigLoaded(updated));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::SetEventTracking { enabled, keywords } => {
                let mut updated = self.config.clone();
                updated.auto_teleport_to_event = enabled;
                updated.event_target_keywords = keywords;
                if let Err(err) = updated.validate() {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                if let Err(err) = StorageService::save_config(&updated) {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                self.config = updated.clone();
                self.logs.push_with_config(
                    LogLevel::Action,
                    None,
                    format!(
                        "Smart Click {} (keywords: {:?}).",
                        if enabled { "enabled" } else { "disabled" },
                        updated.event_target_keywords
                    ),
                    &self.config,
                );
                self.ui_sink.emit(UiEvent::ConfigLoaded(updated));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::ExportLogsJsonl => {
                self.logs.push(
                    LogLevel::Action,
                    None,
                    "Exported activity logs to JSONL.",
                );
                let content = self.logs.export_jsonl();
                self.ui_sink.emit(UiEvent::LogsJsonlExported {
                    filename: "roblox-automator-logs.jsonl".to_string(),
                    content,
                });
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::StartMacroRecording { window_index } => {
                if self.macro_recorder.is_some() {
                    return;
                }
                let windows = self.window_svc.discover_windows(
                    &self.config.target_process_names,
                    &self.config.fallback_window_title,
                );
                let Some(win) = windows.get(window_index) else {
                    self.ui_sink.emit(UiEvent::ActionError {
                        message: format!("Roblox window #{} not found for recording.", window_index + 1),
                    });
                    return;
                };
                self.ensure_uniform_window_size(&win);
                match crate::input::recorder::start_macro_recording(win.hwnd, 3) {
                    Ok(handle) => {
                        self.macro_recorder = Some(handle);
                        self.logs.push(
                            LogLevel::Action,
                            Some(win.instance_id),
                            format!(
                                "Starting macro recording on window #{} (3s countdown, move mouse out to stop)...",
                                window_index + 1
                            ),
                        );
                        self.ui_dirty = true;
                        self.emit_snapshot(now);
                    }
                    Err(err) => {
                        self.ui_sink.emit(UiEvent::ActionError { message: err });
                    }
                }
            }
            UiCommand::StopMacroRecording => {
                self.finish_macro_recording(now);
            }
            UiCommand::TestMacro { window_index } => {
                if self.config.macro_steps.is_empty()
                    || self.macro_player.is_some()
                    || self.macro_recorder.is_some()
                {
                    return;
                }
                let windows = self.window_svc.discover_windows(
                    &self.config.target_process_names,
                    &self.config.fallback_window_title,
                );
                let Some(win) = windows.get(window_index) else {
                    self.ui_sink.emit(UiEvent::ActionError {
                        message: format!("Roblox window #{} not found for macro playback.", window_index + 1),
                    });
                    return;
                };
                self.ensure_uniform_window_size(win);
                self.logs.push(
                    LogLevel::Action,
                    Some(win.instance_id),
                    format!(
                        "Playing macro ({} steps) on window #{}...",
                        self.config.macro_steps.len(),
                        window_index + 1
                    ),
                );
                let stop_flag = Arc::new(AtomicBool::new(false));
                let stop_c = Arc::clone(&stop_flag);
                let input_svc = Arc::clone(&self.input_svc);
                let steps = self.config.macro_steps.clone();
                let hwnd_raw = win.hwnd.0 as isize;
                let handle = thread::spawn(move || {
                    let hwnd = windows::Win32::Foundation::HWND(hwnd_raw as *mut std::ffi::c_void);
                    let _ = input_svc.play_macro(hwnd, &steps, &stop_c);
                });
                self.macro_player = Some((stop_flag, handle));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::StopMacroPlayback => {
                if let Some((ref stop_flag, _)) = self.macro_player {
                    stop_flag.store(true, Ordering::SeqCst);
                    self.logs.push(LogLevel::Action, None, "Stopping macro playback...");
                }
            }
            UiCommand::ClearMacro => {
                self.config.macro_steps.clear();
                self.config.macro_enabled = false;
                let _ = StorageService::save_config(&self.config);
                self.logs.push_with_config(
                    LogLevel::Action,
                    None,
                    "Cleared recorded macro.",
                    &self.config,
                );
                self.ui_sink.emit(UiEvent::ConfigLoaded(self.config.clone()));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
            UiCommand::SetMacroEnabled { enabled } => {
                self.config.macro_enabled = enabled;
                if let Err(err) = StorageService::save_config(&self.config) {
                    self.ui_sink.emit(UiEvent::ActionError { message: err });
                    return;
                }
                self.logs.push_with_config(
                    LogLevel::Action,
                    None,
                    format!("Macro replay {}.", if enabled { "enabled" } else { "disabled" }),
                    &self.config,
                );
                self.ui_sink.emit(UiEvent::ConfigLoaded(self.config.clone()));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
        }
    }

    fn ensure_uniform_window_size(&self, win: &RobloxWindowInfo) {
        if !self.config.uniform_window_size {
            return;
        }
        let target_w = self.config.uniform_window_width;
        let target_h = self.config.uniform_window_height;
        if (win.window_width - target_w).abs() > 4 || (win.window_height - target_h).abs() > 4 {
            let _ = self
                .window_svc
                .resize_window_in_place(win.hwnd, target_w, target_h);
        }
    }

    fn poll_macro_player(&mut self, now: Instant) {
        let finished = self
            .macro_player
            .as_ref()
            .map(|(_, h)| h.is_finished())
            .unwrap_or(false);
        if finished {
            if let Some((_, handle)) = self.macro_player.take() {
                let _ = handle.join();
            }
            self.ui_dirty = true;
            self.emit_snapshot(now);
        }
    }

    fn poll_macro_recorder(&mut self, now: Instant) {
        let finished = self
            .macro_recorder
            .as_ref()
            .map(|r| r.is_finished())
            .unwrap_or(false);
        if finished {
            self.finish_macro_recording(now);
        } else if self.macro_recorder.is_some() {
            self.ui_dirty = true;
            self.emit_snapshot(now);
        }
    }

    fn finish_macro_recording(&mut self, now: Instant) {
        let Some(handle) = self.macro_recorder.take() else {
            return;
        };
        let steps = handle.join();
        let count = steps.len();
        let dur_ms = crate::models::MacroStep::total_duration_ms(&steps);
        if !steps.is_empty() {
            self.config.macro_steps = steps;
            let _ = StorageService::save_config(&self.config);
            self.logs.push_with_config(
                LogLevel::Action,
                None,
                format!(
                    "Macro recorded: {} steps ({:.1}s).",
                    count,
                    (dur_ms as f64) / 1000.0
                ),
                &self.config,
            );
            self.ui_sink.emit(UiEvent::ConfigLoaded(self.config.clone()));
        } else {
            self.logs.push(
                LogLevel::Warn,
                None,
                "Macro recording stopped with 0 events captured.",
            );
        }
        self.ui_dirty = true;
        self.emit_snapshot(now);
    }

    fn discover_and_sync_windows(&mut self, now: Instant, running: bool) -> Vec<RobloxWindowInfo> {
        let windows = self.window_svc.discover_windows(
            &self.config.target_process_names,
            &self.config.fallback_window_title,
        );
        let active_ids: HashSet<InstanceId> = windows.iter().map(|w| w.instance_id).collect();

        // Remove closed windows
        let prev_len = self.trackers.len();
        self.trackers.retain(|id, _| active_ids.contains(id));
        if self.trackers.len() != prev_len {
            self.ui_dirty = true;
        }

        self.ordered_ids.clear();
        let afk_interval = Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
        let standby = &self.config.ui_strings.status_standby;

        for (idx, win) in windows.iter().enumerate() {
            let win_num = idx + 1;
            if running {
                self.ensure_uniform_window_size(&win);
            }
            self.ordered_ids.push(win.instance_id);
            let tracker = self.trackers.entry(win.instance_id).or_insert_with(|| {
                self.ui_dirty = true;
                self.logs.push(
                    LogLevel::Info,
                    Some(win.instance_id),
                    format!("Discovered window #{} (PID {})", win_num, win.pid),
                );
                let mut t = InstanceTracker::new(win.instance_id, win.pid, win.title.clone(), now);
                if running {
                    t.ensure_afk_deadline(now, self.config.anti_afk_enabled, afk_interval);
                } else {
                    t.enter_idle(now, standby);
                }
                t
            });

            if tracker.title != win.title {
                tracker.title = win.title.clone();
                self.ui_dirty = true;
            }

            if running {
                tracker.ensure_afk_deadline(now, self.config.anti_afk_enabled, afk_interval);
            } else {
                tracker.enter_idle(now, standby);
            }
        }

        windows
    }

    /// Captures a one-time thumbnail for any newly discovered window that has no cached preview yet.
    fn capture_idle_thumbnails(&mut self, windows: &[RobloxWindowInfo], force_refresh: bool) {
        for win in windows {
            if self.shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
            let (needs_thumb, prev_hash) = self
                .trackers
                .get(&win.instance_id)
                .map(|t| (t.cached_thumbnail.is_none(), t.last_frame_hash))
                .unwrap_or((false, None));
            if !force_refresh && !needs_thumb {
                continue;
            }
            if let Ok(frame) = self.capture_svc.capture_window(win.hwnd) {
                if !needs_thumb && prev_hash == Some(frame.frame_hash) {
                    continue;
                }
                let thumb = self.capture_svc.encode_thumbnail_base64(
                    &frame,
                    self.config.thumbnail_max_width,
                    self.config.thumbnail_max_height,
                    self.config.thumbnail_jpeg_quality,
                );
                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                    tracker.cached_thumbnail = thumb;
                    tracker.last_frame_hash = Some(frame.frame_hash);
                    self.ui_dirty = true;
                }
            }
        }
    }

    fn run_monitoring_cycle(&mut self, now: Instant) {
        let windows = self.discover_and_sync_windows(now, true);
        let ocr_timeout = Duration::from_millis(self.config.ocr_timeout_ms);
        let afk_interval = Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
        let cooldown = Duration::from_secs_f64(self.config.reconnect_cooldown_seconds);

        let mut last_ui_emit = Instant::now();
        for (idx, win) in windows.into_iter().enumerate() {
            self.drain_commands();
            if self.shutdown_flag.load(Ordering::Relaxed)
                || !self.is_running_flag.load(Ordering::SeqCst)
            {
                break;
            }
            let win_num = idx + 1;

            let frame = match self.capture_svc.capture_window(win.hwnd) {
                Ok(f) => {
                    self.metrics.captures += 1;
                    f
                }
                Err(err) => {
                    self.metrics.capture_failures += 1;
                    if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                        tracker.transition_to(
                            InstanceState::Error,
                            Some(FailureReason::CaptureFailed(err.clone())),
                            now,
                        );
                    }
                    self.logs.push(
                        LogLevel::Warn,
                        Some(win.instance_id),
                        format!("Capture failed on window #{}: {}", win_num, err),
                    );
                    continue;
                }
            };

            let frame_unchanged = self
                .trackers
                .get(&win.instance_id)
                .and_then(|t| t.last_frame_hash)
                .map(|h| h == frame.frame_hash)
                .unwrap_or(false);

            // Encode thumbnail only if frame changed or not yet cached
            let need_thumb = !frame_unchanged
                || self
                    .trackers
                    .get(&win.instance_id)
                    .and_then(|t| t.cached_thumbnail.as_ref())
                    .is_none();

            if need_thumb {
                let thumb = self.capture_svc.encode_thumbnail_base64(
                    &frame,
                    self.config.thumbnail_max_width,
                    self.config.thumbnail_max_height,
                    self.config.thumbnail_jpeg_quality,
                );
                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                    tracker.cached_thumbnail = thumb;
                    tracker.last_frame_hash = Some(frame.frame_hash);
                }
            }

            // Use cached OCR if frame hash is identical and OCR already succeeded
            let cached_ocr = if frame_unchanged {
                self.trackers
                    .get(&win.instance_id)
                    .and_then(|t| t.cached_ocr.clone())
            } else {
                None
            };

            let detected_items = if let Some(cached) = cached_ocr {
                self.metrics.ocr_skipped_unchanged += 1;
                cached
            } else {
                self.logs.push(
                    LogLevel::Info,
                    Some(win.instance_id),
                    format!("Scanning window #{} (PID {})...", win_num, win.pid),
                );
                match self.run_ocr_for_instance(win.instance_id, &frame, ocr_timeout, now) {
                    Some(items) => items,
                    None => continue,
                }
            };

            if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                tracker.cached_ocr = Some(detected_items.clone());
            }

            // 1. Check disconnect keywords & localized reconnect button
            let disconnect_match = detected_items
                .iter()
                .find(|item| {
                    Classifier::is_fatal_disconnect(
                        &item.text,
                        &self.config.disconnect_keywords,
                        &self.config.ignore_phrases,
                    )
                })
                .map(|item| item.text.clone());

            let mut reconnect_btn = Classifier::find_reconnect_button(
                &detected_items,
                &self.config.reconnect_button_phrases,
                &self.config.ignore_phrases,
            );

            if disconnect_match.is_some() && reconnect_btn.is_none() {
                if let Ok(modal_items) = self.ocr_svc.scan_region_upscaled(
                    &frame,
                    crate::models::NormalizedRect::new(0.15, 0.15, 0.85, 0.90),
                    ocr_timeout,
                ) {
                    reconnect_btn = Classifier::find_reconnect_button(
                        &modal_items,
                        &self.config.reconnect_button_phrases,
                        &self.config.ignore_phrases,
                    );
                }
            }

            let is_disconnected = disconnect_match.is_some() || reconnect_btn.is_some();

            if is_disconnected {
                let matched_label = disconnect_match.unwrap_or_else(|| {
                    format!(
                        "Button '{}'",
                        reconnect_btn.as_ref().map(|b| b.text.as_str()).unwrap_or("")
                    )
                });
                let reconnect_crop = reconnect_btn.as_ref().and_then(|btn| {
                    let pt = btn.click_point();
                    self.capture_svc.encode_point_crop_base64(
                        &frame,
                        &pt,
                        40,
                        self.config.thumbnail_jpeg_quality,
                    )
                });
                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                    tracker.reconnect_crop_base64 = reconnect_crop.clone();
                    tracker.event_crop_base64 = None;
                }
                self.logs.push_with_icon(
                    LogLevel::Warn,
                    Some(win.instance_id),
                    format!(
                        "Disconnect detected on window #{} (PID {}): {}",
                        win_num, win.pid, matched_label
                    ),
                    reconnect_crop,
                );

                let cooldown_active = self
                    .trackers
                    .get(&win.instance_id)
                    .map(|t| t.is_cooldown_active(now))
                    .unwrap_or(false);

                if cooldown_active {
                    self.logs.push(
                        LogLevel::Info,
                        Some(win.instance_id),
                        format!("Reconnect cooldown active on window #{}", win_num),
                    );
                    if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                        tracker.transition_to(
                            InstanceState::Disconnected,
                            Some(FailureReason::ReconnectCooldownActive),
                            now,
                        );
                    }
                    continue;
                }

                if let Some(btn) = reconnect_btn {
                    if self.config.auto_click_reconnect {
                        if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                            tracker.transition_to(InstanceState::Reconnecting, None, now);
                        }
                        self.metrics.reconnect_attempts += 1;

                        let click_pt = btn.click_point();
                        let screen_x =
                            win.window_left + (click_pt.x * (win.window_width as f32)) as i32;
                        let screen_y =
                            win.window_top + (click_pt.y * (win.window_height as f32)) as i32;

                        self.logs.push(
                            LogLevel::Action,
                            Some(win.instance_id),
                            format!(
                                "Clicking reconnect button '{}' on window #{} at ({}, {})",
                                btn.text, win_num, screen_x, screen_y
                            ),
                        );

                        match self.input_svc.click_at(win.hwnd, screen_x, screen_y) {
                            Ok(()) => {
                                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                                    tracker.restarts += 1;
                                    tracker.start_reconnect_cooldown(Instant::now(), cooldown);
                                    tracker.cached_ocr = None;
                                    if self.config.macro_enabled
                                        && self.config.macro_replay_on_reconnect
                                        && !self.config.macro_steps.is_empty()
                                    {
                                        tracker.pending_macro_replay_at =
                                            Some(Instant::now() + Duration::from_secs(4));
                                    }
                                }
                            }
                            Err(reason) => {
                                self.metrics.reconnect_failures += 1;
                                self.metrics.input_failures += 1;
                                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                                    tracker.transition_to(
                                        InstanceState::Error,
                                        Some(reason.clone()),
                                        Instant::now(),
                                    );
                                    tracker.start_reconnect_cooldown(Instant::now(), cooldown);
                                }
                                self.logs.push(
                                    LogLevel::Error,
                                    Some(win.instance_id),
                                    format!("Reconnect click failed: {}", reason),
                                );
                            }
                        }
                    } else if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                        tracker.transition_to(InstanceState::Disconnected, None, now);
                    }
                } else if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                    tracker.transition_to(
                        InstanceState::Disconnected,
                        Some(FailureReason::ReconnectButtonNotFound),
                        now,
                    );
                }
                continue;
            }

            // 2. Healthy state & optional post-reconnect macro / event navigation
            let run_reconnect_macro = if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                tracker.reconnect_crop_base64 = None;
                if let Some(due) = tracker.pending_macro_replay_at {
                    if Instant::now() >= due {
                        tracker.pending_macro_replay_at = None;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            if run_reconnect_macro
                && self.config.macro_enabled
                && self.config.macro_replay_on_reconnect
                && !self.config.macro_steps.is_empty()
            {
                self.ensure_uniform_window_size(&win);
                self.logs.push(
                    LogLevel::Action,
                    Some(win.instance_id),
                    format!(
                        "Replaying post-reconnect macro ({} steps) on window #{}...",
                        self.config.macro_steps.len(),
                        win_num
                    ),
                );
                if let Err(reason) = self.input_svc.play_macro(
                    win.hwnd,
                    &self.config.macro_steps,
                    &AtomicBool::new(false),
                ) {
                    self.metrics.input_failures += 1;
                    self.logs.push(
                        LogLevel::Error,
                        Some(win.instance_id),
                        format!("Post-reconnect macro failed on window #{}: {}", win_num, reason),
                    );
                }
            }
            let mut event_status = self.config.ui_strings.status_healthy.clone();
            if self.config.auto_teleport_to_event && !self.config.event_target_keywords.is_empty() {
                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                    tracker.transition_to(InstanceState::Navigating, None, now);
                }
                self.logs.push(
                    LogLevel::Info,
                    Some(win.instance_id),
                    format!(
                        "Looking for event text on window #{} (PID {})...",
                        win_num, win.pid
                    ),
                );
                let sidebar_items = match self.ocr_svc.scan_left_sidebar(&frame, 0.35, ocr_timeout) {
                    Ok(items) => items,
                    Err(err) => {
                        self.logs.push(
                            LogLevel::Warn,
                            Some(win.instance_id),
                            format!("Sidebar OCR failed on window #{}: {}", win_num, err),
                        );
                        Vec::new()
                    }
                };

                let target = Classifier::find_event_target(
                    &sidebar_items,
                    &self.config.event_target_keywords,
                    0.40,
                )
                .or_else(|| {
                    Classifier::find_event_target(
                        &detected_items,
                        &self.config.event_target_keywords,
                        0.40,
                    )
                })
                .or_else(|| {
                    if let Ok(hc_items) =
                        self.ocr_svc
                            .scan_left_sidebar_high_contrast(&frame, 0.35, ocr_timeout)
                    {
                        Classifier::find_event_target(
                            &hc_items,
                            &self.config.event_target_keywords,
                            0.40,
                        )
                    } else {
                        None
                    }
                });

                if let Some(target) = target {
                    let matched_text = target.text.clone();
                    let refined = IconTracer::refine_target(target, &frame);
                    let click_pt = refined.click_point();
                    let crop_b64 = self.capture_svc.encode_point_crop_base64(
                        &frame,
                        &click_pt,
                        40,
                        self.config.thumbnail_jpeg_quality,
                    );
                    if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                        tracker.event_crop_base64 = crop_b64.clone();
                    }
                    event_status = self.config.ui_strings.status_traced.clone();
                    self.logs.push_with_icon(
                        LogLevel::Action,
                        Some(win.instance_id),
                        format!(
                            "Event text found on window #{} (PID {}): '{}'",
                            win_num, win.pid, matched_text
                        ),
                        crop_b64,
                    );

                    let event_cd_active = self
                        .trackers
                        .get(&win.instance_id)
                        .map(|t| t.is_event_cooldown_active(now))
                        .unwrap_or(false);

                    if event_cd_active {
                        self.logs.push(
                            LogLevel::Info,
                            Some(win.instance_id),
                            format!("Event click cooldown active on window #{}", win_num),
                        );
                    } else {
                        let click_pt = refined.click_point();
                        let screen_x =
                            win.window_left + (click_pt.x * (win.window_width as f32)) as i32;
                        let screen_y =
                            win.window_top + (click_pt.y * (win.window_height as f32)) as i32;

                        self.logs.push(
                            LogLevel::Action,
                            Some(win.instance_id),
                            format!(
                                "Clicking event icon '{}' on window #{} at ({}, {})",
                                matched_text, win_num, screen_x, screen_y
                            ),
                        );

                        match self.input_svc.click_at(win.hwnd, screen_x, screen_y) {
                            Ok(()) => {
                                if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                                    tracker.start_event_cooldown(Instant::now(), cooldown);
                                    tracker.cached_ocr = None;
                                }
                                if self.config.macro_enabled
                                    && self.config.macro_replay_on_event
                                    && !self.config.macro_steps.is_empty()
                                {
                                    std::thread::sleep(Duration::from_millis(1500));
                                    self.ensure_uniform_window_size(&win);
                                    self.logs.push(
                                        LogLevel::Action,
                                        Some(win.instance_id),
                                        format!(
                                            "Replaying post-event macro ({} steps) on window #{}...",
                                            self.config.macro_steps.len(),
                                            win_num
                                        ),
                                    );
                                    if let Err(reason) = self.input_svc.play_macro(
                                        win.hwnd,
                                        &self.config.macro_steps,
                                        &AtomicBool::new(false),
                                    ) {
                                        self.metrics.input_failures += 1;
                                        self.logs.push(
                                            LogLevel::Error,
                                            Some(win.instance_id),
                                            format!(
                                                "Post-event macro failed on window #{}: {}",
                                                win_num, reason
                                            ),
                                        );
                                    }
                                }
                            }
                            Err(reason) => {
                                self.metrics.input_failures += 1;
                                self.logs.push(
                                    LogLevel::Error,
                                    Some(win.instance_id),
                                    format!(
                                        "Event click failed on window #{}: {}",
                                        win_num, reason
                                    ),
                                );
                            }
                        }
                    }
                } else {
                    if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                        tracker.event_crop_base64 = None;
                    }
                    self.logs.push(
                        LogLevel::Info,
                        Some(win.instance_id),
                        format!(
                            "Event text not found on window #{} (sidebar OCR saw {} lines)",
                            win_num,
                            sidebar_items.len()
                        ),
                    );
                }
            }

            if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                if !self.config.auto_teleport_to_event || self.config.event_target_keywords.is_empty() {
                    tracker.event_crop_base64 = None;
                }
                tracker.event_status = event_status;
                tracker.transition_to(InstanceState::Healthy, None, now);
            }

            if last_ui_emit.elapsed() >= Duration::from_millis(self.config.ui_refresh_interval_ms) {
                let t_now = Instant::now();
                self.emit_snapshot(t_now);
                last_ui_emit = t_now;
            }
        }

        // 3. Global Anti-AFK deadline check across all active windows
        let global_afk_due = self.config.anti_afk_enabled
            && self
                .global_afk_deadline
                .map(|d| now >= d)
                .unwrap_or(false);

        if global_afk_due {
            let active_windows = self.window_svc.discover_windows(
                &self.config.target_process_names,
                &self.config.fallback_window_title,
            );
            self.logs.push(
                LogLevel::Action,
                None,
                format!(
                    "Triggering global Anti-AFK jump across {} windows.",
                    active_windows.len()
                ),
            );
            for (idx, win) in active_windows.into_iter().enumerate() {
                if self.shutdown_flag.load(Ordering::Relaxed) {
                    break;
                }
                let win_num = idx + 1;
                match self.input_svc.anti_afk_jump(win.hwnd) {
                    Ok(()) => {
                        self.metrics.anti_afk_jumps += 1;
                        self.logs.push(
                            LogLevel::Action,
                            Some(win.instance_id),
                            format!("Anti-AFK jump completed on window #{} (PID {})", win_num, win.pid),
                        );
                    }
                    Err(reason) => {
                        self.metrics.input_failures += 1;
                        if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                            tracker.transition_to(
                                InstanceState::Error,
                                Some(reason.clone()),
                                Instant::now(),
                            );
                        }
                        self.logs.push(
                            LogLevel::Error,
                            Some(win.instance_id),
                            format!("Anti-AFK jump failed on window #{}: {}", win_num, reason),
                        );
                    }
                }
            }
            self.global_afk_deadline = Some(Instant::now() + afk_interval);
        }
    }

    fn run_ocr_for_instance(
        &mut self,
        instance_id: InstanceId,
        frame: &crate::models::CapturedFrame,
        timeout: Duration,
        now: Instant,
    ) -> Option<Vec<crate::models::DetectedText>> {
        self.metrics.ocr_runs += 1;
        match self.ocr_svc.scan_frame(frame, timeout) {
            Ok(items) => Some(items),
            Err(err) => {
                self.metrics.ocr_failures += 1;
                let reason = if err.to_lowercase().contains("timed out") {
                    FailureReason::OcrTimeout
                } else {
                    FailureReason::OcrFailed(err.clone())
                };
                if let Some(tracker) = self.trackers.get_mut(&instance_id) {
                    tracker.ocr_failures += 1;
                    tracker.transition_to(InstanceState::Error, Some(reason), now);
                }
                self.logs.push(
                    LogLevel::Error,
                    Some(instance_id),
                    format!("OCR failed: {}", err),
                );
                None
            }
        }
    }

    fn emit_snapshot(&self, now: Instant) {
        let running = self.is_running_flag.load(Ordering::SeqCst);
        let mut cards = Vec::with_capacity(self.ordered_ids.len());

        let global_afk_secs = if running && self.config.anti_afk_enabled {
            self.global_afk_deadline
                .map(|d| d.saturating_duration_since(now).as_secs())
        } else {
            None
        };

        let next_scan_secs = if running {
            self.next_scan_deadline
                .map(|d| d.saturating_duration_since(now).as_secs())
        } else {
            None
        };

        let session_uptime_secs = if running {
            self.session_started_at
                .map(|s| now.saturating_duration_since(s).as_secs())
        } else {
            None
        };

        let mut total_restarts = 0u32;

        for id in &self.ordered_ids {
            if let Some(t) = self.trackers.get(id) {
                total_restarts = total_restarts.saturating_add(t.restarts);
                cards.push(InstanceCard {
                    instance_id: t.instance_id,
                    pid: t.pid,
                    title: t.title.clone(),
                    state: t.state,
                    failure_reason: t.failure_reason.as_ref().map(|r| r.to_string()),
                    afk_remaining_secs: global_afk_secs,
                    cooldown_remaining_secs: t.cooldown_remaining_secs(now),
                    event_status: t.event_status.clone(),
                    restarts: t.restarts,
                    ocr_failures: t.ocr_failures,
                    screenshot_base64: t.cached_thumbnail.clone(),
                    event_crop_base64: t.event_crop_base64.clone(),
                    reconnect_crop_base64: t.reconnect_crop_base64.clone(),
                });
            }
        }

        let event_keyword = self
            .config
            .event_target_keywords
            .first()
            .cloned()
            .unwrap_or_else(|| self.config.ui_strings.status_no_event.clone());

        let snapshot = UIStateSnapshot {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            running,
            instances: cards,
            queue_status: if running {
                self.config.ui_strings.status_active.clone()
            } else {
                self.config.ui_strings.status_idle.clone()
            },
            event_keyword,
            auto_teleport_to_event: self.config.auto_teleport_to_event,
            event_target_keywords: self.config.event_target_keywords.clone(),
            scan_interval_seconds: self.config.check_interval_seconds.round() as u32,
            next_scan_remaining_secs: next_scan_secs,
            session_uptime_secs,
            total_restarts,
            anti_afk_enabled: self.config.anti_afk_enabled,
            anti_afk_interval_minutes: (self.config.anti_afk_interval_seconds / 60.0).round() as u32,
            next_afk_remaining_secs: global_afk_secs,
            macro_enabled: self.config.macro_enabled,
            macro_recording: self.macro_recorder.is_some(),
            macro_playing: self.macro_player.is_some(),
            macro_countdown_secs: self
                .macro_recorder
                .as_ref()
                .and_then(|r| r.countdown_secs()),
            macro_step_count: self.config.macro_steps.len(),
            macro_duration_ms: crate::models::MacroStep::total_duration_ms(&self.config.macro_steps),
            logs: self.logs.snapshot(),
            metrics: self.metrics.clone(),
        };

        self.ui_sink.emit(UiEvent::TelemetrySnapshot(snapshot));
    }
}

#[cfg(test)]
mod tests;
