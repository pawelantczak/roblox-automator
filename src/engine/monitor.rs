use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::engine::classifier::Classifier;
use crate::engine::constants::{
    EVENT_MAX_NORMALIZED_X, MODAL_OCR_REGION, SIDEBAR_OCR_WIDTH_RATIO,
};
use crate::engine::tracer::IconTracer;
use crate::engine::EngineWorker;
use crate::models::{
    CapturedFrame, DetectedText, FailureReason, InstanceId, InstanceState, LogLevel, NormalizedRect,
};

impl EngineWorker {
    pub(super) fn run_monitoring_cycle(&mut self, now: Instant) {
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
                    Some(items) => {
                        if let Some(tracker) = self.trackers.get_mut(&win.instance_id) {
                            tracker.cached_ocr = Some(items.clone());
                        }
                        items
                    }
                    None => continue,
                }
            };

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
                let (mx0, my0, mx1, my1) = MODAL_OCR_REGION;
                if let Ok(modal_items) = self.ocr_svc.scan_region_upscaled(
                    &frame,
                    NormalizedRect::new(mx0, my0, mx1, my1),
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
                self.replay_macro_for_window(&win, win_num, "reconnect");
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
                let sidebar_items = match self.ocr_svc.scan_left_sidebar(
                    &frame,
                    SIDEBAR_OCR_WIDTH_RATIO,
                    ocr_timeout,
                ) {
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
                    EVENT_MAX_NORMALIZED_X,
                )
                .or_else(|| {
                    Classifier::find_event_target(
                        &detected_items,
                        &self.config.event_target_keywords,
                        EVENT_MAX_NORMALIZED_X,
                    )
                })
                .or_else(|| {
                    if let Ok(hc_items) = self.ocr_svc.scan_left_sidebar_high_contrast(
                        &frame,
                        SIDEBAR_OCR_WIDTH_RATIO,
                        ocr_timeout,
                    ) {
                        Classifier::find_event_target(
                            &hc_items,
                            &self.config.event_target_keywords,
                            EVENT_MAX_NORMALIZED_X,
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
                                    self.replay_macro_for_window(&win, win_num, "event");
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
                if !self.config.auto_teleport_to_event
                    || self.config.event_target_keywords.is_empty()
                {
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
        let afk_due = self.config.anti_afk_enabled
            && self
                .global_afk_deadline
                .map(|d| Instant::now() >= d)
                .unwrap_or(false);

        if afk_due {
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
        frame: &CapturedFrame,
        timeout: Duration,
        now: Instant,
    ) -> Option<Vec<DetectedText>> {
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
}
