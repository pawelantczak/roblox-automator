use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::engine::scheduler::InstanceTracker;
use crate::engine::EngineWorker;
use crate::models::{InstanceCard, InstanceId, LogLevel, UIStateSnapshot, UiEvent};
use crate::window::RobloxWindowInfo;

impl EngineWorker {
    pub(super) fn discover_and_sync_windows(
        &mut self,
        now: Instant,
        running: bool,
    ) -> Vec<RobloxWindowInfo> {
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
                self.ensure_uniform_window_size(win);
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
    pub(super) fn capture_idle_thumbnails(
        &mut self,
        windows: &[RobloxWindowInfo],
        force_refresh: bool,
    ) {
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

    pub(super) fn emit_snapshot(&self, now: Instant) {
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
            anti_afk_interval_minutes: (self.config.anti_afk_interval_seconds / 60.0).round()
                as u32,
            next_afk_remaining_secs: global_afk_secs,
            macro_enabled: self.config.macro_enabled,
            macro_recording: self.macro_recorder.is_some(),
            macro_playing: self.macro_player.is_some(),
            macro_countdown_secs: self
                .macro_recorder
                .as_ref()
                .and_then(|r| r.countdown_secs()),
            macro_step_count: self.config.macro_steps.len(),
            macro_duration_ms: crate::models::MacroStep::total_duration_ms(
                &self.config.macro_steps,
            ),
            logs: self.logs.snapshot(),
            metrics: self.metrics.clone(),
        };

        self.ui_sink.emit(UiEvent::TelemetrySnapshot(snapshot));
    }
}
