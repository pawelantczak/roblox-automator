use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::capture::CaptureService;
use crate::classifier::Classifier;
use crate::input::InputService;
use crate::models::{Config, InstanceCard, UIStateBroadcast};
use crate::ocr::OcrService;
use crate::power::PowerService;
use crate::tracer::IconTracer;
use crate::window::WindowService;

pub struct WatcherEngine {
    pub config: Arc<RwLock<Config>>,
    pub is_running: Arc<AtomicBool>,
    pub logs: Arc<Mutex<Vec<String>>>,
    pub last_afk_times: Arc<Mutex<std::collections::HashMap<isize, Instant>>>,
    pub restarts_count: Arc<Mutex<std::collections::HashMap<isize, u32>>>,
}

impl WatcherEngine {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            is_running: Arc::new(AtomicBool::new(false)),
            logs: Arc::new(Mutex::new(vec!["[System] Initialized Roblox Automator engine.".to_string()])),
            last_afk_times: Arc::new(Mutex::new(std::collections::HashMap::new())),
            restarts_count: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn append_log(&self, msg: String) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(msg);
        if logs.len() > 150 {
            logs.remove(0);
        }
    }

    pub fn start(&self) {
        if !self.is_running.swap(true, Ordering::SeqCst) {
            PowerService::enable_sleep_prevention();
            self.append_log("[System] Watcher started. Windows sleep prevention active.".to_string());
        }
    }

    pub fn stop(&self) {
        if self.is_running.swap(false, Ordering::SeqCst) {
            PowerService::disable_sleep_prevention();
            self.append_log("[System] Watcher stopped. Windows sleep prevention released.".to_string());
        }
    }

    pub fn trigger_test_jump_all(&self) {
        let windows = WindowService::find_roblox_windows();
        self.append_log(format!("[Action] Triggering Anti-AFK focus jumps across {} windows...", windows.len()));

        std::thread::spawn(move || {
            for (_idx, win) in windows.iter().enumerate() {
                InputService::anti_afk_jump(win.hwnd);
                std::thread::sleep(Duration::from_millis(150));
            }
        });
    }

    /// Performs one single monitoring iteration across all active Roblox windows
    pub fn poll_cycle(&self) -> UIStateBroadcast {
        let config = self.config.read().unwrap().clone();
        let running = self.is_running.load(Ordering::SeqCst);
        let windows = WindowService::find_roblox_windows();
        let now = Instant::now();

        let mut instance_cards = Vec::new();
        let mut min_afk_remaining_secs = config.anti_afk_interval_seconds;

        for (idx, win) in windows.iter().enumerate() {
            let hwnd_val = win.hwnd.0 as isize;
            let mut status = if running { "HEALTHY".to_string() } else { "IDLE".to_string() };
            let mut event_status = "Scanning".to_string();

            // Calculate Anti-AFK remaining time
            let mut last_afk_map = self.last_afk_times.lock().unwrap();
            let last_afk = *last_afk_map.entry(hwnd_val).or_insert(now);
            let elapsed_afk = now.duration_since(last_afk).as_secs_f64();
            let remaining_afk = (config.anti_afk_interval_seconds - elapsed_afk).max(0.0);
            if remaining_afk < min_afk_remaining_secs {
                min_afk_remaining_secs = remaining_afk;
            }

            let afk_countdown = if !config.anti_afk_enabled {
                "OFF".to_string()
            } else if !running {
                "STANDBY".to_string()
            } else {
                let m = (remaining_afk as u64) / 60;
                let s = (remaining_afk as u64) % 60;
                format!("{:02}m {:02}s", m, s)
            };

            let restarts = *self.restarts_count.lock().unwrap().entry(hwnd_val).or_insert(0);

            // Background screen capture
            let mut thumb_base64 = None;
            if let Some(frame) = CaptureService::capture_window(win.hwnd) {
                thumb_base64 = CaptureService::create_thumbnail_base64(&frame, 320, 180);

                if running {
                    // 1. Scan for disconnect dialog
                    let detected_items = OcrService::scan_frame(&frame);
                    let mut is_disconnected = false;
                    let mut disconnect_match = String::new();

                    for item in &detected_items {
                        if Classifier::is_fatal_disconnect(&item.text, &config.disconnect_keywords, &config.ignore_phrases) {
                            is_disconnected = true;
                            disconnect_match = item.text.clone();
                            break;
                        }
                    }

                    if is_disconnected {
                        status = "DISCONNECTED".to_string();
                        self.append_log(format!("[ALERT] Disconnect detected on #{} ({})! Matched: {}", idx + 1, win.pid, disconnect_match));

                        if let Some(reconnect_btn) = Classifier::find_reconnect_button(&detected_items, &config.reconnect_button_text, &config.ignore_phrases) {
                            if config.auto_click_reconnect {
                                status = "RECONNECTING".to_string();
                                let click_pt = reconnect_btn.click_point();
                                let screen_x = win.bounds.left + (click_pt.x * (win.client_width as f32)) as i32;
                                let screen_y = win.bounds.top + (click_pt.y * (win.client_height as f32)) as i32;

                                self.append_log(format!("[Action] Clicking Reconnect at ({}, {}) for #{}...", screen_x, screen_y, idx + 1));
                                InputService::click_at(win.hwnd, screen_x, screen_y);

                                let mut r_map = self.restarts_count.lock().unwrap();
                                *r_map.entry(hwnd_val).or_insert(0) += 1;
                            }
                        }
                    } else {
                        // Healthy game: Check for event icon
                        if config.auto_teleport_to_event {
                            let sidebar_items = OcrService::scan_left_sidebar(&frame, 0.20);
                            if let Some(target) = Classifier::find_event_target(&sidebar_items, &config.event_target_keywords, 0.35) {
                                let _refined = IconTracer::refine_target(target, &frame);
                                event_status = "Traced ✓".to_string();
                            }
                        }

                        // Check Anti-AFK trigger
                        if config.anti_afk_enabled && remaining_afk <= 0.0 {
                            self.append_log(format!("[Anti-AFK] Triggering keep-alive jump for #{} ({})", idx + 1, win.pid));
                            InputService::anti_afk_jump(win.hwnd);
                            last_afk_map.insert(hwnd_val, Instant::now());
                        }
                    }
                }
            }

            instance_cards.push(InstanceCard {
                id: idx + 1,
                pid: win.pid,
                hwnd: hwnd_val,
                title: win.title.clone(),
                place_id: config.place_id.clone(),
                status,
                afk_countdown,
                event_status,
                restarts,
                screenshot_base64: thumb_base64,
            });
        }

        let next_afk = format!("{:02}m {:02}s", (min_afk_remaining_secs as u64) / 60, (min_afk_remaining_secs as u64) % 60);
        let logs_copy = self.logs.lock().unwrap().clone();
        let target_kw = config.event_target_keywords.first().cloned().unwrap_or_else(|| "Event".to_string());

        UIStateBroadcast {
            running,
            instances: instance_cards,
            queue_status: "Idle".to_string(),
            event_keyword: target_kw,
            next_afk_countdown: next_afk,
            logs: logs_copy,
        }
    }
}
