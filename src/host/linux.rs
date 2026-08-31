use super::platform::LinuxDiagnostics;
use crate::app_core::AppEvent;
use crate::window_policy::{choose_linux_window_backend, LinuxSessionBackend, LinuxWindowBackend};
use downshift::{LinuxOutputPlacement, LinuxWindowMode, LINUX_APPLICATION_ID};
use gtk::glib::translate::ToGlibPtr;
use gtk::prelude::*;
use std::cell::Cell;
use std::ffi::c_void;
use std::rc::Rc;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size};
use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
use winit::window::{Window, WindowAttributes, WindowId};
use wry::{Rect, WebView, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix};
use x11rb::connection::Connection as _;
use x11rb::protocol::xproto::ConnectionExt as _;
use x11rb::wrapper::ConnectionExt as _;

const DEFAULT_MARGIN: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowKind {
    Main,
    Child,
}

#[derive(Clone, Copy)]
struct NativePointerPress {
    root_x: i32,
    root_y: i32,
    timestamp: u32,
}

pub(crate) struct HostWindow {
    winit_window: Window,
    gtk_window: gtk::Window,
    container: gtk::Fixed,
    logical_size: Rc<Cell<LogicalSize<f64>>>,
    physical_position: Rc<Cell<Option<PhysicalPosition<i32>>>>,
    native_pointer_press: Rc<Cell<Option<NativePointerPress>>>,
    scale_factor: Rc<Cell<f64>>,
    kind: WindowKind,
    backend: LinuxWindowBackend,
    available_monitors: Vec<winit::monitor::MonitorHandle>,
    diagnostics: LinuxDiagnostics,
    layer_shell: Option<LayerShellApi>,
}

impl HostWindow {
    pub(crate) fn create_main(
        event_loop: &ActiveEventLoop,
        attributes: WindowAttributes,
        initial_position: Option<PhysicalPosition<i32>>,
        initial_size: LogicalSize<f64>,
        requested_mode: LinuxWindowMode,
        placement: Option<&LinuxOutputPlacement>,
        event_loop_proxy: &EventLoopProxy<AppEvent>,
    ) -> Result<Self, String> {
        Self::create(
            event_loop,
            attributes,
            initial_position,
            initial_size,
            requested_mode,
            placement,
            WindowKind::Main,
            "downshift",
            event_loop_proxy,
        )
    }

    pub(crate) fn create_child(
        event_loop: &ActiveEventLoop,
        title: &str,
        size: LogicalSize<f64>,
        event_loop_proxy: &EventLoopProxy<AppEvent>,
    ) -> Result<Self, String> {
        let attributes = Window::default_attributes()
            .with_title(title)
            .with_visible(false)
            .with_resizable(false)
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size);
        Self::create(
            event_loop,
            attributes,
            None,
            size,
            LinuxWindowMode::NormalWindow,
            None,
            WindowKind::Child,
            title,
            event_loop_proxy,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        event_loop: &ActiveEventLoop,
        attributes: WindowAttributes,
        initial_position: Option<PhysicalPosition<i32>>,
        initial_size: LogicalSize<f64>,
        requested_mode: LinuxWindowMode,
        placement: Option<&LinuxOutputPlacement>,
        kind: WindowKind,
        title: &str,
        event_loop_proxy: &EventLoopProxy<AppEvent>,
    ) -> Result<Self, String> {
        init_gtk()?;
        let proxy_attributes = attributes.with_visible(false);
        let winit_window = event_loop
            .create_window(proxy_attributes)
            .map_err(|error| error.to_string())?;
        let available_monitors = event_loop.available_monitors().collect::<Vec<_>>();
        let session = session_backend(&winit_window);
        let layer_shell_api = (session == LinuxSessionBackend::Wayland
            && requested_mode != LinuxWindowMode::NormalWindow)
            .then(LayerShellApi::load)
            .flatten();
        let overlay_supported = layer_shell_api
            .as_ref()
            .is_some_and(|layer_shell| layer_shell.is_supported());
        let decision = choose_linux_window_backend(
            session,
            requested_mode,
            overlay_supported,
            layer_shell_drag_verified(),
        );
        let mut backend = if kind == WindowKind::Child && session == LinuxSessionBackend::X11 {
            LinuxWindowBackend::X11
        } else if kind == WindowKind::Child {
            LinuxWindowBackend::WaylandNormal
        } else {
            decision.backend
        };
        let mut fallback_reason = if kind == WindowKind::Child {
            None
        } else {
            decision.fallback_reason.map(str::to_string)
        };

        let gtk_window = gtk::Window::new(gtk::WindowType::Toplevel);
        gtk_window.set_title(title);
        gtk_window.set_resizable(false);
        if kind == WindowKind::Main {
            gtk_window.set_decorated(false);
            gtk_window.set_app_paintable(true);
            gtk_window.set_keep_above(backend == LinuxWindowBackend::X11);
            gtk_window.set_skip_taskbar_hint(true);
            gtk_window.set_skip_pager_hint(true);
            gtk_window.set_type_hint(gtk::gdk::WindowTypeHint::Utility);
            if let Some(screen) = gtk::prelude::GtkWindowExt::screen(&gtk_window) {
                if let Some(visual) = screen.rgba_visual() {
                    gtk_window.set_visual(Some(&visual));
                }
            }
        }
        let container = gtk::Fixed::new();
        let logical_size = Rc::new(Cell::new(initial_size));
        let scale_factor = Rc::new(Cell::new(1.0));
        let physical_position = Rc::new(Cell::new(
            if matches!(
                backend,
                LinuxWindowBackend::WaylandNormal | LinuxWindowBackend::WaylandLayerShell
            ) {
                None
            } else {
                initial_position
            },
        ));
        let native_pointer_press = Rc::new(Cell::new(None));
        container.set_size_request(
            logical_size.get().width.round().max(1.0) as i32,
            logical_size.get().height.round().max(1.0) as i32,
        );
        gtk_window.set_default_size(
            logical_size.get().width.round().max(1.0) as i32,
            logical_size.get().height.round().max(1.0) as i32,
        );
        gtk_window.add(&container);

        let window_id = winit_window.id();
        let close_proxy = event_loop_proxy.clone();
        gtk_window.connect_delete_event(move |_, _| {
            let _ = close_proxy.send_event(AppEvent::HostWindowClosed(window_id));
            gtk::glib::Propagation::Proceed
        });
        let resize_proxy = event_loop_proxy.clone();
        let resize_size = logical_size.clone();
        let resize_scale = scale_factor.clone();
        gtk_window.connect_size_allocate(move |window, allocation| {
            resize_size.set(LogicalSize::new(
                f64::from(allocation.width()),
                f64::from(allocation.height()),
            ));
            let _ = resize_proxy.send_event(AppEvent::HostWindowResized(window_id));
            resize_scale.set(f64::from(window.scale_factor()).max(1.0));
        });
        if kind == WindowKind::Main {
            let move_proxy = event_loop_proxy.clone();
            gtk_window.connect_configure_event(move |_, _| {
                let _ = move_proxy.send_event(AppEvent::HostWindowMoved(window_id));
                false
            });
        }

        let layer_shell = if backend == LinuxWindowBackend::WaylandLayerShell {
            match layer_shell_api {
                Some(layer_shell) => match layer_shell.configure(&gtk_window, placement) {
                    Ok(()) => Some(layer_shell),
                    Err(error) => {
                        backend = LinuxWindowBackend::WaylandNormal;
                        fallback_reason = Some(format!(
                            "layer-shell configuration failed ({error}); using a regular Wayland window"
                        ));
                        None
                    }
                },
                None => {
                    backend = LinuxWindowBackend::WaylandNormal;
                    fallback_reason = Some(
                        "layer-shell was selected but its library is unavailable; using a regular Wayland window"
                            .to_string(),
                    );
                    None
                }
            }
        } else {
            None
        };

        gtk_window.show_all();
        scale_factor.set(f64::from(gtk_window.scale_factor()).max(1.0));
        if backend == LinuxWindowBackend::X11 && kind == WindowKind::Main {
            if let Some(position) = initial_position {
                gtk_window.move_(position.x, position.y);
            }
            apply_x11_window_properties(&gtk_window);
        }
        let diagnostics = LinuxDiagnostics {
            session_backend: session_backend_label(session).to_string(),
            window_backend: linux_backend_label(backend).to_string(),
            requested_mode: linux_mode_label(requested_mode).to_string(),
            overlay_supported,
            fallback_reason,
        };
        Ok(Self {
            winit_window,
            gtk_window,
            container,
            logical_size,
            physical_position,
            native_pointer_press,
            scale_factor,
            kind,
            backend,
            available_monitors,
            diagnostics,
            layer_shell,
        })
    }

    pub(crate) fn id(&self) -> WindowId {
        self.winit_window.id()
    }

    pub(crate) fn inner_size(&self) -> PhysicalSize<u32> {
        self.logical_size
            .get()
            .to_physical(self.scale_factor.get().max(0.01))
    }

    pub(crate) fn scale_factor(&self) -> f64 {
        self.scale_factor.get()
    }

    pub(crate) fn outer_position(&self) -> Result<PhysicalPosition<i32>, String> {
        if self.backend == LinuxWindowBackend::X11 {
            if let Some(gdk_window) = self.gtk_window.window() {
                let (result, x, y) = gdk_window.origin();
                if result != 0 {
                    self.physical_position
                        .set(Some(PhysicalPosition::new(x, y)));
                }
            }
        }
        self.physical_position
            .get()
            .ok_or_else(|| "Wayland does not expose a global window position".to_string())
    }

    pub(crate) fn current_monitor(&self) -> Option<winit::monitor::MonitorHandle> {
        if let Some(gdk_monitor) = self.current_gdk_monitor() {
            let geometry = gdk_monitor.geometry();
            let model = gdk_monitor.model().map(|model| model.to_string());
            if let Some(monitor) = self.available_monitors.iter().find(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (position.x == geometry.x()
                    && position.y == geometry.y()
                    && size.width == geometry.width().max(0) as u32
                    && size.height == geometry.height().max(0) as u32)
                    || model
                        .as_deref()
                        .is_some_and(|model| monitor.name().as_deref() == Some(model))
            }) {
                return Some(monitor.clone());
            }
        }
        self.winit_window.current_monitor()
    }

    fn current_gdk_monitor(&self) -> Option<gtk::gdk::Monitor> {
        let gdk_window = self.gtk_window.window()?;
        self.gtk_window.display().monitor_at_window(&gdk_window)
    }

    pub(crate) fn set_resizable(&self, resizable: bool) {
        self.gtk_window.set_resizable(resizable);
    }

    pub(crate) fn set_min_inner_size<S>(&self, size: Option<S>)
    where
        S: Into<Size>,
    {
        if let Some(size) = size.map(Into::into).and_then(logical_size_from_size) {
            self.gtk_window.set_size_request(
                size.width.round().max(1.0) as i32,
                size.height.round().max(1.0) as i32,
            );
        }
    }

    pub(crate) fn set_max_inner_size<S>(&self, _size: Option<S>)
    where
        S: Into<Size>,
    {
    }

    pub(crate) fn request_inner_size<S>(&self, size: S) -> Option<PhysicalSize<u32>>
    where
        S: Into<Size>,
    {
        let size = logical_size_from_size(size.into())?;
        self.logical_size.set(size);
        self.container.set_size_request(
            size.width.round().max(1.0) as i32,
            size.height.round().max(1.0) as i32,
        );
        self.gtk_window.resize(
            size.width.round().max(1.0) as i32,
            size.height.round().max(1.0) as i32,
        );
        Some(self.inner_size())
    }

    pub(crate) fn set_outer_position<P>(&self, position: P)
    where
        P: Into<Position>,
    {
        let position = match position.into() {
            Position::Physical(position) => position,
            Position::Logical(position) => position.to_physical(self.scale_factor()),
        };
        if matches!(
            self.backend,
            LinuxWindowBackend::WaylandNormal | LinuxWindowBackend::WaylandLayerShell
        ) {
            return;
        }
        self.physical_position.set(Some(position));
        self.gtk_window.move_(position.x, position.y);
    }

    pub(crate) fn set_visible(&self, visible: bool) {
        if visible {
            self.gtk_window.show_all();
        } else {
            self.gtk_window.hide();
        }
    }

    pub(crate) fn focus_window(&self) {
        self.gtk_window.present();
        self.gtk_window.grab_focus();
    }

    pub(crate) fn begin_native_drag(&self) -> bool {
        if self.backend != LinuxWindowBackend::WaylandNormal {
            return false;
        }
        let Some(press) = self.native_pointer_press.take() else {
            return false;
        };
        self.gtk_window
            .begin_move_drag(1, press.root_x, press.root_y, press.timestamp);
        true
    }

    pub(crate) fn refresh_layer_shell_monitor(&self) {
        if self.backend != LinuxWindowBackend::WaylandLayerShell {
            return;
        }
        let Some(layer_shell) = self.layer_shell.as_ref() else {
            return;
        };
        layer_shell.set_monitor(&self.gtk_window, self.current_gdk_monitor().as_ref());
    }

    pub(crate) fn set_cursor_hittest(&self, _enabled: bool) -> Result<(), String> {
        Err("cursor hit testing is unavailable on Linux".to_string())
    }

    pub(crate) fn gtk_window(&self) -> &gtk::Window {
        &self.gtk_window
    }

    pub(crate) fn gtk_container(&self) -> &gtk::Fixed {
        &self.container
    }

    pub(crate) fn diagnostics(&self) -> LinuxDiagnostics {
        self.diagnostics.clone()
    }

    #[allow(dead_code)]
    pub(crate) fn winit_window(&self) -> &Window {
        &self.winit_window
    }

    #[allow(dead_code)]
    pub(crate) fn has_layer_shell(&self) -> bool {
        self.layer_shell.is_some()
    }
}

pub(crate) fn configure_created_window(window: &HostWindow) {
    if window.kind == WindowKind::Main && window.backend == LinuxWindowBackend::X11 {
        apply_x11_window_properties(window.gtk_window());
    }
}

fn init_gtk() -> Result<(), String> {
    gtk::glib::set_prgname(Some(LINUX_APPLICATION_ID));
    if gtk::is_initialized() {
        return Ok(());
    }
    gtk::init().map_err(|error| format!("failed to initialize GTK: {error}"))
}

fn logical_size_from_size(size: Size) -> Option<LogicalSize<f64>> {
    match size {
        Size::Logical(size) => Some(size),
        Size::Physical(size) => Some(size.to_logical(1.0)),
    }
}

fn session_backend(window: &Window) -> LinuxSessionBackend {
    let Ok(display) = window.display_handle() else {
        return LinuxSessionBackend::Unknown;
    };
    match display.as_raw() {
        RawDisplayHandle::Xlib(_) => LinuxSessionBackend::X11,
        RawDisplayHandle::Wayland(_) => LinuxSessionBackend::Wayland,
        _ => LinuxSessionBackend::Unknown,
    }
}

fn session_backend_label(backend: LinuxSessionBackend) -> &'static str {
    match backend {
        LinuxSessionBackend::X11 => "x11",
        LinuxSessionBackend::Wayland => "wayland",
        LinuxSessionBackend::Unknown => "unknown",
    }
}

fn linux_backend_label(backend: LinuxWindowBackend) -> &'static str {
    match backend {
        LinuxWindowBackend::X11 => "x11_ewmh",
        LinuxWindowBackend::WaylandNormal => "wayland_normal",
        LinuxWindowBackend::WaylandLayerShell => "wayland_layer_shell",
    }
}

fn linux_mode_label(mode: LinuxWindowMode) -> &'static str {
    match mode {
        LinuxWindowMode::Auto => "auto",
        LinuxWindowMode::NormalWindow => "normal_window",
        LinuxWindowMode::Overlay => "overlay",
    }
}

fn layer_shell_drag_verified() -> bool {
    // Keep the optional protocol behind the compositor smoke matrix until
    // native movement, output changes, resize, and fallback are verified.
    false
}

pub(crate) fn build_webview(
    window: &HostWindow,
    html: &str,
    init_script: Option<&str>,
    transparent: bool,
    event_loop_proxy: &EventLoopProxy<AppEvent>,
) -> Result<WebView, String> {
    let ipc_proxy = event_loop_proxy.clone();
    let mut builder = WebViewBuilder::new().with_html(html).with_ipc_handler(
        move |request: wry::http::Request<String>| {
            let _ = ipc_proxy.send_event(AppEvent::Ipc(request.into_body()));
        },
    );
    if transparent {
        builder = builder.with_transparent(true);
    }
    if let Some(init_script) = init_script {
        builder = builder.with_initialization_script(init_script);
    }
    let size = window.logical_size.get();
    builder = builder.with_bounds(Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: LogicalSize::new(size.width, size.height).into(),
    });
    let webview = builder
        .build_gtk(window.gtk_container())
        .map_err(|error| error.to_string())?;
    let native_pointer_press = window.native_pointer_press.clone();
    let gtk_webview = webview.webview();
    gtk_webview.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK);
    gtk_webview.connect_button_press_event(move |_, event| {
        if event.button() == 1 {
            let (root_x, root_y) = event.root();
            native_pointer_press.set(Some(NativePointerPress {
                root_x: root_x.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32,
                root_y: root_y.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32,
                timestamp: event.time(),
            }));
        }
        gtk::glib::Propagation::Proceed
    });
    Ok(webview)
}

pub(crate) fn diagnostics(window: Option<&HostWindow>) -> LinuxDiagnostics {
    window
        .map(HostWindow::diagnostics)
        .unwrap_or_else(|| LinuxDiagnostics {
            session_backend: "not_started".to_string(),
            window_backend: "not_started".to_string(),
            requested_mode: "auto".to_string(),
            overlay_supported: false,
            fallback_reason: None,
        })
}

fn apply_x11_window_properties(window: &gtk::Window) {
    let Some(gdk_window) = window.window() else {
        return;
    };
    let Ok(x11_window) = gdk_window.downcast::<gdkx11::X11Window>() else {
        return;
    };
    let xid = x11_window.xid() as u32;
    let Ok((connection, _)) = x11rb::connect(None) else {
        return;
    };
    let Ok(window_type) = intern_atom(&connection, b"_NET_WM_WINDOW_TYPE") else {
        return;
    };
    let Ok(utility) = intern_atom(&connection, b"_NET_WM_WINDOW_TYPE_UTILITY") else {
        return;
    };
    let Ok(state) = intern_atom(&connection, b"_NET_WM_STATE") else {
        return;
    };
    let Ok(above) = intern_atom(&connection, b"_NET_WM_STATE_ABOVE") else {
        return;
    };
    let Ok(sticky) = intern_atom(&connection, b"_NET_WM_STATE_STICKY") else {
        return;
    };
    let Ok(skip_taskbar) = intern_atom(&connection, b"_NET_WM_STATE_SKIP_TASKBAR") else {
        return;
    };
    let Ok(skip_pager) = intern_atom(&connection, b"_NET_WM_STATE_SKIP_PAGER") else {
        return;
    };
    let Ok(desktop) = intern_atom(&connection, b"_NET_WM_DESKTOP") else {
        return;
    };
    let atom_type: u32 = x11rb::protocol::xproto::AtomEnum::ATOM.into();
    let cardinal_type: u32 = x11rb::protocol::xproto::AtomEnum::CARDINAL.into();
    let _ = connection.change_property32(
        x11rb::protocol::xproto::PropMode::REPLACE,
        xid,
        window_type,
        atom_type,
        &[utility],
    );
    let _ = connection.change_property32(
        x11rb::protocol::xproto::PropMode::REPLACE,
        xid,
        state,
        atom_type,
        &[above, sticky, skip_taskbar, skip_pager],
    );
    let _ = connection.change_property32(
        x11rb::protocol::xproto::PropMode::REPLACE,
        xid,
        desktop,
        cardinal_type,
        &[u32::MAX],
    );
    let _ = connection.flush();
}

fn intern_atom<C>(connection: &C, name: &[u8]) -> Result<u32, String>
where
    C: x11rb::connection::Connection,
{
    connection
        .intern_atom(false, name)
        .map_err(|error| error.to_string())?
        .reply()
        .map(|reply| reply.atom)
        .map_err(|error| error.to_string())
}

fn gdk_monitor_for_placement(
    window: &gtk::Window,
    placement: &LinuxOutputPlacement,
) -> Option<gtk::gdk::Monitor> {
    let display = window.display();
    let monitors = (0..display.n_monitors())
        .filter_map(|index| display.monitor(index))
        .collect::<Vec<_>>();
    if let Some(name) = placement.output_name.as_deref() {
        if let Some(monitor) = monitors
            .iter()
            .find(|monitor| monitor.model().is_some_and(|model| model.as_str() == name))
        {
            return Some(monitor.clone());
        }
    }
    monitors
        .iter()
        .find(|monitor| {
            let geometry = monitor.geometry();
            geometry.width().max(0) as u32 == placement.output.width
                && geometry.height().max(0) as u32 == placement.output.height
                && (f64::from(monitor.scale_factor()) - placement.output.scale_factor).abs() < 0.01
        })
        .cloned()
        .or_else(|| display.primary_monitor())
}

fn gdk_monitor_ptr(monitor: Option<&gtk::gdk::Monitor>) -> *mut c_void {
    monitor
        .map(|monitor| {
            <gtk::gdk::Monitor as ToGlibPtr<*mut gtk::gdk::ffi::GdkMonitor>>::to_glib_none(monitor)
                .0
                .cast()
        })
        .unwrap_or(std::ptr::null_mut())
}

struct LayerShellApi {
    handle: *mut c_void,
    is_supported_fn: unsafe extern "C" fn() -> i32,
    init_for_window_fn: unsafe extern "C" fn(*mut c_void),
    set_monitor_fn: unsafe extern "C" fn(*mut c_void, *mut c_void),
    set_layer_fn: unsafe extern "C" fn(*mut c_void, i32),
    set_anchor_fn: unsafe extern "C" fn(*mut c_void, i32, i32),
    set_exclusive_zone_fn: unsafe extern "C" fn(*mut c_void, i32),
    set_keyboard_interactivity_fn: unsafe extern "C" fn(*mut c_void, i32),
    set_margin_fn: unsafe extern "C" fn(*mut c_void, i32, i32),
}

impl LayerShellApi {
    fn load() -> Option<Self> {
        let handle = [
            b"libgtk-layer-shell.so.0\0".as_slice(),
            b"libgtk-layer-shell.so\0",
        ]
        .iter()
        .find_map(|name| unsafe {
            let handle = libc::dlopen(name.as_ptr().cast(), libc::RTLD_LAZY | libc::RTLD_LOCAL);
            (!handle.is_null()).then_some(handle)
        })?;
        let (
            Some(is_supported_fn),
            Some(init_for_window_fn),
            Some(set_monitor_fn),
            Some(set_layer_fn),
            Some(set_anchor_fn),
            Some(set_exclusive_zone_fn),
            Some(set_keyboard_interactivity_fn),
            Some(set_margin_fn),
        ) = (unsafe {
            (
                load_symbol::<unsafe extern "C" fn() -> i32>(handle, b"gtk_layer_is_supported\0"),
                load_symbol::<unsafe extern "C" fn(*mut c_void)>(
                    handle,
                    b"gtk_layer_init_for_window\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, *mut c_void)>(
                    handle,
                    b"gtk_layer_set_monitor\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, i32)>(
                    handle,
                    b"gtk_layer_set_layer\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, i32, i32)>(
                    handle,
                    b"gtk_layer_set_anchor\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, i32)>(
                    handle,
                    b"gtk_layer_set_exclusive_zone\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, i32)>(
                    handle,
                    b"gtk_layer_set_keyboard_interactivity\0",
                ),
                load_symbol::<unsafe extern "C" fn(*mut c_void, i32, i32)>(
                    handle,
                    b"gtk_layer_set_margin\0",
                ),
            )
        })
        else {
            unsafe {
                libc::dlclose(handle);
            }
            return None;
        };
        Some(Self {
            handle,
            is_supported_fn,
            init_for_window_fn,
            set_monitor_fn,
            set_layer_fn,
            set_anchor_fn,
            set_exclusive_zone_fn,
            set_keyboard_interactivity_fn,
            set_margin_fn,
        })
    }

    fn is_supported(&self) -> bool {
        unsafe { (self.is_supported_fn)() != 0 }
    }

    fn configure(
        &self,
        window: &gtk::Window,
        placement: Option<&LinuxOutputPlacement>,
    ) -> Result<(), String> {
        if !self.is_supported() {
            return Err("the compositor does not support gtk-layer-shell".to_string());
        }
        let ptr: *mut c_void =
            <gtk::Window as ToGlibPtr<*mut gtk::ffi::GtkWindow>>::to_glib_none(window)
                .0
                .cast();
        let placement = placement.cloned().unwrap_or(LinuxOutputPlacement {
            output_name: None,
            output: downshift::PersistedMonitor {
                width: 1,
                height: 1,
                scale_factor: 1.0,
            },
            anchor: downshift::LinuxWindowAnchor::TopRight,
            margin_x: DEFAULT_MARGIN,
            margin_y: DEFAULT_MARGIN,
        });
        let monitor = gdk_monitor_for_placement(window, &placement);
        let (left, right, top, bottom) = match placement.anchor {
            downshift::LinuxWindowAnchor::TopLeft => (true, false, true, false),
            downshift::LinuxWindowAnchor::TopRight => (false, true, true, false),
            downshift::LinuxWindowAnchor::BottomLeft => (true, false, false, true),
            downshift::LinuxWindowAnchor::BottomRight => (false, true, false, true),
        };
        unsafe {
            (self.init_for_window_fn)(ptr);
            (self.set_monitor_fn)(ptr, gdk_monitor_ptr(monitor.as_ref()));
            (self.set_layer_fn)(ptr, 3);
            (self.set_anchor_fn)(ptr, 0, i32::from(left));
            (self.set_anchor_fn)(ptr, 1, i32::from(right));
            (self.set_anchor_fn)(ptr, 2, i32::from(top));
            (self.set_anchor_fn)(ptr, 3, i32::from(bottom));
            let horizontal_edge = if right { 1 } else { 0 };
            let vertical_edge = if top { 2 } else { 3 };
            (self.set_margin_fn)(ptr, horizontal_edge, placement.margin_x.max(0));
            (self.set_margin_fn)(ptr, vertical_edge, placement.margin_y.max(0));
            (self.set_exclusive_zone_fn)(ptr, -1);
            (self.set_keyboard_interactivity_fn)(ptr, 0);
        }
        Ok(())
    }

    fn set_monitor(&self, window: &gtk::Window, monitor: Option<&gtk::gdk::Monitor>) {
        let ptr: *mut c_void =
            <gtk::Window as ToGlibPtr<*mut gtk::ffi::GtkWindow>>::to_glib_none(window)
                .0
                .cast();
        unsafe {
            (self.set_monitor_fn)(ptr, gdk_monitor_ptr(monitor));
        }
    }
}

impl Drop for LayerShellApi {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.handle);
        }
    }
}

unsafe fn load_symbol<T: Copy>(handle: *mut c_void, name: &[u8]) -> Option<T> {
    let symbol = libc::dlsym(handle, name.as_ptr().cast());
    (!symbol.is_null()).then(|| std::mem::transmute_copy(&symbol))
}
