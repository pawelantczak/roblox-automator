use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::capture::ScreenCapture;
use crate::engine::EngineWorker;
use crate::input::InputAutomation;
use crate::models::{Config, UiCommand, UiEvent};
use crate::ocr::OcrProvider;
use crate::power::PowerService;
use crate::window::WindowDiscovery;

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
