use tao::event_loop::EventLoopProxy;
use tao::window::Window;
use wry::{WebView, WebViewBuilder};

use crate::engine::EngineHandle;
use crate::models::{UiCommand, UiEvent};

pub fn check_webview2_runtime() -> Result<String, String> {
    wry::webview_version().map_err(|e| {
        format!(
            "Microsoft Edge WebView2 Runtime is required but could not be detected ({}).\n\nPlease install it via:\nwinget install --id Microsoft.EdgeWebView2Runtime -e",
            e
        )
    })
}

pub fn build_bundled_html() -> String {
    use base64::prelude::*;
    let html_template = include_str!("../../resources/index.html");
    let css = include_str!("../../resources/styles.css");
    let js = include_str!("../../resources/app.js");
    let icon_bytes = include_bytes!("../../resources/app_icon.png");
    let icon_data_uri = format!("data:image/png;base64,{}", BASE64_STANDARD.encode(icon_bytes));

    html_template
        .replace("/* __LOCAL_CSS__ */", css)
        .replace("/* __LOCAL_JS__ */", js)
        .replace("__APP_ICON_DATA_URI__", &icon_data_uri)
}

pub fn create_webview(
    window: &Window,
    engine_handle: EngineHandle,
    proxy: EventLoopProxy<UiEvent>,
) -> anyhow::Result<WebView> {
    let bundled_html = build_bundled_html();

    let builder = WebViewBuilder::new()
        .with_html(bundled_html)
        .with_navigation_handler(|uri| {
            let trimmed = uri.trim_end_matches('/');
            if matches!(trimmed, "https://ps99clan.xyz" | "https://discord.gg/ce5yGvTf9") {
                let _ = std::process::Command::new("explorer")
                    .arg(trimmed)
                    .spawn();
                return false;
            }
            uri == "about:blank" || uri.starts_with("data:")
        })
        .with_ipc_handler(move |msg| match UiCommand::parse_and_validate(msg.body().as_str()) {
            Ok(cmd) => {
                if let Err(err) = engine_handle.send(cmd) {
                    let _ = proxy.send_event(UiEvent::ActionError { message: err });
                }
            }
            Err(err) => {
                let _ = proxy.send_event(UiEvent::ActionError { message: err });
            }
        });

    #[cfg(not(debug_assertions))]
    let builder = builder.with_devtools(false);

    let webview = builder.build(window)?;
    Ok(webview)
}

pub fn dispatch_event_to_webview(webview: &WebView, event: &UiEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        let script = format!(
            "if (window.handleNativeEvent) window.handleNativeEvent({});",
            json
        );
        let _ = webview.evaluate_script(&script);
    }
}
