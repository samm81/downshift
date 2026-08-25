use winit::dpi::LogicalSize;
use winit::event_loop::{ActiveEventLoop, EventLoopBuilder, EventLoopProxy};
use winit::window::{Window, WindowAttributes, WindowLevel};
use wry::{WebView, WebViewBuilder};

use crate::app_core::AppEvent;
use crate::diagnostics;

pub(crate) fn configure_main_window(initial_size: LogicalSize<f64>) -> WindowAttributes {
    let attributes = Window::default_attributes()
        .with_title("downshift")
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_min_inner_size(initial_size)
        .with_max_inner_size(initial_size)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_inner_size(initial_size);

    #[cfg(target_os = "windows")]
    let attributes = {
        // A transparent WebView2 child can retain the previous opaque DWM
        // redirection bitmap after the host window is resized. The no-
        // redirection path keeps the transparent surface current.
        use winit::platform::windows::WindowAttributesExtWindows;
        attributes
            .with_no_redirection_bitmap(true)
            .with_skip_taskbar(true)
    };

    attributes
}

pub(crate) fn create_main_window(
    event_loop: &ActiveEventLoop,
    attributes: WindowAttributes,
) -> Result<Window, String> {
    event_loop
        .create_window(attributes)
        .map_err(|error| error.to_string())
}

pub(crate) fn native_menu_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

pub(crate) fn configure_event_loop_builder(builder: &mut EventLoopBuilder<AppEvent>) {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = builder;
}

#[cfg(target_os = "macos")]
pub(crate) fn configure_window_for_all_spaces(window: &Window) {
    use objc2_app_kit::{NSView, NSWindowCollectionBehavior};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let ns_view = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr().cast::<NSView>(),
            _ => {
                diagnostics::log_line("ERROR", "warning: window handle was not an AppKit handle");
                return;
            }
        },
        Err(error) => {
            diagnostics::log_line(
                "ERROR",
                &format!(
                    "warning: failed to access window handle for spaces configuration: {error}"
                ),
            );
            return;
        }
    };
    let Some(ns_view) = (unsafe { ns_view.as_ref() }) else {
        diagnostics::log_line("ERROR", "warning: window handle returned a null NSView");
        return;
    };
    let Some(ns_window) = ns_view.window() else {
        diagnostics::log_line("ERROR", "warning: failed to resolve NSWindow from NSView");
        return;
    };

    let mut behavior = unsafe { ns_window.collectionBehavior() };
    behavior.insert(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    behavior.remove(NSWindowCollectionBehavior::MoveToActiveSpace);
    unsafe {
        ns_window.setCollectionBehavior(behavior);
    }
}

pub(crate) fn configure_created_window(window: &Window) {
    #[cfg(target_os = "macos")]
    configure_window_for_all_spaces(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub(crate) fn build_main_webview(
    window: &Window,
    html: &str,
    init_script: &str,
    event_loop_proxy: &EventLoopProxy<AppEvent>,
) -> Result<WebView, String> {
    let ipc_proxy = event_loop_proxy.clone();
    let builder = WebViewBuilder::new()
        .with_html(html)
        .with_transparent(true)
        .with_initialization_script(init_script)
        .with_ipc_handler(move |request: wry::http::Request<String>| {
            let payload = request.into_body();
            let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
        });

    #[cfg(target_os = "windows")]
    let webview = builder.build(window);
    #[cfg(not(target_os = "windows"))]
    let webview = builder.build_as_child(window);

    webview.map_err(|error| error.to_string())
}

pub(crate) fn open_external_url(url: &str) {
    #[cfg(target_os = "macos")]
    let command = ("open", vec![url.to_string()]);
    #[cfg(target_os = "windows")]
    let command = (
        "cmd",
        vec![
            "/C".to_string(),
            "start".to_string(),
            "".to_string(),
            url.to_string(),
        ],
    );
    #[cfg(all(unix, not(target_os = "macos")))]
    let command = ("xdg-open", vec![url.to_string()]);

    let (program, args) = command;
    if let Err(error) = std::process::Command::new(program).args(args).spawn() {
        diagnostics::log_line(
            "ERROR",
            &format!("warning: failed to open external url: {error}"),
        );
    }
}

pub(crate) fn current_os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output();
        if let Ok(output) = output {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !version.is_empty() {
                return format!("macOS {version}");
            }
        }
    }
    std::env::consts::OS.to_string()
}

pub(crate) fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        copy_to_command("pbcopy", text)
    }
    #[cfg(target_os = "windows")]
    {
        copy_to_command("clip.exe", text)
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = text;
        Err("clipboard copy is unsupported on this platform".to_string())
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn copy_to_command(program: &str, text: &str) -> Result<(), String> {
    use std::io::Write;

    let mut process = std::process::Command::new(program)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let Some(stdin) = process.stdin.as_mut() else {
        return Err("clipboard stdin unavailable".to_string());
    };
    stdin
        .write_all(text.as_bytes())
        .map_err(|error| error.to_string())?;
    let status = process.wait().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} exited with status {status}"))
    }
}
