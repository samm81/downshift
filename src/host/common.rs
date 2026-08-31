use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use wry::{Rect, WebView};

use super::platform::HostWindow;
use crate::app_core::AppEvent;
use crate::diagnostics;

pub(crate) fn sync_child_webview_bounds(
    window: Option<&HostWindow>,
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

pub(crate) fn sync_main_webview_bounds(window: Option<&HostWindow>, webview: Option<&WebView>) {
    // Wry's non-child Windows WebView2 path subclasses the parent HWND and
    // resizes the controller directly from WM_SIZE. The explicit bounds sync
    // is retained for hosts where the app owns the child view geometry.
    #[cfg(not(target_os = "windows"))]
    sync_child_webview_bounds(window, webview, "main webview");
    #[cfg(target_os = "windows")]
    let _ = (window, webview);
}

pub(crate) fn focus_existing_child_window(window: Option<&HostWindow>) -> bool {
    let Some(window) = window else {
        return false;
    };
    window.focus_window();
    true
}

pub(crate) fn clear_child_window(window: &mut Option<HostWindow>, webview: &mut Option<WebView>) {
    *webview = None;
    *window = None;
}

pub(crate) fn create_fixed_child_window(
    event_loop: &ActiveEventLoop,
    event_loop_proxy: Option<&EventLoopProxy<AppEvent>>,
    title: &str,
    width: f64,
    height: f64,
    html: &str,
    label: &str,
) -> Result<(HostWindow, WebView), String> {
    let ipc_proxy = event_loop_proxy
        .cloned()
        .ok_or_else(|| format!("missing event loop proxy for {label} window"))?;
    #[cfg(target_os = "linux")]
    let window = HostWindow::create_child(
        event_loop,
        title,
        LogicalSize::new(width, height),
        &ipc_proxy,
    )
    .map_err(|error| format!("failed to create {label} window: {error}"))?;
    #[cfg(not(target_os = "linux"))]
    let window = {
        let attrs = winit::window::Window::default_attributes()
            .with_title(title)
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(width, height))
            .with_min_inner_size(LogicalSize::new(width, height))
            .with_max_inner_size(LogicalSize::new(width, height));
        event_loop
            .create_window(attrs)
            .map_err(|error| format!("failed to create {label} window: {error}"))?
    };
    let webview = super::platform::build_child_webview(&window, html, &ipc_proxy)
        .map_err(|error| format!("failed to create {label} webview: {error}"))?;
    Ok((window, webview))
}
