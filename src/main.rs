#![windows_subsystem = "windows"]

mod capture;
mod classifier;
mod engine;
mod input;
mod models;
mod ocr;
mod power;
mod storage;
mod tracer;
mod tray;
mod window;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use muda::MenuEvent;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::TrayIconEvent;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use wry::WebViewBuilder;

use crate::engine::WatcherEngine;
use crate::models::IPCAction;
use crate::storage::StorageService;
use crate::tray::TrayManager;
use crate::window::WindowService;

enum UserEvent {
    UpdateUI(String),
}

fn main() -> anyhow::Result<()> {
    // 1. Single Instance Guard via Win32 Named Mutex
    unsafe {
        let _ = CreateMutexW(None, true, windows::core::w!("Global\\RobloxAutomator_Mutex"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("Another instance of Roblox Automator is already running.");
            return Ok(());
        }
    }

    // 2. Load Configuration and Setup Engine
    let config = StorageService::load_config();
    let engine = Arc::new(WatcherEngine::new(config));

    // 3. Initialize Tao Event Loop & Window
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
        .with_title("Roblox Automator")
        .with_inner_size(LogicalSize::new(1040.0, 680.0))
        .with_min_inner_size(LogicalSize::new(820.0, 520.0))
        .build(&event_loop)?;

    // 4. Setup System Tray
    let tray_manager = TrayManager::new()?;
    let show_item_id = tray_manager.show_item.id().clone();
    let toggle_item_id = tray_manager.toggle_item.id().clone();
    let exit_item_id = tray_manager.exit_item.id().clone();

    // 5. Setup WebView2
    let html_content = include_str!("../resources/index.html");
    let engine_ipc = Arc::clone(&engine);
    let proxy = event_loop.create_proxy();

    let webview = WebViewBuilder::new()
        .with_html(html_content)
        .with_ipc_handler(move |msg| {
            if let Ok(action) = serde_json::from_str::<IPCAction>(msg.body().as_str()) {
                match action {
                    IPCAction::Start => {
                        engine_ipc.start();
                    }
                    IPCAction::Stop => {
                        engine_ipc.stop();
                    }
                    IPCAction::Rescan => {
                        let state = engine_ipc.poll_cycle();
                        if let Ok(json) = serde_json::to_string(&state) {
                            let _ = proxy.send_event(UserEvent::UpdateUI(json));
                        }
                    }
                    IPCAction::TestJump => {
                        engine_ipc.trigger_test_jump_all();
                    }
                    IPCAction::ClearLogs => {
                        engine_ipc.logs.lock().unwrap().clear();
                    }
                    IPCAction::GetConfig => {
                        let cfg = engine_ipc.config.read().unwrap().clone();
                        if let Ok(json) = serde_json::to_string(&cfg) {
                            let script = format!("if (window.populateConfig) window.populateConfig({});", json);
                            let _ = proxy.send_event(UserEvent::UpdateUI(script));
                        }
                    }
                    IPCAction::SaveConfig { config } => {
                        StorageService::save_config(&config);
                        *engine_ipc.config.write().unwrap() = config;
                        engine_ipc.append_log("[Config] Updated and saved settings.".to_string());
                    }
                }
            }
        })
        .build(&window)?;

    // Initial config push to WebView
    {
        let cfg = engine.config.read().unwrap().clone();
        if let Ok(json) = serde_json::to_string(&cfg) {
            let _ = webview.evaluate_script(&format!("if (window.populateConfig) window.populateConfig({});", json));
        }
    }

    // 6. Spawn Background Telemetry & Polling Thread
    let engine_worker = Arc::clone(&engine);
    let proxy_worker = event_loop.create_proxy();
    let is_app_running = Arc::new(AtomicBool::new(true));
    let is_app_running_worker = Arc::clone(&is_app_running);

    std::thread::spawn(move || {
        while is_app_running_worker.load(Ordering::Relaxed) {
            let state = engine_worker.poll_cycle();
            if let Ok(json) = serde_json::to_string(&state) {
                let _ = proxy_worker.send_event(UserEvent::UpdateUI(json));
            }
            std::thread::sleep(Duration::from_millis(1500));
        }
    });

    // 7. Event Loop Lifecycle
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(100));

        // Handle Tray Menu Events
        if let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            if menu_event.id == show_item_id {
                window.set_visible(true);
                window.set_focus();
            } else if menu_event.id == toggle_item_id {
                let currently_running = engine.is_running.load(Ordering::SeqCst);
                if currently_running {
                    engine.stop();
                } else {
                    engine.start();
                }
                let win_count = WindowService::find_roblox_windows().len();
                tray_manager.set_running_state(!currently_running, win_count);
            } else if menu_event.id == exit_item_id {
                is_app_running.store(false, Ordering::Relaxed);
                engine.stop();
                *control_flow = ControlFlow::Exit;
            }
        }

        // Handle Tray Icon Click Events (Left-click toggles window visibility)
        if let Ok(tray_event) = TrayIconEvent::receiver().try_recv() {
            if let tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } = tray_event {
                let visible = window.is_visible();
                window.set_visible(!visible);
                if !visible {
                    window.set_focus();
                }
            }
        }

        match event {
            Event::UserEvent(UserEvent::UpdateUI(script)) => {
                if script.starts_with("if (") {
                    let _ = webview.evaluate_script(&script);
                } else {
                    let s = format!("if (window.updateUIState) window.updateUIState({});", script);
                    let _ = webview.evaluate_script(&s);
                }
            }
            Event::WindowEvent { event, .. } => match event {
                // Intercept window close button (X): hide to tray instead of terminating
                WindowEvent::CloseRequested => {
                    window.set_visible(false);
                }
                _ => {}
            },
            Event::LoopDestroyed => {
                is_app_running.store(false, Ordering::Relaxed);
                engine.stop();
            }
            _ => {}
        }
    });
}
