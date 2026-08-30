pub(crate) mod common;
pub(crate) mod instance;
pub(crate) mod launch_at_login;
pub(crate) mod menu;
pub(crate) mod monitor;
pub(crate) mod platform;
pub(crate) mod tray;
pub(crate) mod window;

pub(crate) use common::{
    clear_child_window, create_fixed_child_window, focus_existing_child_window,
    sync_child_webview_bounds, sync_main_webview_bounds,
};
pub(crate) use instance::{start as start_instance, InstanceStart};
pub(crate) use launch_at_login::{reconcile_launch_at_login, set_launch_at_login};
pub(crate) use monitor::{persisted_monitor, snapshot_monitor};
pub(crate) use platform::{
    build_main_webview, configure_created_window, configure_event_loop_builder,
    configure_main_window, copy_text_to_clipboard, create_main_window, current_os_version,
    native_menu_available, open_external_url,
};

pub(crate) use menu::{install_event_handler as install_menu_event_handler, NativeContextMenu};
pub(crate) use tray::{
    create_tray_icon, install_event_handler as install_tray_event_handler,
    update_menu as update_tray_menu, TrayIconHandle,
};
pub(crate) use window::{
    enforce_fixed_size, logical_outer_position, set_outer_position, set_outer_position_physical,
    set_visible, show_without_focus,
};
