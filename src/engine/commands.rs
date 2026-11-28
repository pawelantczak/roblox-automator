use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::EngineWorker;
use crate::models::{LogLevel, UiCommand, UiEvent};
use crate::power::PowerService;
use crate::storage::StorageService;

impl EngineWorker {
    pub(super) fn drain_commands(&mut self) {
        loop {
            match self.cmd_rx.try_recv() {
                Ok(cmd) => self.handle_command(cmd),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.shutdown_flag.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }
    }

    pub(super) fn handle_command(&mut self, cmd: UiCommand) {
        let now = Instant::now();
        match cmd {
            UiCommand::StartAutomator => {
                if !self.is_running_flag.swap(true, Ordering::SeqCst) {
                    PowerService::enable_sleep_prevention();
                    let afk_interval =
                        Duration::from_secs_f64(self.config.anti_afk_interval_seconds);
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
                    if last_ui_emit.elapsed()
                        >= Duration::from_millis(self.config.ui_refresh_interval_ms)
                    {
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
                self.ui_sink
                    .emit(UiEvent::ConfigLoaded(self.config.clone()));
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
                        message: format!(
                            "Roblox window #{} not found for recording.",
                            window_index + 1
                        ),
                    });
                    return;
                };
                self.ensure_uniform_window_size(win);
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
                        message: format!(
                            "Roblox window #{} not found for macro playback.",
                            window_index + 1
                        ),
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
                    self.logs
                        .push(LogLevel::Action, None, "Stopping macro playback...");
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
                self.ui_sink
                    .emit(UiEvent::ConfigLoaded(self.config.clone()));
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
                self.ui_sink
                    .emit(UiEvent::ConfigLoaded(self.config.clone()));
                self.ui_dirty = true;
                self.emit_snapshot(now);
            }
        }
    }
}
