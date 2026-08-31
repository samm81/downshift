use super::platform::HostWindow;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use muda::dpi::PhysicalPosition as MenuPhysicalPosition;
#[cfg(target_os = "windows")]
use muda::Menu;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use muda::{
    CheckMenuItem, ContextMenu, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use winit::event_loop::EventLoopProxy;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use crate::app_core::breathing_pattern_summary;
use crate::app_core::{AppEvent, SNOOZE_PRESET_MINUTES};
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use crate::diagnostics;
#[cfg(any(
    debug_assertions,
    not(any(target_os = "macos", target_os = "windows", target_os = "linux"))
))]
use crate::update_check::UpdateCheckService;
use downshift::Settings;
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use downshift::{built_in_breathing_presets, BreathingPattern, BREATHING_PRESET_ID_COHERENT};

pub(crate) const MENU_ID_PAUSE: &str = "pause";
pub(crate) const MENU_ID_FOLLOW_CURSOR: &str = "follow_cursor";
pub(crate) const MENU_ID_SNOOZE_ROOT: &str = "snooze_root";
pub(crate) const MENU_ID_SNOOZE_5: &str = "snooze_5";
pub(crate) const MENU_ID_SNOOZE_10: &str = "snooze_10";
pub(crate) const MENU_ID_SNOOZE_15: &str = "snooze_15";
pub(crate) const MENU_ID_SNOOZE_30: &str = "snooze_30";
pub(crate) const MENU_ID_SNOOZE_60: &str = "snooze_60";
pub(crate) const MENU_ID_SNOOZE_CUSTOM: &str = "snooze_custom";
pub(crate) const MENU_ID_SIZE_S: &str = "size_s";
pub(crate) const MENU_ID_SIZE_M: &str = "size_m";
pub(crate) const MENU_ID_SIZE_L: &str = "size_l";
pub(crate) const MENU_ID_SIZE_XL: &str = "size_xl";
pub(crate) const MENU_ID_BREATHING_PATTERN: &str = "breathing_pattern";
pub(crate) const MENU_ID_BREATHING_COHERENT: &str = "breathing_coherent";
pub(crate) const MENU_ID_BREATHING_BOX: &str = "breathing_box";
pub(crate) const MENU_ID_BREATHING_479: &str = "breathing_479";
pub(crate) const MENU_ID_BREATHING_EDIT: &str = "breathing_edit";
pub(crate) const MENU_ID_BREATHING_DELETE_ROOT: &str = "breathing_delete_root";
pub(crate) const MENU_ID_BREATHING_DELETE_PREFIX: &str = "breathing_delete:";
pub(crate) const MENU_ID_BREATHING_SAVED_PREFIX: &str = "breathing_saved:";
pub(crate) const MENU_ID_RESET: &str = "reset";
pub(crate) const MENU_ID_QUIT: &str = "quit";
pub(crate) const MENU_ID_ANALYTICS_ROOT: &str = "analytics_root";
pub(crate) const MENU_ID_USAGE_ON: &str = "usage_on";
pub(crate) const MENU_ID_USAGE_OFF: &str = "usage_off";
pub(crate) const MENU_ID_CRASH_ON: &str = "crash_on";
pub(crate) const MENU_ID_CRASH_OFF: &str = "crash_off";
pub(crate) const MENU_ID_ANALYTICS_INFO: &str = "analytics_info";
pub(crate) const MENU_ID_UPDATE_ROOT: &str = "update_root";
pub(crate) const MENU_ID_UPDATE_PRIMARY: &str = "update_primary";
pub(crate) const MENU_ID_UPDATE_IGNORE_CURRENT: &str = "update_ignore_current";
pub(crate) const MENU_ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
pub(crate) const MENU_ID_BUGS_ROOT: &str = "bugs_root";
pub(crate) const MENU_ID_COPY_DIAGNOSTICS: &str = "copy_diagnostics";
pub(crate) const MENU_ID_FILE_BUG_GITHUB: &str = "file_bug_github";
pub(crate) const MENU_ID_FILE_BUG_EMAIL: &str = "file_bug_email";
#[cfg(debug_assertions)]
pub(crate) const MENU_ID_DEVELOPER_PREVIEWS_ROOT: &str = "developer_previews_root";
#[cfg(debug_assertions)]
pub(crate) const MENU_ID_SIMULATE_PENDING_UPDATE: &str = "simulate_pending_update";
#[cfg(debug_assertions)]
pub(crate) const MENU_ID_FORCE_BACKGROUND_UPDATE_CHECK: &str = "force_background_update_check";
#[cfg(debug_assertions)]
pub(crate) const MENU_ID_CLEAR_UPDATE_NOTIFICATION_DISMISSED: &str =
    "clear_update_notification_dismissed";

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
macro_rules! log_stderr {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        diagnostics::log_line("ERROR", &message);
    }};
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[derive(Clone)]
pub(crate) struct NativeContextMenu {
    root: Submenu,
    pause: CheckMenuItem,
    follow_cursor: CheckMenuItem,
    launch_at_login: CheckMenuItem,
    size_menu: Submenu,
    size_s: MenuItem,
    size_m: MenuItem,
    size_l: MenuItem,
    size_xl: MenuItem,
    size_scroll_hint: MenuItem,
    breathing_menu: Submenu,
    breathing_coherent: CheckMenuItem,
    breathing_box: CheckMenuItem,
    breathing_479: CheckMenuItem,
    breathing_saved: Vec<(String, CheckMenuItem)>,
    breathing_delete_menu: Submenu,
    breathing_delete_items: Vec<(String, MenuItem)>,
    update_primary: MenuItem,
    update_ignore_current: CheckMenuItem,
    usage_on: CheckMenuItem,
    usage_off: CheckMenuItem,
    crash_on: CheckMenuItem,
    crash_off: CheckMenuItem,
    #[cfg(debug_assertions)]
    simulate_pending_update: CheckMenuItem,
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
impl NativeContextMenu {
    pub(crate) fn new(settings: &Settings) -> Option<Self> {
        let visible_built_in_presets = built_in_breathing_presets()
            .into_iter()
            .filter(|preset| {
                !settings
                    .hidden_breathing_preset_ids
                    .iter()
                    .any(|id| id == preset.id)
            })
            .collect::<Vec<_>>();
        let pause = CheckMenuItem::with_id(MENU_ID_PAUSE, "paused", true, false, None);
        let follow_cursor =
            CheckMenuItem::with_id(MENU_ID_FOLLOW_CURSOR, "follow cursor", true, false, None);
        let launch_at_login =
            CheckMenuItem::with_id(MENU_ID_LAUNCH_AT_LOGIN, "start at login", true, true, None);
        let snooze_5 = MenuItem::with_id(MENU_ID_SNOOZE_5, "snooze for 5 minutes", true, None);
        let snooze_10 = MenuItem::with_id(MENU_ID_SNOOZE_10, "snooze for 10 minutes", true, None);
        let snooze_15 = MenuItem::with_id(MENU_ID_SNOOZE_15, "snooze for 15 minutes", true, None);
        let snooze_30 = MenuItem::with_id(MENU_ID_SNOOZE_30, "snooze for 30 minutes", true, None);
        let snooze_60 = MenuItem::with_id(MENU_ID_SNOOZE_60, "snooze for 60 minutes", true, None);
        let snooze_custom = MenuItem::with_id(
            MENU_ID_SNOOZE_CUSTOM,
            "snooze for custom minutes…",
            true,
            None,
        );
        let size_s = MenuItem::with_id(MENU_ID_SIZE_S, "S (64px)", true, None);
        let size_m = MenuItem::with_id(MENU_ID_SIZE_M, "M (96px)", true, None);
        let size_l = MenuItem::with_id(MENU_ID_SIZE_L, "L (128px)", true, None);
        let size_xl = MenuItem::with_id(MENU_ID_SIZE_XL, "XL (160px)", true, None);
        let size_scroll_hint = MenuItem::with_id(
            "size_scroll_hint",
            "tip: scroll the ball to resize",
            false,
            None,
        );
        let breathing_coherent = CheckMenuItem::with_id(
            MENU_ID_BREATHING_COHERENT,
            format!(
                "coherent breathing ({})",
                breathing_pattern_summary(&BreathingPattern::coherent())
            ),
            visible_built_in_presets
                .iter()
                .any(|preset| preset.id == BREATHING_PRESET_ID_COHERENT),
            false,
            None,
        );
        let breathing_box = CheckMenuItem::with_id(
            MENU_ID_BREATHING_BOX,
            format!(
                "box breathing ({})",
                breathing_pattern_summary(&BreathingPattern::box_breathing())
            ),
            visible_built_in_presets
                .iter()
                .any(|preset| preset.id == "box_breathing"),
            false,
            None,
        );
        let breathing_479 = CheckMenuItem::with_id(
            MENU_ID_BREATHING_479,
            format!(
                "4-7-9 ({})",
                breathing_pattern_summary(&BreathingPattern::four_seven_nine())
            ),
            visible_built_in_presets
                .iter()
                .any(|preset| preset.id == "4_7_9"),
            false,
            None,
        );
        let breathing_edit = MenuItem::with_id(MENU_ID_BREATHING_EDIT, "add new…", true, None);
        let breathing_saved = settings
            .saved_breathing_presets
            .iter()
            .map(|preset| {
                (
                    preset.id.clone(),
                    CheckMenuItem::with_id(
                        breathing_saved_menu_id(&preset.id),
                        format!(
                            "{} ({})",
                            preset.name,
                            breathing_pattern_summary(&preset.pattern)
                        ),
                        true,
                        false,
                        None,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let breathing_delete_items = visible_built_in_presets
            .iter()
            .map(|preset| {
                (
                    preset.id.to_string(),
                    MenuItem::with_id(
                        breathing_delete_menu_id(preset.id),
                        format!(
                            "{} ({})",
                            preset.name,
                            breathing_pattern_summary(&preset.pattern)
                        ),
                        true,
                        None,
                    ),
                )
            })
            .chain(settings.saved_breathing_presets.iter().map(|preset| {
                (
                    preset.id.clone(),
                    MenuItem::with_id(
                        breathing_delete_menu_id(&preset.id),
                        format!(
                            "{} ({})",
                            preset.name,
                            breathing_pattern_summary(&preset.pattern)
                        ),
                        true,
                        None,
                    ),
                )
            }))
            .collect::<Vec<_>>();
        let reset = MenuItem::with_id(MENU_ID_RESET, "reset", true, None);
        let quit = MenuItem::with_id(MENU_ID_QUIT, "quit", true, None);
        let update_primary = MenuItem::with_id(
            MENU_ID_UPDATE_PRIMARY,
            format!("check for updates (version {})", env!("CARGO_PKG_VERSION")),
            true,
            None,
        );
        let update_ignore_current = CheckMenuItem::with_id(
            MENU_ID_UPDATE_IGNORE_CURRENT,
            "do not remind me about the current update again",
            false,
            false,
            None,
        );
        let copy_diagnostics =
            MenuItem::with_id(MENU_ID_COPY_DIAGNOSTICS, "copy diagnostics", true, None);
        let file_bug_github = MenuItem::with_id(
            MENU_ID_FILE_BUG_GITHUB,
            "file a bug report on github",
            true,
            None,
        );
        let file_bug_email = MenuItem::with_id(
            MENU_ID_FILE_BUG_EMAIL,
            "file a bug report by email",
            true,
            None,
        );
        let usage_on = CheckMenuItem::with_id(
            MENU_ID_USAGE_ON,
            "share anonymous usage data",
            true,
            false,
            None,
        );
        let usage_off = CheckMenuItem::with_id(
            MENU_ID_USAGE_OFF,
            "don’t share usage data",
            true,
            false,
            None,
        );
        let crash_on = CheckMenuItem::with_id(
            MENU_ID_CRASH_ON,
            "share anonymous crash reports",
            true,
            false,
            None,
        );
        let crash_off = CheckMenuItem::with_id(
            MENU_ID_CRASH_OFF,
            "don't share crash reports",
            true,
            false,
            None,
        );
        let analytics_info =
            MenuItem::with_id(MENU_ID_ANALYTICS_INFO, "what we collect…", true, None);
        #[cfg(debug_assertions)]
        let simulate_pending_update = CheckMenuItem::with_id(
            MENU_ID_SIMULATE_PENDING_UPDATE,
            "simulate pending update",
            true,
            false,
            None,
        );
        #[cfg(debug_assertions)]
        let force_background_update_check = MenuItem::with_id(
            MENU_ID_FORCE_BACKGROUND_UPDATE_CHECK,
            "force background update check",
            true,
            None,
        );
        #[cfg(debug_assertions)]
        let clear_update_notification_dismissed = MenuItem::with_id(
            MENU_ID_CLEAR_UPDATE_NOTIFICATION_DISMISSED,
            "clear update notification dismissed",
            true,
            None,
        );
        #[cfg(debug_assertions)]
        let developer_previews_menu = match Submenu::with_id_and_items(
            MENU_ID_DEVELOPER_PREVIEWS_ROOT,
            "developer previews",
            true,
            &[
                &simulate_pending_update,
                &force_background_update_check,
                &clear_update_notification_dismissed,
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build developer previews submenu: {error}");
                return None;
            }
        };
        let analytics_separator_one = PredefinedMenuItem::separator();
        let analytics_separator_two = PredefinedMenuItem::separator();
        let snooze_submenu = match Submenu::with_id_and_items(
            MENU_ID_SNOOZE_ROOT,
            "snooze",
            true,
            &[
                &snooze_5,
                &snooze_10,
                &snooze_15,
                &snooze_30,
                &snooze_60,
                &snooze_custom,
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build snooze submenu: {error}");
                return None;
            }
        };
        let analytics_menu = match Submenu::with_id_and_items(
            MENU_ID_ANALYTICS_ROOT,
            "help improve downshift",
            true,
            &[
                &usage_on,
                &usage_off,
                &analytics_separator_one,
                &crash_on,
                &crash_off,
                &analytics_separator_two,
                &analytics_info,
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build analytics submenu: {error}");
                return None;
            }
        };
        let bugs_menu = match Submenu::with_id_and_items(
            MENU_ID_BUGS_ROOT,
            "bugs",
            true,
            &[&copy_diagnostics, &file_bug_github, &file_bug_email],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build bugs submenu: {error}");
                return None;
            }
        };
        let update_menu = match Submenu::with_id_and_items(
            MENU_ID_UPDATE_ROOT,
            "updates",
            true,
            &[&update_primary, &update_ignore_current],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build update submenu: {error}");
                return None;
            }
        };
        let size_separator = PredefinedMenuItem::separator();
        let size_submenu = match Submenu::with_items(
            "size",
            true,
            &[
                &size_s,
                &size_m,
                &size_l,
                &size_xl,
                &size_separator,
                &size_scroll_hint,
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build size submenu: {error}");
                return None;
            }
        };
        let breathing_separator = PredefinedMenuItem::separator();
        let mut breathing_items: Vec<&dyn IsMenuItem> =
            vec![&breathing_coherent, &breathing_box, &breathing_479];
        if !breathing_saved.is_empty() {
            for (_, item) in &breathing_saved {
                breathing_items.push(item);
            }
        }
        breathing_items.push(&breathing_separator);
        breathing_items.push(&breathing_edit);
        let delete_items = breathing_delete_items
            .iter()
            .map(|(_, item)| item as &dyn IsMenuItem)
            .collect::<Vec<_>>();
        let breathing_delete_menu = match Submenu::with_id_and_items(
            MENU_ID_BREATHING_DELETE_ROOT,
            "delete",
            !delete_items.is_empty(),
            &delete_items,
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build breathing delete submenu: {error}");
                return None;
            }
        };
        breathing_items.push(&breathing_delete_menu);
        let breathing_menu = match Submenu::with_id_and_items(
            MENU_ID_BREATHING_PATTERN,
            "breathing pattern",
            true,
            &breathing_items,
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build breathing submenu: {error}");
                return None;
            }
        };
        let separator_one = PredefinedMenuItem::separator();
        let separator_two = PredefinedMenuItem::separator();
        let separator_three = PredefinedMenuItem::separator();
        let separator_four = PredefinedMenuItem::separator();
        let separator_five = PredefinedMenuItem::separator();
        let separator_six = PredefinedMenuItem::separator();
        let separator_seven = PredefinedMenuItem::separator();
        let separator_pattern = PredefinedMenuItem::separator();
        #[cfg(debug_assertions)]
        let separator_developer_previews = PredefinedMenuItem::separator();
        #[allow(unused_mut)]
        let mut root_items: Vec<&dyn IsMenuItem> = vec![
            &pause,
            &follow_cursor,
            &separator_one,
            &snooze_submenu,
            &separator_two,
            &size_submenu,
            &separator_three,
            &breathing_menu,
            &separator_pattern,
            &reset,
            &launch_at_login,
            &separator_four,
            &quit,
            &separator_five,
            &update_menu,
            &separator_six,
            &bugs_menu,
            &separator_seven,
            &analytics_menu,
        ];
        #[cfg(debug_assertions)]
        {
            root_items.push(&separator_developer_previews);
            root_items.push(&developer_previews_menu);
        }
        let root = match Submenu::with_items("menu", true, &root_items) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build native context menu: {error}");
                return None;
            }
        };
        Some(Self {
            root,
            pause,
            follow_cursor,
            launch_at_login,
            size_menu: size_submenu,
            size_s,
            size_m,
            size_l,
            size_xl,
            size_scroll_hint,
            breathing_menu,
            breathing_coherent,
            breathing_box,
            breathing_479,
            breathing_saved,
            breathing_delete_menu,
            breathing_delete_items,
            update_primary,
            update_ignore_current,
            usage_on,
            usage_off,
            crash_on,
            crash_off,
            #[cfg(debug_assertions)]
            simulate_pending_update,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync_from_settings(
        &self,
        settings: &Settings,
        size_presets: [f64; 4],
        update_label: &str,
        update_ignore_enabled: bool,
        update_ignore_checked: bool,
        follow_cursor_active: bool,
        follow_cursor_available: bool,
        follow_cursor_unavailable_reason: &str,
    ) {
        self.pause.set_checked(settings.paused);
        self.pause
            .set_text(if settings.paused { "paused" } else { "pause" });
        self.follow_cursor.set_checked(follow_cursor_active);
        self.follow_cursor.set_text(if !follow_cursor_available {
            follow_cursor_unavailable_reason
        } else if follow_cursor_active {
            "return to fixed mode"
        } else {
            "follow cursor"
        });
        self.follow_cursor.set_enabled(follow_cursor_available);
        self.launch_at_login.set_checked(settings.launch_at_login);
        self.size_menu
            .set_text(format!("size ({}px)", settings.size.round() as i32));
        self.size_s
            .set_text(format!("S ({}px)", size_presets[0].round() as i32));
        self.size_m
            .set_text(format!("M ({}px)", size_presets[1].round() as i32));
        self.size_l
            .set_text(format!("L ({}px)", size_presets[2].round() as i32));
        self.size_xl
            .set_text(format!("XL ({}px)", size_presets[3].round() as i32));
        self.size_scroll_hint.set_enabled(false);
        self.breathing_menu.set_text("breathing pattern");
        self.breathing_coherent
            .set_checked(settings.active_breathing_preset_id == BREATHING_PRESET_ID_COHERENT);
        self.breathing_box
            .set_checked(settings.active_breathing_preset_id == "box_breathing");
        self.breathing_479
            .set_checked(settings.active_breathing_preset_id == "4_7_9");
        for (id, item) in &self.breathing_saved {
            item.set_checked(settings.active_breathing_preset_id == *id);
        }
        self.breathing_delete_menu
            .set_enabled(!self.breathing_delete_items.is_empty());
        self.update_primary.set_text(update_label);
        self.update_ignore_current
            .set_enabled(update_ignore_enabled);
        self.update_ignore_current
            .set_checked(update_ignore_checked);
    }

    pub(crate) fn clone_for_tray(&self) -> Box<dyn muda::ContextMenu> {
        #[cfg(target_os = "windows")]
        {
            let tray_menu = Menu::new();
            for item in self.root.items() {
                let item: &dyn IsMenuItem = match &item {
                    muda::MenuItemKind::MenuItem(item) => item,
                    muda::MenuItemKind::Submenu(item) => item,
                    muda::MenuItemKind::Predefined(item) => item,
                    muda::MenuItemKind::Check(item) => item,
                    muda::MenuItemKind::Icon(item) => item,
                };
                tray_menu
                    .append(item)
                    .expect("native context menu items should be valid for the tray menu");
            }
            Box::new(tray_menu)
        }

        #[cfg(not(target_os = "windows"))]
        Box::new(self.root.clone())
    }

    pub(crate) fn sync_consent(&self, usage_enabled: bool, crash_enabled: bool) {
        self.usage_on.set_checked(usage_enabled);
        self.usage_off.set_checked(!usage_enabled);
        self.crash_on.set_checked(crash_enabled);
        self.crash_off.set_checked(!crash_enabled);
    }

    #[cfg(debug_assertions)]
    pub(crate) fn sync_developer_controls(&self, update_check: &UpdateCheckService) {
        self.simulate_pending_update
            .set_checked(update_check.simulate_pending_update());
    }

    pub(crate) fn show(&self, window: &HostWindow, x: i32, y: i32) {
        #[cfg(not(target_os = "linux"))]
        let position: muda::dpi::Position = MenuPhysicalPosition::new(x as f64, y as f64).into();

        #[cfg(target_os = "linux")]
        let position: muda::dpi::Position = MenuPhysicalPosition::new(
            f64::from(x) * window.scale_factor(),
            f64::from(y) * window.scale_factor(),
        )
        .into();

        #[cfg(target_os = "macos")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

            let view = match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
                    _ => return,
                },
                Err(error) => {
                    log_stderr!("warning: failed to access window handle for native menu: {error}");
                    return;
                }
            };
            unsafe {
                let _ = self
                    .root
                    .show_context_menu_for_nsview(view.cast_const(), Some(position));
            }
        }

        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

            let hwnd = match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::Win32(handle) => handle.hwnd.get(),
                    _ => return,
                },
                Err(error) => {
                    log_stderr!("warning: failed to access window handle for native menu: {error}");
                    return;
                }
            };
            unsafe {
                let _ = self.root.show_context_menu_for_hwnd(hwnd, Some(position));
            }
        }

        #[cfg(target_os = "linux")]
        {
            let _ = self
                .root
                .show_context_menu_for_gtk_window(window.gtk_window(), Some(position));
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
#[derive(Clone)]
pub(crate) struct NativeContextMenu;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
impl NativeContextMenu {
    pub(crate) fn new(_settings: &Settings) -> Option<Self> {
        None
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync_from_settings(
        &self,
        _settings: &Settings,
        _size_presets: [f64; 4],
        _update_label: &str,
        _update_ignore_enabled: bool,
        _update_ignore_checked: bool,
        _follow_cursor_active: bool,
        _follow_cursor_available: bool,
        _follow_cursor_unavailable_reason: &str,
    ) {
    }

    pub(crate) fn sync_consent(&self, _usage_enabled: bool, _crash_enabled: bool) {}

    pub(crate) fn sync_developer_controls(&self, _update_check: &UpdateCheckService) {}

    pub(crate) fn show(&self, _window: &HostWindow, _x: i32, _y: i32) {}
}

pub(crate) fn breathing_delete_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_DELETE_PREFIX}{id}")
}

pub(crate) fn deleted_breathing_preset_id_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MENU_ID_BREATHING_DELETE_PREFIX)
}

pub(crate) fn breathing_saved_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_SAVED_PREFIX}{id}")
}

pub(crate) fn saved_breathing_preset_id_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MENU_ID_BREATHING_SAVED_PREFIX)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) fn install_event_handler(proxy: EventLoopProxy<AppEvent>) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(AppEvent::MenuActivated(event.id().as_ref().to_string()));
    }));
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub(crate) fn install_event_handler(_proxy: EventLoopProxy<AppEvent>) {}

pub(crate) fn snooze_minutes_for_menu_id(id: &str) -> Option<u64> {
    match id {
        MENU_ID_SNOOZE_5 => Some(SNOOZE_PRESET_MINUTES[0]),
        MENU_ID_SNOOZE_10 => Some(SNOOZE_PRESET_MINUTES[1]),
        MENU_ID_SNOOZE_15 => Some(SNOOZE_PRESET_MINUTES[2]),
        MENU_ID_SNOOZE_30 => Some(SNOOZE_PRESET_MINUTES[3]),
        MENU_ID_SNOOZE_60 => Some(SNOOZE_PRESET_MINUTES[4]),
        _ => None,
    }
}

pub(crate) fn size_slot_for_menu_id(id: &str) -> Option<usize> {
    match id {
        MENU_ID_SIZE_S => Some(0),
        MENU_ID_SIZE_M => Some(1),
        MENU_ID_SIZE_L => Some(2),
        MENU_ID_SIZE_XL => Some(3),
        _ => None,
    }
}
