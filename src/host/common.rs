use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};
use wry::{Rect, WebView, WebViewBuilder};

use crate::app_core::AppEvent;
use crate::diagnostics;

pub(crate) fn sync_child_webview_bounds(
    window: Option<&Window>,
    webview: Option<&WebView>,
    label: &str,
) {
    let (Some(window), Some(webview)) = (window, webview) else {
        return;
    };
    let size = window.inner_size().to_logical::<u32>(window.scale_factor());
    let bounds = Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: LogicalSize::new(size.width, size.height).into(),
    };
    if let Err(error) = webview.set_bounds(bounds) {
        diagnostics::log_line(
            "ERROR",
            &format!("warning: failed to sync {label} bounds: {error}"),
        );
    }
}

pub(crate) fn sync_main_webview_bounds(window: Option<&Window>, webview: Option<&WebView>) {
    // Wry's non-child Windows WebView2 path subclasses the parent HWND and
    // resizes the controller directly from WM_SIZE. The explicit bounds sync
    // is retained for hosts where the app owns the child view geometry.
    #[cfg(not(target_os = "windows"))]
    sync_child_webview_bounds(window, webview, "main webview");
    #[cfg(target_os = "windows")]
    let _ = (window, webview);
}

pub(crate) fn focus_existing_child_window(window: Option<&Window>) -> bool {
    let Some(window) = window else {
        return false;
    };
    window.focus_window();
    true
}

pub(crate) fn clear_child_window(
    window: &mut Option<Window>,
    window_id: &mut Option<WindowId>,
    webview: &mut Option<WebView>,
) {
    *webview = None;
    *window = None;
    *window_id = None;
}

pub(crate) fn create_fixed_child_window(
    event_loop: &ActiveEventLoop,
    event_loop_proxy: Option<&EventLoopProxy<AppEvent>>,
    title: &str,
    width: f64,
    height: f64,
    html: &str,
    label: &str,
) -> Result<(Window, WindowId, WebView), String> {
    let attrs = Window::default_attributes()
        .with_title(title)
        .with_resizable(false)
        .with_inner_size(LogicalSize::new(width, height))
        .with_min_inner_size(LogicalSize::new(width, height))
        .with_max_inner_size(LogicalSize::new(width, height));
    let window = event_loop
        .create_window(attrs)
        .map_err(|error| format!("failed to create {label} window: {error}"))?;
    let ipc_proxy = event_loop_proxy
        .cloned()
        .ok_or_else(|| format!("missing event loop proxy for {label} window"))?;
    let window_id = window.id();
    let webview = WebViewBuilder::new()
        .with_html(html)
        .with_ipc_handler(move |request: wry::http::Request<String>| {
            let payload = request.into_body();
            let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
        })
        .build_as_child(&window)
        .map_err(|error| format!("failed to create {label} webview: {error}"))?;
    Ok((window, window_id, webview))
}
