pub mod webview;

use std::sync::Arc;
use std::time::Duration;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;

use crate::capture::Win32CaptureService;
use crate::engine::{EngineController, UiEventSink};
use crate::input::Win32InputService;
use crate::models::{Config, UiCommand, UiEvent};
use crate::ocr::WindowsMediaOcrService;
use crate::window::Win32WindowService;

struct ProxyEventSink {
    proxy: EventLoopProxy<UiEvent>,
}

impl UiEventSink for ProxyEventSink {
    fn emit(&self, event: UiEvent) {
        let _ = self.proxy.send_event(event);
    }
}

pub fn show_fatal_error_box(title: &str, message: &str) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
    let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let msg_w: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(
            HWND(std::ptr::null_mut()),
            windows::core::PCWSTR(msg_w.as_ptr()),
            windows::core::PCWSTR(title_w.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn load_window_icon() -> Option<tao::window::Icon> {
    let bytes = include_bytes!("../../resources/app_icon.png");
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.resize(64, 64, image::imageops::FilterType::Triangle).to_rgba8();
    let (w, h) = rgba.dimensions();
    tao::window::Icon::from_rgba(rgba.into_raw(), w, h).ok()
}

pub fn run_app(initial_config: Config) -> anyhow::Result<()> {
    let event_loop = EventLoopBuilder::<UiEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let mut builder = WindowBuilder::new()
        .with_title(&initial_config.ui_strings.app_title)
        .with_inner_size(LogicalSize::new(1040.0, 680.0))
        .with_min_inner_size(LogicalSize::new(820.0, 520.0));

    if let Some(icon) = load_window_icon() {
        builder = builder.with_window_icon(Some(icon));
    }

    let window = builder.build(&event_loop)?;

    let ui_sink: Arc<dyn UiEventSink> = Arc::new(ProxyEventSink {
        proxy: proxy.clone(),
    });

    let mut engine = Some(EngineController::start(
        initial_config,
        Arc::new(Win32WindowService),
        Arc::new(Win32CaptureService),
        Arc::new(WindowsMediaOcrService),
        Arc::new(Win32InputService),
        ui_sink,
    ));

    let engine_handle = engine.as_ref().unwrap().handle();
    let webview = Some(webview::create_webview(
        &window,
        engine_handle.clone(),
        proxy,
    )?);

    // Request initial config sync once WebView is ready
    let _ = engine_handle.send(UiCommand::GetConfig);

    event_loop.run(move |event, _, control_flow| {
        *control_flow =
            ControlFlow::WaitUntil(std::time::Instant::now() + Duration::from_millis(100));

        match event {
            Event::UserEvent(ui_event) => {
                if let UiEvent::ConfigSaved(ref cfg) = ui_event {
                    window.set_title(&cfg.ui_strings.app_title);
                }
                if let Some(ref wv) = webview {
                    webview::dispatch_event_to_webview(wv, &ui_event);
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                // Stop and join engine workers BEFORE closing the WebView
                if let Some(mut ctrl) = engine.take() {
                    ctrl.shutdown();
                }
                *control_flow = ControlFlow::Exit;
            }
            Event::LoopDestroyed => {
                if let Some(mut ctrl) = engine.take() {
                    ctrl.shutdown();
                }
            }
            _ => {}
        }
    });
}
