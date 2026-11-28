use std::sync::atomic::AtomicBool;
use std::time::Instant;

use crate::engine::EngineWorker;
use crate::models::{LogLevel, UiEvent};
use crate::storage::StorageService;
use crate::window::RobloxWindowInfo;

impl EngineWorker {
    pub(super) fn ensure_uniform_window_size(&self, win: &RobloxWindowInfo) {
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

    pub(super) fn replay_macro_for_window(
        &mut self,
        win: &RobloxWindowInfo,
        win_num: usize,
        trigger: &str,
    ) {
        self.ensure_uniform_window_size(win);
        self.logs.push(
            LogLevel::Action,
            Some(win.instance_id),
            format!(
                "Replaying post-{} macro ({} steps) on window #{}...",
                trigger,
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
                format!("Post-{} macro failed on window #{}: {}", trigger, win_num, reason),
            );
        }
    }

    pub(super) fn poll_macro_player(&mut self, now: Instant) {
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

    pub(super) fn poll_macro_recorder(&mut self, now: Instant) {
        let finished = self
            .macro_recorder
            .as_ref()
            .map(|r| r.is_finished())
            .unwrap_or(false);
        if finished {
            self.finish_macro_recording(now);
        } else if self.macro_recorder.is_some() {
            self.ui_dirty = true;
        }
    }

    pub(super) fn finish_macro_recording(&mut self, now: Instant) {
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
            self.ui_sink
                .emit(UiEvent::ConfigLoaded(self.config.clone()));
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
}
