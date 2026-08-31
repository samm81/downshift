#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
#![cfg_attr(target_os = "linux", allow(dead_code))]

mod app_core;
mod cursor;
mod host;
mod ui_assets;
mod window_policy;

use app_core::*;
use cursor::{CoordinateSpace, CursorError, CursorPosition, CursorProvider, CursorSource};
use downshift::telemetry::{
    telemetry_state, ActivityState, ActivityTrigger, EventName, MenuAction, RuntimeTelemetryClient,
    SessionEndReason,
};
use downshift::{
    apply_resize_step, built_in_breathing_preset, built_in_breathing_presets, clamp_size,
    diagnostics, load_settings_result, BreathingPattern, IpcCommand, SavedBreathingPreset,
    Settings, BREATHING_PRESET_ID_COHERENT, BREATHING_PRESET_ID_CUSTOM,
};
#[cfg(all(test, unix))]
use host::instance::instance_socket_path_for_executable;
#[cfg(all(test, target_os = "windows"))]
use host::instance::windows_instance_pipe_name;
#[cfg(all(test, target_os = "macos"))]
use host::launch_at_login::{remove_launch_agent, write_launch_agent};
use host::menu::*;
use host::InstanceStart;
use host::NativeContextMenu;
use host::{
    build_main_webview, clear_child_window, configure_event_loop_builder, copy_text_to_clipboard,
    create_fixed_child_window, create_main_window, create_tray_icon, current_os_version,
    enforce_fixed_size, focus_existing_child_window, install_menu_event_handler,
    install_tray_event_handler, logical_outer_position, native_menu_available, open_external_url,
    persisted_monitor, snapshot_monitor, sync_child_webview_bounds, sync_main_webview_bounds,
    update_tray_menu, TrayIconHandle,
};
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};
use ui_assets::*;
use window_policy::*;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId};
use wry::WebView;

mod update_check;
use update_check::{UpdateCheckResult, UpdateCheckService, UpdateCheckSource};

const ANIMATION_BOUNDS_PADDING_PX: f64 = 2.0;
const ANIMATION_VIEWBOX_SIZE: f64 = 100.0;
const FOLLOW_CURSOR_POLL_INTERVAL: Duration = Duration::from_millis(33);

macro_rules! log_stderr {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        diagnostics::log_line("ERROR", &message);
    }};
}

fn export_diagnostics_to_temp_file(text: &str) -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let path = std::env::temp_dir().join(format!("downshift-diagnostics-{timestamp}.txt"));
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(path)
}

fn spawn_update_check(
    proxy: EventLoopProxy<AppEvent>,
    update_check: UpdateCheckService,
    source: UpdateCheckSource,
) {
    std::thread::spawn(move || {
        let result = update_check.check();
        let _ = proxy.send_event(AppEvent::UpdateCheckFinished(result, source));
    });
}

struct App {
    window: Option<Window>,
    window_id: Option<WindowId>,
    webview: Option<WebView>,
    custom_snooze_window: Option<Window>,
    custom_snooze_window_id: Option<WindowId>,
    custom_snooze_webview: Option<WebView>,
    breathing_pattern_window: Option<Window>,
    breathing_pattern_window_id: Option<WindowId>,
    breathing_pattern_webview: Option<WebView>,
    telemetry_info_window: Option<Window>,
    telemetry_info_window_id: Option<WindowId>,
    telemetry_info_webview: Option<WebView>,
    update_dialog_window: Option<Window>,
    update_dialog_window_id: Option<WindowId>,
    update_dialog_webview: Option<WebView>,
    native_context_menu: Option<NativeContextMenu>,
    tray_icon: Option<TrayIconHandle>,
    startup_error: Option<String>,
    settings: Settings,
    config_path: Option<std::path::PathBuf>,
    event_loop_proxy: Option<EventLoopProxy<AppEvent>>,
    drag_anchor_window_pos: Option<LogicalPosition<f64>>,
    drag_anchor_pointer_pos: Option<LogicalPosition<f64>>,
    activity_mode: ActivityMode,
    snooze_deadline: Option<SystemTime>,
    snooze_generation: u64,
    telemetry: RuntimeTelemetryClient,
    telemetry_install_first_run: bool,
    session_ended: bool,
    settings_load_error: Option<String>,
    settings_backup_pending: bool,
    startup_provenance: String,
    updates: UpdateUiState,
    update_check: UpdateCheckService,
    manual_update_check_in_flight: bool,
    animation_bounds: AnimationBounds,
    follow_cursor_active: bool,
    follow_cursor_supported: bool,
    follow_cursor_unavailable_reason: &'static str,
    cursor_source: Option<Box<dyn CursorSource>>,
    follow_cursor_previous_position: Option<PhysicalPosition<i32>>,
    follow_cursor_placement: Option<FollowPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AnimationBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    badge_visible: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FollowMonitor {
    work_area: ScreenRect,
    scale_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FollowPlacement {
    cursor: PhysicalPosition<i32>,
    monitor: FollowMonitor,
}

impl AnimationBounds {
    const fn full() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: ANIMATION_VIEWBOX_SIZE,
            height: ANIMATION_VIEWBOX_SIZE,
            badge_visible: false,
        }
    }

    fn is_valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= 0.0
            && self.height >= 0.0
    }
}

impl Default for App {
    fn default() -> Self {
        let telemetry_state = telemetry_state();
        let telemetry_install_first_run = telemetry_state.install_first_run;
        let telemetry = RuntimeTelemetryClient::from_state(telemetry_state);
        Self {
            window: None,
            window_id: None,
            webview: None,
            custom_snooze_window: None,
            custom_snooze_window_id: None,
            custom_snooze_webview: None,
            breathing_pattern_window: None,
            breathing_pattern_window_id: None,
            breathing_pattern_webview: None,
            telemetry_info_window: None,
            telemetry_info_window_id: None,
            telemetry_info_webview: None,
            update_dialog_window: None,
            update_dialog_window_id: None,
            update_dialog_webview: None,
            native_context_menu: None,
            tray_icon: None,
            startup_error: None,
            settings: Settings::default(),
            config_path: None,
            event_loop_proxy: None,
            drag_anchor_window_pos: None,
            drag_anchor_pointer_pos: None,
            activity_mode: ActivityMode::Active,
            snooze_deadline: None,
            snooze_generation: 0,
            telemetry,
            telemetry_install_first_run,
            session_ended: false,
            settings_load_error: None,
            settings_backup_pending: false,
            startup_provenance: "unknown".to_string(),
            updates: UpdateUiState::default(),
            update_check: UpdateCheckService::new(download_release_url()),
            manual_update_check_in_flight: false,
            animation_bounds: AnimationBounds::full(),
            follow_cursor_active: false,
            follow_cursor_supported: false,
            follow_cursor_unavailable_reason:
                "cursor following is unavailable until the app window is ready",
            cursor_source: None,
            follow_cursor_previous_position: None,
            follow_cursor_placement: None,
        }
    }
}

impl App {
    fn handle_app_suspend(&self) {
        self.telemetry.note_suspend();
    }

    fn handle_app_resume(&mut self) {
        self.telemetry.note_resume();
        self.reconcile_snooze_after_resume();
    }

    fn current_activity_state(&self) -> ActivityState {
        match self.activity_mode {
            ActivityMode::Active => ActivityState::Active,
            ActivityMode::Paused => ActivityState::Paused,
            ActivityMode::Snoozed => ActivityState::Snoozed,
        }
    }

    fn current_activity_label(&self) -> &'static str {
        match self.activity_mode {
            ActivityMode::Active => "active",
            ActivityMode::Paused => "paused",
            ActivityMode::Snoozed => "snoozed",
        }
    }

    fn diagnostics_snapshot(&self) -> diagnostics::DiagnosticsSnapshot {
        let executable_path = std::env::current_exe()
            .ok()
            .map(|path| path.display().to_string());
        let window_position = logical_outer_position(self.window.as_ref()).map(|position| {
            format!(
                "x={}, y={}",
                position.x.round() as i32,
                position.y.round() as i32
            )
        });
        let window_scale_factor = self
            .window
            .as_ref()
            .map(|window| format!("{:.2}", window.scale_factor()));
        let monitor = self
            .window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .map(|monitor| persisted_monitor(&monitor))
            .or(self.settings.monitor);
        let monitor = monitor.map(|monitor| {
            format!(
                "{}x{} @ {:.2}x",
                monitor.width, monitor.height, monitor.scale_factor
            )
        });
        let (width_px, height_px) = self.widget_dimensions_px();
        diagnostics::DiagnosticsSnapshot {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            build_channel: build_channel_label(),
            env: runtime_env_label(),
            os_version: current_os_version(),
            arch: std::env::consts::ARCH.to_string(),
            runtime_state: self.current_activity_label().to_string(),
            startup_provenance: self.startup_provenance.clone(),
            settings_load_status: self
                .settings_load_error
                .clone()
                .unwrap_or_else(|| "ok".to_string()),
            telemetry_global_enabled: telemetry_globally_enabled(),
            usage_sharing_enabled: self.settings.usage_data_sharing,
            crash_reports_enabled: self.settings.crash_reports_sharing,
            telemetry_install_first_run: self.telemetry_install_first_run,
            executable_path,
            settings_path: self
                .config_path
                .as_ref()
                .map(|path| path.display().to_string()),
            log_path: diagnostics::log_path().map(|path| path.display().to_string()),
            window_position,
            window_size_px: format!("{width_px}x{height_px}"),
            window_scale_factor,
            monitor,
            settings_toml: toml::to_string_pretty(&self.settings)
                .unwrap_or_else(|error| format!("serialization_error = {:?}", error.to_string())),
        }
    }

    fn diagnostics_summary(&self) -> String {
        diagnostics::build_summary(&self.diagnostics_snapshot())
    }

    fn copy_diagnostics_summary(&self) {
        let summary = self.diagnostics_summary();
        if let Err(error) = copy_text_to_clipboard(&summary) {
            log_stderr!("warning: failed to copy diagnostics to clipboard: {error}");
            match export_diagnostics_to_temp_file(&summary) {
                Ok(path) => log_stderr!(
                    "warning: exported diagnostics summary instead: {}",
                    path.display()
                ),
                Err(export_error) => {
                    log_stderr!("error: failed to export diagnostics summary: {export_error}")
                }
            }
        }
    }

    fn telemetry_activity_state(
        &self,
        state: ActivityState,
        trigger: ActivityTrigger,
        requested_duration_sec: Option<u64>,
    ) {
        self.telemetry
            .track_activity_state(state, trigger, requested_duration_sec);
    }

    fn telemetry_menu_action(&self, action: MenuAction, size_target: Option<&str>) {
        let mut payload = serde_json::json!({
            "action": serde_json::to_value(action).unwrap_or_else(|_| serde_json::json!("unknown")),
        });
        if let Some(target) = size_target {
            payload["size_target"] = serde_json::json!(target);
        }
        self.telemetry.track(EventName::MenuAction, payload);
    }

    fn telemetry_follow_cursor_change(&self, enabled: bool) {
        self.telemetry.track(
            EventName::MenuAction,
            serde_json::json!({
                "action": serde_json::to_value(MenuAction::FollowCursor)
                    .unwrap_or_else(|_| serde_json::json!("follow_cursor")),
                "enabled": enabled,
            }),
        );
    }

    fn telemetry_privacy_change(&self, setting: &str, enabled: bool) {
        self.telemetry.track(
            EventName::PrivacyPreferenceChanged,
            serde_json::json!({
                "setting": setting,
                "new_value": if enabled { "enabled" } else { "disabled" },
            }),
        );
    }

    fn telemetry_launch_at_login_change(&self, enabled: bool) {
        self.telemetry.track(
            EventName::MenuAction,
            serde_json::json!({
                "action": "launch_at_login",
                "enabled": enabled,
            }),
        );
    }

    fn telemetry_update_flow(&self, action: &str, mut properties: serde_json::Value) {
        properties["action"] = serde_json::json!(action);
        self.telemetry.track(EventName::UpdateFlow, properties);
    }

    fn telemetry_breathing_pattern_change(
        &self,
        action: &str,
        preset_id: &str,
        preset_name: Option<&str>,
        pattern: &BreathingPattern,
    ) {
        self.telemetry.track(
            EventName::BreathingPatternChanged,
            serde_json::json!({
                "action": action,
                "preset_id": preset_id,
                "preset_name": preset_name,
                "is_custom": preset_id == BREATHING_PRESET_ID_CUSTOM,
                "is_saved_preset": preset_name.is_some() && preset_id != BREATHING_PRESET_ID_CUSTOM
                    && built_in_breathing_preset(preset_id).is_none(),
                "pattern": breathing_pattern_payload(pattern),
            }),
        );
    }

    fn telemetry_breathing_pattern_window(&self, action: &str) {
        self.telemetry.track(
            EventName::BreathingPatternChanged,
            serde_json::json!({
                "action": action,
                "pattern": breathing_pattern_payload(&self.settings.breathing_pattern),
            }),
        );
    }

    fn apply_usage_data_sharing(&mut self, enabled: bool) {
        self.settings.usage_data_sharing = enabled;
        if enabled {
            self.telemetry.set_usage_enabled(true);
            self.telemetry_privacy_change("usage_data", true);
        } else {
            // Emit the opt-out event while usage telemetry is still enabled.
            self.telemetry_privacy_change("usage_data", false);
            self.telemetry.set_usage_enabled(false);
        }
        self.sync_privacy_state_to_webview();
        self.sync_analytics_menu_state();
        self.save_settings();
    }

    fn apply_crash_reports_sharing(&mut self, enabled: bool) {
        self.settings.crash_reports_sharing = enabled;
        if enabled {
            self.telemetry.set_crash_enabled(true);
            self.telemetry_privacy_change("crash_reports", true);
        } else {
            // Emit the opt-out event before disabling crash telemetry.
            self.telemetry_privacy_change("crash_reports", false);
            self.telemetry.set_crash_enabled(false);
        }
        self.sync_privacy_state_to_webview();
        self.sync_analytics_menu_state();
        self.save_settings();
    }

    fn sync_launch_at_login_setting(&mut self, enabled: bool) {
        let result = host::set_launch_at_login(enabled);
        match result {
            Ok(()) => {
                self.settings.launch_at_login = enabled;
            }
            Err(error) => {
                log_stderr!("warning: failed to update launch-at-login setting: {error}");
            }
        }
    }

    fn apply_launch_at_login(&mut self, enabled: bool) {
        self.sync_launch_at_login_setting(enabled);
        self.sync_update_menu_state();
        self.save_settings();
    }

    fn reconcile_launch_at_login(&mut self) {
        if let Err(error) = host::set_launch_at_login(self.settings.launch_at_login) {
            log_stderr!("warning: failed to reconcile launch-at-login setting: {error}");
        }
    }

    fn widget_dimensions_px(&self) -> (u32, u32) {
        let size = self.settings.size.round().max(0.0) as u32;
        (size, size)
    }

    fn widget_window_dimensions(&self, size: f64) -> LogicalSize<f64> {
        let (width, height) = app_core::widget_window_dimensions(size);
        LogicalSize::new(width, height)
    }

    fn follow_cursor_artwork_size(&self) -> f64 {
        FOLLOW_CURSOR_ARTWORK_SIZE_LOGICAL
    }

    fn artwork_size_for_window(&self) -> f64 {
        if self.follow_cursor_active {
            self.follow_cursor_artwork_size()
        } else {
            self.settings.size
        }
    }

    fn animation_window_dimensions_for(
        &self,
        artwork_size: f64,
        bounds: AnimationBounds,
    ) -> LogicalSize<f64> {
        if self.follow_cursor_active {
            return LogicalSize::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
            );
        }
        let view_box_scale = artwork_size / ANIMATION_VIEWBOX_SIZE;
        let badge_reserve = if bounds.badge_visible {
            UPDATE_BADGE_WINDOW_RESERVE_PX
        } else {
            0.0
        };
        LogicalSize::new(
            (bounds.width * view_box_scale + ANIMATION_BOUNDS_PADDING_PX * 2.0)
                .ceil()
                .max(1.0),
            (bounds.height * view_box_scale + ANIMATION_BOUNDS_PADDING_PX * 2.0 + badge_reserve)
                .ceil()
                .max(1.0),
        )
    }

    fn animation_shape_bottom_center(
        &self,
        artwork_size: f64,
        bounds: AnimationBounds,
    ) -> LogicalPosition<f64> {
        let view_box_scale = artwork_size / ANIMATION_VIEWBOX_SIZE;
        LogicalPosition::new(
            ANIMATION_BOUNDS_PADDING_PX + bounds.width / 2.0 * view_box_scale,
            ANIMATION_BOUNDS_PADDING_PX + bounds.height * view_box_scale,
        )
    }

    fn set_animation_bounds(&mut self, next: AnimationBounds) {
        if !next.is_valid() {
            return;
        }
        let previous = self.animation_bounds;
        if previous == next {
            return;
        }

        let position = if self.follow_cursor_active {
            None
        } else {
            logical_outer_position(self.window.as_ref())
        };
        let previous_artwork_size = self.artwork_size_for_window();
        let previous_anchor = self.animation_shape_bottom_center(previous_artwork_size, previous);
        self.animation_bounds = next;

        let artwork_size = self.artwork_size_for_window();
        let next_anchor = self.animation_shape_bottom_center(artwork_size, next);
        let position_delta = LogicalPosition::new(
            previous_anchor.x - next_anchor.x,
            previous_anchor.y - next_anchor.y,
        );

        if self.follow_cursor_active {
            self.apply_main_webview_state(serde_json::json!({
                "artwork_size": artwork_size,
            }));
        }

        let target_dimensions =
            self.animation_window_dimensions_for(artwork_size, self.animation_bounds);
        {
            let Some(window) = self.window.as_ref() else {
                return;
            };
            let current = window.inner_size().to_logical::<f64>(window.scale_factor());
            let width_mismatch = (current.width - target_dimensions.width).abs() > 0.5;
            let height_mismatch = (current.height - target_dimensions.height).abs() > 0.5;
            window.set_resizable(false);
            window.set_min_inner_size(Some(target_dimensions));
            window.set_max_inner_size(Some(target_dimensions));
            if width_mismatch || height_mismatch {
                let _ = window.request_inner_size(target_dimensions);
            }
        }

        if self.follow_cursor_active {
            if let Some(placement) = self.follow_cursor_placement {
                self.position_follow_cursor(placement);
            }
        } else if let Some(position) = position {
            let Some(window) = self.window.as_ref() else {
                return;
            };
            let next_position =
                LogicalPosition::new(position.x + position_delta.x, position.y + position_delta.y);
            window.set_outer_position(next_position);
            if let Some(anchor) = self.drag_anchor_window_pos.as_mut() {
                anchor.x += position_delta.x;
                anchor.y += position_delta.y;
            }
        }
    }

    fn telemetry_heartbeat(&self) {
        if self.session_ended {
            return;
        }
        self.telemetry.track(
            EventName::SessionHeartbeat,
            self.heartbeat_snapshot().into_properties(),
        );
    }

    fn finish_session(&mut self, reason: SessionEndReason) {
        if self.session_ended {
            return;
        }
        self.session_ended = true;
        self.telemetry.end_session(reason);
        self.telemetry.flush(std::time::Duration::from_millis(1500));
        self.telemetry
            .shutdown(std::time::Duration::from_millis(1500));
    }

    fn quit_app(&mut self, event_loop: &ActiveEventLoop) {
        self.telemetry_menu_action(MenuAction::Quit, None);
        self.save_settings();
        self.finish_session(SessionEndReason::QuitMenu);
        event_loop.exit();
    }

    fn start_manual_drag(&mut self, screen_x: i32, screen_y: i32) {
        if self.follow_cursor_active {
            return;
        }
        self.drag_anchor_window_pos = logical_outer_position(self.window.as_ref());
        self.drag_anchor_pointer_pos = Some(LogicalPosition::new(screen_x as f64, screen_y as f64));
    }

    fn drag_to(&mut self, screen_x: i32, screen_y: i32) {
        if self.follow_cursor_active {
            return;
        }
        let (Some(anchor_window), Some(anchor_pointer), Some(window)) = (
            self.drag_anchor_window_pos,
            self.drag_anchor_pointer_pos,
            self.window.as_ref(),
        ) else {
            return;
        };

        let (next_x, next_y) = app_core::drag_position(
            (anchor_window.x, anchor_window.y),
            (anchor_pointer.x, anchor_pointer.y),
            (screen_x as f64, screen_y as f64),
        );
        window.set_outer_position(LogicalPosition::new(next_x, next_y));
    }

    fn stop_manual_drag(&mut self) {
        self.drag_anchor_window_pos = None;
        self.drag_anchor_pointer_pos = None;
    }

    fn enforce_fixed_widget_size(&self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let target_dimensions = self
            .animation_window_dimensions_for(self.artwork_size_for_window(), self.animation_bounds);
        enforce_fixed_size(window, target_dimensions);
    }

    fn sync_webview_bounds(&self) {
        sync_main_webview_bounds(self.window.as_ref(), self.webview.as_ref());
    }

    fn sync_telemetry_info_webview_bounds(&self) {
        sync_child_webview_bounds(
            self.telemetry_info_window.as_ref(),
            self.telemetry_info_webview.as_ref(),
            "telemetry info webview",
        );
    }

    fn config_path() -> Option<std::path::PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("downshift");
        path.push("settings.toml");
        Some(path)
    }

    fn settings_backup_path(path: &std::path::Path) -> PathBuf {
        app_core::settings_backup_path(path)
    }

    fn backup_corrupt_settings_if_needed(&mut self, path: &std::path::Path) -> Result<(), String> {
        if !self.settings_backup_pending {
            return Ok(());
        }
        if !path.exists() {
            self.settings_backup_pending = false;
            self.settings_load_error = None;
            return Ok(());
        }
        let backup_path = Self::settings_backup_path(path);
        std::fs::copy(path, &backup_path).map_err(|error| {
            format!(
                "failed to back up unreadable settings to {}: {error}",
                backup_path.display()
            )
        })?;
        self.settings_backup_pending = false;
        self.settings_load_error = None;
        Ok(())
    }

    fn save_settings(&mut self) {
        let Some(path) = &self.config_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                log_stderr!("warning: failed to create config directory: {error}");
                return;
            }
        }
        let path = path.clone();
        if let Err(error) = self.backup_corrupt_settings_if_needed(&path) {
            log_stderr!("warning: {error}");
            return;
        }
        let content = match toml::to_string_pretty(&self.settings) {
            Ok(content) => content,
            Err(error) => {
                log_stderr!("warning: failed to serialize settings: {error}");
                return;
            }
        };
        if let Err(error) = std::fs::write(path, content) {
            log_stderr!("warning: failed to write settings: {error}");
        }
    }

    fn sync_analytics_menu_state(&self) {
        if let Some(menu) = self.native_context_menu.as_ref() {
            menu.sync_consent(
                self.settings.usage_data_sharing,
                self.settings.crash_reports_sharing,
            );
        }
    }

    fn heartbeat_snapshot(&self) -> HeartbeatSnapshot {
        let (width_px, height_px) = self.widget_dimensions_px();
        HeartbeatSnapshot {
            state: self.current_activity_label().to_string(),
            paused: self.activity_mode == ActivityMode::Paused,
            snoozed: self.activity_mode == ActivityMode::Snoozed,
            active_breathing_preset_id: self.settings.active_breathing_preset_id.clone(),
            breathing_pattern: self.settings.breathing_pattern.clone(),
            width_px,
            height_px,
            usage_enabled: self.settings.usage_data_sharing,
            crash_enabled: self.settings.crash_reports_sharing,
        }
    }

    fn sync_update_menu_state(&self) {
        if let Some(menu) = self.native_context_menu.as_ref() {
            menu.sync_from_settings(
                &self.settings,
                self.current_size_presets(),
                &self.updates.menu_label(),
                self.updates.has_update_available(),
                self.updates.is_ignoring_current_update(),
                self.follow_cursor_active,
                self.follow_cursor_supported,
                self.follow_cursor_unavailable_reason,
            );
        }
    }

    fn sync_tray_menu(&self) {
        update_tray_menu(self.tray_icon.as_ref(), self.native_context_menu.as_ref());
    }

    fn rebuild_native_context_menu(&mut self) {
        self.native_context_menu = NativeContextMenu::new(&self.settings);
        #[cfg(debug_assertions)]
        if let Some(menu) = self.native_context_menu.as_ref() {
            menu.sync_developer_controls(&self.update_check);
        }
        self.sync_update_menu_state();
        self.sync_analytics_menu_state();
        self.sync_tray_menu();
    }

    fn sync_update_state_to_webview(&self) {
        self.apply_main_webview_state(serde_json::json!({
            "update_menu_label": self.updates.menu_label(),
            "update_has_new_version": self.updates.has_update_available(),
            "update_show_badge": self.updates.should_show_badge(),
            "update_ignore_current_enabled": self.updates.has_update_available(),
            "update_ignore_current_checked": self.updates.is_ignoring_current_update(),
            "follow_cursor_active": self.follow_cursor_active,
            "follow_cursor_available": self.follow_cursor_supported,
            "follow_cursor_unavailable_reason": self.follow_cursor_unavailable_reason,
            "artwork_size": self.artwork_size_for_window(),
            "follow_cursor_halo_size": FOLLOW_CURSOR_HALO_SIZE_LOGICAL,
            "follow_cursor_window_size": FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
        }));
    }

    fn sync_update_surfaces(&self) {
        self.enforce_fixed_widget_size();
        self.sync_update_menu_state();
        self.sync_update_state_to_webview();
    }

    fn dismiss_current_update_badge(&mut self) {
        let Some(latest) = self.updates.latest_version.as_ref() else {
            return;
        };
        self.telemetry_update_flow(
            "badge_dismissed",
            serde_json::json!({
                "latest_version": latest,
            }),
        );
        self.settings.update_badge_snoozed_version = Some(latest.clone());
        self.settings.update_badge_snoozed_at_epoch_seconds = Some(now_epoch_seconds());
        self.updates.badge_snoozed_version = Some(latest.clone());
        self.updates.badge_snoozed_at_epoch_seconds =
            self.settings.update_badge_snoozed_at_epoch_seconds;
        self.save_settings();
        self.sync_update_surfaces();
    }

    fn apply_ignore_current_update(&mut self, ignored: bool) {
        let latest = self
            .updates
            .latest_version
            .as_ref()
            .filter(|latest| is_newer_version(latest, env!("CARGO_PKG_VERSION")))
            .cloned();
        if ignored {
            let Some(latest) = latest else {
                return;
            };
            self.settings.ignored_update_version = Some(latest.clone());
            self.updates.ignored_version = Some(latest.clone());
            self.telemetry_update_flow(
                "ignore_current_update_changed",
                serde_json::json!({
                    "latest_version": latest,
                    "ignored": true,
                }),
            );
        } else {
            let was_ignored = self.updates.is_ignoring_current_update()
                || self.settings.ignored_update_version.is_some();
            self.settings.ignored_update_version = match (
                latest.as_deref(),
                self.settings.ignored_update_version.as_deref(),
            ) {
                (Some(latest), Some(ignored_version)) if ignored_version != latest => {
                    self.settings.ignored_update_version.clone()
                }
                _ => None,
            };
            self.updates.ignored_version = self.settings.ignored_update_version.clone();
            if was_ignored {
                self.telemetry_update_flow(
                    "ignore_current_update_changed",
                    serde_json::json!({
                        "latest_version": latest,
                        "ignored": false,
                    }),
                );
            }
        }
        self.save_settings();
        self.sync_update_surfaces();
    }

    fn apply_update_check_result(&mut self, result: UpdateCheckResult) {
        let should_persist_latest_version = result.should_persist_latest_version();
        if let Some(latest) = result.latest_version {
            self.updates.latest_version = Some(latest.clone());
            if should_persist_latest_version {
                self.settings.cached_latest_update_version = Some(latest);
                self.save_settings();
            }
        }
        self.updates.download_url = result.download_url;
        self.updates.checked_once = true;
        self.updates.checking = false;
        self.updates.badge_snoozed_version = self.settings.update_badge_snoozed_version.clone();
        self.updates.badge_snoozed_at_epoch_seconds =
            self.settings.update_badge_snoozed_at_epoch_seconds;
        self.updates.ignored_version = self.settings.ignored_update_version.clone();

        if let Some(latest) = self.updates.latest_version.as_ref() {
            let mut settings_changed = false;
            if self.settings.ignored_update_version.as_deref() == Some(latest.as_str())
                && !self.updates.has_update_available()
            {
                self.settings.ignored_update_version = None;
                settings_changed = true;
            }
            if self.settings.update_badge_snoozed_version.as_deref() == Some(latest.as_str())
                && !self.updates.has_update_available()
            {
                self.settings.update_badge_snoozed_version = None;
                self.settings.update_badge_snoozed_at_epoch_seconds = None;
                settings_changed = true;
            }
            if settings_changed {
                self.updates.badge_snoozed_version =
                    self.settings.update_badge_snoozed_version.clone();
                self.updates.badge_snoozed_at_epoch_seconds =
                    self.settings.update_badge_snoozed_at_epoch_seconds;
                self.updates.ignored_version = self.settings.ignored_update_version.clone();
                self.save_settings();
            }
        }
        self.sync_update_surfaces();
    }

    fn launch_update_download(&self, source: &str) {
        let latest_version = self.updates.latest_version.clone();
        self.telemetry_update_flow(
            "download_opened",
            serde_json::json!({
                "source": source,
                "has_update_available": self.updates.has_update_available(),
                "latest_version": latest_version,
            }),
        );
        let url = if self.updates.has_update_available() {
            self.updates.download_url.as_str()
        } else {
            &self.updates.download_url
        };
        open_external_url(url);
    }

    fn open_update_dialog_window(&mut self, event_loop: &ActiveEventLoop) {
        if focus_existing_child_window(self.update_dialog_window.as_ref()) {
            return;
        }
        let (window, window_id, webview) = match create_fixed_child_window(
            event_loop,
            self.event_loop_proxy.as_ref(),
            "updates",
            360.0,
            168.0,
            update_dialog_html(),
            "update dialog",
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                log_stderr!("warning: {error}");
                return;
            }
        };

        self.update_dialog_window = Some(window);
        self.update_dialog_window_id = Some(window_id);
        self.update_dialog_webview = Some(webview);
        self.sync_update_dialog_webview_bounds();
    }

    fn sync_update_dialog_webview_bounds(&self) {
        sync_child_webview_bounds(
            self.update_dialog_window.as_ref(),
            self.update_dialog_webview.as_ref(),
            "update dialog webview",
        );
    }

    fn close_update_dialog_window(&mut self) {
        clear_child_window(
            &mut self.update_dialog_window,
            &mut self.update_dialog_window_id,
            &mut self.update_dialog_webview,
        );
    }

    fn set_update_dialog_mode_checking(&self) {
        if let Some(webview) = self.update_dialog_webview.as_ref() {
            let _ = webview.evaluate_script("window.updateDialogApplyState({ mode: 'checking' });");
        }
    }

    fn set_update_dialog_mode_result(&self) {
        let Some(webview) = self.update_dialog_webview.as_ref() else {
            return;
        };
        let js = if self.updates.has_update_available() {
            let latest_version = self.updates.latest_version.as_deref().unwrap_or("latest");
            format!(
                "window.updateDialogApplyState({{ mode: 'available', latest_version: {} }});",
                serde_json::json!(latest_version)
            )
        } else {
            "window.updateDialogApplyState({ mode: 'latest' });".to_string()
        };
        let _ = webview.evaluate_script(&js);
    }

    fn run_manual_update_check_with_dialog(&mut self, event_loop: &ActiveEventLoop) {
        self.open_update_dialog_window(event_loop);
        self.set_update_dialog_mode_checking();
        if self.manual_update_check_in_flight {
            return;
        }
        self.telemetry_update_flow("manual_check_started", serde_json::json!({}));
        self.manual_update_check_in_flight = true;
        self.updates.checking = true;
        let Some(proxy) = self.event_loop_proxy.as_ref().cloned() else {
            self.manual_update_check_in_flight = false;
            self.updates.checking = false;
            return;
        };
        spawn_update_check(proxy, self.update_check.clone(), UpdateCheckSource::Manual);
    }

    #[cfg(debug_assertions)]
    fn run_forced_background_update_check(&mut self) {
        let Some(proxy) = self.event_loop_proxy.as_ref().cloned() else {
            return;
        };
        spawn_update_check(
            proxy,
            self.update_check.clone(),
            UpdateCheckSource::Background,
        );
    }

    #[cfg(debug_assertions)]
    fn clear_update_notification_dismissed(&mut self) {
        self.settings.update_badge_snoozed_version = None;
        self.settings.update_badge_snoozed_at_epoch_seconds = None;
        self.updates.badge_snoozed_version = None;
        self.updates.badge_snoozed_at_epoch_seconds = None;
        self.save_settings();
        self.sync_update_surfaces();
    }

    fn handle_update_primary_action(&mut self, event_loop: &ActiveEventLoop) {
        if self.updates.has_update_available() {
            self.dismiss_current_update_badge();
            self.launch_update_download("menu");
            return;
        }
        self.run_manual_update_check_with_dialog(event_loop);
    }

    fn open_custom_snooze_window(&mut self, event_loop: &ActiveEventLoop) {
        if focus_existing_child_window(self.custom_snooze_window.as_ref()) {
            return;
        }
        let (window, window_id, webview) = match create_fixed_child_window(
            event_loop,
            self.event_loop_proxy.as_ref(),
            "custom snooze",
            320.0,
            144.0,
            custom_snooze_html(),
            "custom snooze",
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                log_stderr!("warning: {error}");
                return;
            }
        };
        self.custom_snooze_window = Some(window);
        self.custom_snooze_window_id = Some(window_id);
        self.custom_snooze_webview = Some(webview);
        self.sync_custom_snooze_webview_bounds();
    }

    fn breathing_pattern_editor_payload(&self) -> serde_json::Value {
        serde_json::json!({
            "pattern": self.settings.breathing_pattern,
        })
    }

    fn breathing_pattern_menu_presets_payload(&self) -> serde_json::Value {
        let mut presets = built_in_breathing_presets()
            .into_iter()
            .filter(|preset| {
                !self
                    .settings
                    .hidden_breathing_preset_ids
                    .iter()
                    .any(|id| id == preset.id)
            })
            .map(|preset| {
                serde_json::json!({
                    "id": preset.id,
                    "name": format!("{} ({})", preset.name, breathing_pattern_summary(&preset.pattern)),
                })
            })
            .collect::<Vec<_>>();
        presets.extend(self.settings.saved_breathing_presets.iter().map(|preset| {
            serde_json::json!({
                "id": preset.id,
                "name": format!("{} ({})", preset.name, breathing_pattern_summary(&preset.pattern)),
            })
        }));
        serde_json::json!(presets)
    }

    fn sync_breathing_pattern_webview_bounds(&self) {
        sync_child_webview_bounds(
            self.breathing_pattern_window.as_ref(),
            self.breathing_pattern_webview.as_ref(),
            "breathing pattern webview",
        );
    }

    fn sync_breathing_pattern_editor_state(&self) {
        let Some(webview) = self.breathing_pattern_webview.as_ref() else {
            return;
        };
        let js = format!(
            "window.breathingPatternApplyState({});",
            self.breathing_pattern_editor_payload()
        );
        let _ = webview.evaluate_script(&js);
    }

    fn open_breathing_pattern_window(&mut self, event_loop: &ActiveEventLoop) {
        if focus_existing_child_window(self.breathing_pattern_window.as_ref()) {
            self.sync_breathing_pattern_editor_state();
            return;
        }
        let (window, window_id, webview) = match create_fixed_child_window(
            event_loop,
            self.event_loop_proxy.as_ref(),
            "add breathing pattern",
            420.0,
            340.0,
            breathing_pattern_html(),
            "breathing pattern",
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                log_stderr!("warning: {error}");
                return;
            }
        };
        self.breathing_pattern_window = Some(window);
        self.breathing_pattern_window_id = Some(window_id);
        self.breathing_pattern_webview = Some(webview);
        self.sync_breathing_pattern_webview_bounds();
        self.sync_breathing_pattern_editor_state();
        self.telemetry_breathing_pattern_window("add_new_opened");
    }

    fn close_breathing_pattern_window(&mut self) {
        clear_child_window(
            &mut self.breathing_pattern_window,
            &mut self.breathing_pattern_window_id,
            &mut self.breathing_pattern_webview,
        );
    }

    fn cancel_breathing_pattern_window(&mut self) {
        if self.breathing_pattern_window.is_none() {
            return;
        }
        self.telemetry_breathing_pattern_window("add_new_canceled");
        self.close_breathing_pattern_window();
    }

    fn sync_custom_snooze_webview_bounds(&self) {
        sync_child_webview_bounds(
            self.custom_snooze_window.as_ref(),
            self.custom_snooze_webview.as_ref(),
            "custom snooze webview",
        );
    }

    fn close_custom_snooze_window(&mut self) {
        clear_child_window(
            &mut self.custom_snooze_window,
            &mut self.custom_snooze_window_id,
            &mut self.custom_snooze_webview,
        );
    }

    fn open_telemetry_info_window(&mut self, event_loop: &ActiveEventLoop) {
        if focus_existing_child_window(self.telemetry_info_window.as_ref()) {
            return;
        }
        let (window, window_id, webview) = match create_fixed_child_window(
            event_loop,
            self.event_loop_proxy.as_ref(),
            "what we collect",
            420.0,
            240.0,
            telemetry_info_html(),
            "telemetry info",
        ) {
            Ok(bundle) => bundle,
            Err(error) => {
                log_stderr!("warning: {error}");
                self.telemetry.track_error(
                    EventName::AppError,
                    serde_json::json!({
                        "category": "telemetry_info_window_create",
                        "severity": "warn",
                        "recoverable": true,
                    }),
                );
                return;
            }
        };
        self.telemetry_info_window = Some(window);
        self.telemetry_info_window_id = Some(window_id);
        self.telemetry_info_webview = Some(webview);
        self.sync_telemetry_info_webview_bounds();
    }

    fn close_telemetry_info_window(&mut self) {
        clear_child_window(
            &mut self.telemetry_info_window,
            &mut self.telemetry_info_window_id,
            &mut self.telemetry_info_webview,
        );
    }

    fn handle_telemetry_info_window_event(&mut self, event: WindowEvent) {
        handle_child_window_event(
            self,
            event,
            Self::close_telemetry_info_window,
            Self::sync_telemetry_info_webview_bounds,
        );
    }

    fn handle_custom_snooze_window_event(&mut self, event: WindowEvent) {
        handle_child_window_event(
            self,
            event,
            Self::close_custom_snooze_window,
            Self::sync_custom_snooze_webview_bounds,
        );
    }

    fn handle_breathing_pattern_window_event(&mut self, event: WindowEvent) {
        handle_child_window_event(
            self,
            event,
            Self::cancel_breathing_pattern_window,
            Self::sync_breathing_pattern_webview_bounds,
        );
    }

    fn handle_update_dialog_window_event(&mut self, event: WindowEvent) {
        handle_child_window_event(
            self,
            event,
            Self::close_update_dialog_window,
            Self::sync_update_dialog_webview_bounds,
        );
    }

    fn sync_privacy_state_to_webview(&self) {
        self.apply_main_webview_state(serde_json::json!({
            "usage_data_sharing": self.settings.usage_data_sharing,
            "crash_reports_sharing": self.settings.crash_reports_sharing,
        }));
    }

    fn choose_initial_position(
        &self,
        event_loop: &ActiveEventLoop,
        size: f64,
    ) -> Option<PhysicalPosition<i32>> {
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        if monitors.is_empty() {
            return None;
        }
        let primary = event_loop
            .primary_monitor()
            .or_else(|| monitors.first().cloned())?;
        let monitor_snapshots = monitors.iter().map(snapshot_monitor).collect::<Vec<_>>();
        let primary_snapshot = snapshot_monitor(&primary);
        let position = window_policy::choose_initial_position(
            &self.settings,
            &monitor_snapshots,
            &primary_snapshot,
            size,
        );
        Some(PhysicalPosition::new(position.x, position.y))
    }

    fn build_init_script(&self, size_presets: [f64; 4]) -> String {
        let payload = serde_json::json!({
          "paused": self.activity_mode == ActivityMode::Paused,
          "breathing_pattern": self.settings.breathing_pattern,
          "active_breathing_preset_id": self.settings.active_breathing_preset_id,
          "breathing_presets": self.breathing_pattern_menu_presets_payload(),
          "usage_data_sharing": self.settings.usage_data_sharing,
          "crash_reports_sharing": self.settings.crash_reports_sharing,
          "update_menu_label": self.updates.menu_label(),
          "update_has_new_version": self.updates.has_update_available(),
          "update_show_badge": self.updates.should_show_badge(),
            "update_ignore_current_enabled": self.updates.has_update_available(),
          "update_ignore_current_checked": self.updates.is_ignoring_current_update(),
          "follow_cursor_active": self.follow_cursor_active,
          "follow_cursor_available": self.follow_cursor_supported,
          "follow_cursor_unavailable_reason": self.follow_cursor_unavailable_reason,
          "update_tooltip": UPDATE_TOOLTIP,
          "artwork_size": self.artwork_size_for_window(),
          "follow_cursor_halo_size": FOLLOW_CURSOR_HALO_SIZE_LOGICAL,
          "follow_cursor_window_size": FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
          "size_presets": size_presets,
           "use_native_menu": native_menu_available(),
        });
        format!("window.__BB_INIT__ = {payload};")
    }

    fn current_window_physical_position(&self) -> Option<PhysicalPosition<i32>> {
        self.window.as_ref()?.outer_position().ok()
    }

    fn follow_monitor(monitor: &MonitorHandle) -> FollowMonitor {
        FollowMonitor {
            // winit does not expose a platform-independent work-area API. Use
            // the monitor bounds for this prototype; platform-specific work
            // areas can be added with the future tray/control-plane work.
            work_area: ScreenRect::from_monitor(monitor),
            scale_factor: monitor.scale_factor(),
        }
    }

    fn follow_placement_for_cursor(
        &self,
        event_loop: &ActiveEventLoop,
        cursor: CursorPosition,
    ) -> Option<FollowPlacement> {
        let monitors = event_loop.available_monitors().collect::<Vec<_>>();
        if monitors.is_empty() {
            return None;
        }

        let physical_cursor = match cursor.space {
            CoordinateSpace::Physical => {
                if !cursor.x.is_finite() || !cursor.y.is_finite() {
                    return None;
                }
                PhysicalPosition::new(cursor.x.round() as i32, cursor.y.round() as i32)
            }
            CoordinateSpace::Logical => {
                if !cursor.x.is_finite() || !cursor.y.is_finite() {
                    return None;
                }
                let logical_cursor = LogicalPoint::new(cursor.x, cursor.y);
                monitors
                    .iter()
                    .find_map(|monitor| {
                        let snapshot = snapshot_monitor(monitor);
                        if !snapshot.contains_logical(logical_cursor) {
                            return None;
                        }
                        let physical = logical_cursor_to_physical(logical_cursor, &snapshot)?;
                        Some(PhysicalPosition::new(physical.x, physical.y))
                    })
                    .or_else(|| {
                        let monitor = self
                            .window
                            .as_ref()
                            .and_then(|window| window.current_monitor())
                            .or_else(|| event_loop.primary_monitor())
                            .or_else(|| monitors.first().cloned())?;
                        let snapshot = snapshot_monitor(&monitor);
                        let physical = logical_cursor_to_physical(logical_cursor, &snapshot)?;
                        Some(PhysicalPosition::new(physical.x, physical.y))
                    })?
            }
        };

        let monitor = monitors
            .iter()
            .find(|monitor| ScreenRect::from_monitor(monitor).contains(physical_cursor))
            .or_else(|| {
                self.window
                    .as_ref()
                    .and_then(|window| window.current_monitor())
                    .as_ref()
                    .and_then(|current| {
                        monitors.iter().find(|monitor| {
                            monitor.position() == current.position()
                                && monitor.size() == current.size()
                        })
                    })
            })
            .or_else(|| monitors.first())?;

        Some(FollowPlacement {
            cursor: physical_cursor,
            monitor: Self::follow_monitor(monitor),
        })
    }

    fn follow_window_position(&self, placement: FollowPlacement) -> PhysicalPosition<i32> {
        let artwork_size = self.artwork_size_for_window();
        let target_dimensions =
            self.animation_window_dimensions_for(artwork_size, self.animation_bounds);
        let window_center = LogicalPosition::new(
            target_dimensions.width / 2.0,
            target_dimensions.height / 2.0,
        );
        follow_window_origin(
            placement.cursor,
            placement.monitor.work_area,
            placement.monitor.scale_factor,
            target_dimensions,
            window_center,
        )
    }

    fn position_follow_cursor(&mut self, placement: FollowPlacement) {
        let next_position = self.follow_window_position(placement);
        self.follow_cursor_placement = Some(placement);
        if self.current_window_physical_position() != Some(next_position) {
            if let Some(window) = self.window.as_ref() {
                window.set_outer_position(next_position);
            }
        }
    }

    fn poll_follow_cursor(&mut self, event_loop: &ActiveEventLoop) {
        if !self.follow_cursor_active {
            return;
        }
        let sample = match self.cursor_source.as_mut() {
            Some(source) => source.sample(),
            None => Err(CursorError::Unavailable(
                "cursor following has no cursor provider",
            )),
        };
        let sample = match sample {
            Ok(sample) => sample,
            Err(error) => {
                log_stderr!("warning: cursor following stopped: {error}");
                self.set_follow_cursor(event_loop, false);
                return;
            }
        };
        let Some(placement) = self.follow_placement_for_cursor(event_loop, sample) else {
            log_stderr!("warning: cursor following stopped: unable to locate the active monitor");
            self.set_follow_cursor(event_loop, false);
            return;
        };
        self.position_follow_cursor(placement);
    }

    fn set_follow_cursor(&mut self, event_loop: &ActiveEventLoop, enabled: bool) {
        if enabled {
            if self.follow_cursor_active {
                return;
            }
            if !self.follow_cursor_supported {
                log_stderr!(
                    "warning: follow cursor requested but {}",
                    self.follow_cursor_unavailable_reason
                );
                return;
            }
            let sample = match self.cursor_source.as_mut() {
                Some(source) => match source.sample() {
                    Ok(sample) => sample,
                    Err(error) => {
                        log_stderr!("warning: cannot enable cursor following: {error}");
                        return;
                    }
                },
                None => {
                    log_stderr!("warning: cannot enable cursor following without a provider");
                    return;
                }
            };
            let Some(placement) = self.follow_placement_for_cursor(event_loop, sample) else {
                log_stderr!("warning: cannot enable cursor following without an active monitor");
                return;
            };
            let previous_position = self.current_window_physical_position();
            let Some(window) = self.window.as_ref() else {
                return;
            };
            if let Err(error) = window.set_cursor_hittest(false) {
                log_stderr!("warning: failed to make widget click-through: {error}");
                return;
            }
            self.follow_cursor_previous_position = previous_position;
            self.follow_cursor_active = true;
            self.stop_manual_drag();
            self.enforce_fixed_widget_size();
            self.telemetry_follow_cursor_change(true);
            self.sync_update_menu_state();
            self.sync_update_state_to_webview();
            self.position_follow_cursor(placement);
            return;
        }

        if !self.follow_cursor_active {
            return;
        }
        if let Some(window) = self.window.as_ref() {
            if let Err(error) = window.set_cursor_hittest(true) {
                log_stderr!("warning: failed to restore widget cursor hit testing: {error}");
            }
        }
        self.follow_cursor_active = false;
        self.follow_cursor_placement = None;
        self.enforce_fixed_widget_size();
        if let Some(previous_position) = self.follow_cursor_previous_position.take() {
            if let Some(window) = self.window.as_ref() {
                window.set_outer_position(previous_position);
            }
        }
        self.telemetry_follow_cursor_change(false);
        self.sync_update_menu_state();
        self.sync_update_state_to_webview();
    }

    fn apply_size(&mut self, size: f64) {
        let window = match self.window.as_ref() {
            Some(window) => window,
            None => return,
        };
        let size = clamp_size(size);
        self.settings.size = size;
        let artwork_size = self.artwork_size_for_window();
        let target_dimensions =
            self.animation_window_dimensions_for(artwork_size, self.animation_bounds);
        let old_dimensions = window.inner_size().to_logical::<f64>(window.scale_factor());
        window.set_min_inner_size(Some(target_dimensions));
        window.set_max_inner_size(Some(target_dimensions));
        let _ = window.request_inner_size(target_dimensions);

        if let Some(current_pos) = logical_outer_position(Some(window)) {
            let center_x = current_pos.x + old_dimensions.width / 2.0;
            let center_y = current_pos.y + old_dimensions.height / 2.0;
            let next_x = (center_x - target_dimensions.width / 2.0).round() as i32;
            let next_y = (center_y - target_dimensions.height / 2.0).round() as i32;
            window.set_outer_position(LogicalPosition::new(next_x, next_y));
            if !self.follow_cursor_active {
                if let Ok(physical) = window.outer_position() {
                    self.settings.physical_x = Some(physical.x);
                    self.settings.physical_y = Some(physical.y);
                }
            }
        }
        self.apply_main_webview_state(serde_json::json!({
            "artwork_size": artwork_size,
        }));
        if self.follow_cursor_active {
            if let Some(placement) = self.follow_cursor_placement {
                self.position_follow_cursor(placement);
            }
        }
    }

    fn sync_breathing_pattern_to_webview(&self) {
        self.apply_main_webview_state(serde_json::json!({
            "breathing_pattern": self.settings.breathing_pattern,
            "active_breathing_preset_id": self.settings.active_breathing_preset_id,
            "breathing_presets": self.breathing_pattern_menu_presets_payload(),
        }));
    }

    fn sync_breathing_pattern_surfaces(&mut self) {
        self.sync_breathing_pattern_to_webview();
        self.rebuild_native_context_menu();
        self.sync_breathing_pattern_editor_state();
    }

    fn next_saved_breathing_preset_id(&self, name: &str) -> String {
        app_core::next_saved_preset_id(name, &self.settings.saved_breathing_presets, |id| {
            built_in_breathing_preset(id).is_some()
        })
    }

    fn apply_breathing_pattern(&mut self, preset_id: String, mut pattern: BreathingPattern) {
        pattern.sanitize();
        if preset_id == BREATHING_PRESET_ID_CUSTOM {
            self.settings.active_breathing_preset_id = BREATHING_PRESET_ID_CUSTOM.to_string();
            self.settings.breathing_pattern = pattern;
        } else if let Some(preset) = built_in_breathing_preset(&preset_id) {
            self.settings.active_breathing_preset_id = preset.id.to_string();
            self.settings.breathing_pattern = preset.pattern;
        } else if let Some(preset) = self
            .settings
            .saved_breathing_presets
            .iter()
            .find(|preset| preset.id == preset_id)
        {
            self.settings.active_breathing_preset_id = preset.id.clone();
            self.settings.breathing_pattern = preset.pattern.clone();
        } else {
            self.settings.active_breathing_preset_id = BREATHING_PRESET_ID_CUSTOM.to_string();
            self.settings.breathing_pattern = pattern;
        }
        self.sync_breathing_pattern_surfaces();
    }

    fn save_breathing_preset(
        &mut self,
        name: String,
        mut pattern: BreathingPattern,
    ) -> Option<SavedBreathingPreset> {
        let trimmed_name = name.trim();
        if trimmed_name.is_empty() {
            return None;
        }
        pattern.sanitize();
        let id = self.next_saved_breathing_preset_id(trimmed_name);
        let preset = SavedBreathingPreset {
            id: id.clone(),
            name: trimmed_name.to_string(),
            pattern,
        };
        self.settings.saved_breathing_presets.push(preset.clone());
        self.settings.active_breathing_preset_id = preset.id.clone();
        self.settings.breathing_pattern = preset.pattern.clone();
        self.sync_breathing_pattern_surfaces();
        Some(preset)
    }

    fn delete_breathing_preset(&mut self, preset_id: &str) -> Option<SavedBreathingPreset> {
        let removed = if let Some(index) = self
            .settings
            .saved_breathing_presets
            .iter()
            .position(|preset| preset.id == preset_id)
        {
            self.settings.saved_breathing_presets.remove(index)
        } else {
            let preset = built_in_breathing_preset(preset_id)?;
            if !self
                .settings
                .hidden_breathing_preset_ids
                .iter()
                .any(|id| id == preset_id)
            {
                self.settings
                    .hidden_breathing_preset_ids
                    .push(preset_id.to_string());
            }
            SavedBreathingPreset {
                id: preset.id.to_string(),
                name: preset.name.to_string(),
                pattern: preset.pattern,
            }
        };
        if self.settings.active_breathing_preset_id == removed.id {
            if let Some(next_builtin) = built_in_breathing_presets().into_iter().find(|preset| {
                !self
                    .settings
                    .hidden_breathing_preset_ids
                    .iter()
                    .any(|id| id == preset.id)
            }) {
                self.settings.active_breathing_preset_id = next_builtin.id.to_string();
                self.settings.breathing_pattern = next_builtin.pattern;
            } else if let Some(next_saved) = self.settings.saved_breathing_presets.first() {
                self.settings.active_breathing_preset_id = next_saved.id.clone();
                self.settings.breathing_pattern = next_saved.pattern.clone();
            } else {
                self.settings.active_breathing_preset_id = BREATHING_PRESET_ID_CUSTOM.to_string();
            }
        }
        self.sync_breathing_pattern_surfaces();
        Some(removed)
    }

    fn sync_window_visibility(&self) {
        if let Some(window) = self.window.as_ref() {
            window.set_visible(self.activity_mode != ActivityMode::Snoozed);
        }
    }

    fn cancel_snooze(&mut self) {
        self.snooze_generation = self.snooze_generation.wrapping_add(1);
        self.snooze_deadline = None;
    }

    fn reconcile_snooze_after_resume(&mut self) {
        let Some(deadline) = self.snooze_deadline else {
            return;
        };
        if self.activity_mode != ActivityMode::Snoozed {
            return;
        }
        if SystemTime::now() < deadline {
            return;
        }
        if self.resume_from_snooze() {
            self.telemetry_activity_state(
                ActivityState::Active,
                ActivityTrigger::SnoozeExpired,
                None,
            );
            self.save_settings();
        }
    }

    fn sync_main_webview_paused_state(&self, paused: bool) {
        self.apply_main_webview_state(serde_json::json!({
            "paused": paused,
        }));
    }

    fn apply_paused(&mut self, paused: bool) {
        self.cancel_snooze();
        self.settings.paused = paused;
        self.activity_mode = if paused {
            ActivityMode::Paused
        } else {
            ActivityMode::Active
        };
        self.sync_window_visibility();
        self.sync_main_webview_paused_state(paused);
    }

    fn schedule_snooze_expiry(&self, generation: u64, duration: Duration) {
        let Some(proxy) = self.event_loop_proxy.as_ref().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            let _ = proxy.send_event(AppEvent::SnoozeExpired(generation));
        });
    }

    fn apply_snooze(&mut self, minutes: u64) -> Option<u64> {
        let minutes = minutes.max(1);
        let duration = Duration::from_secs(minutes.saturating_mul(60));
        self.close_custom_snooze_window();
        self.settings.paused = false;
        self.activity_mode = ActivityMode::Snoozed;
        self.snooze_deadline = Some(SystemTime::now() + duration);
        self.snooze_generation = self.snooze_generation.wrapping_add(1);
        let generation = self.snooze_generation;
        self.sync_window_visibility();
        self.sync_main_webview_paused_state(false);
        self.schedule_snooze_expiry(generation, duration);
        Some(duration.as_secs())
    }

    fn show_main_window_without_focus(&self) {
        if let Some(window) = self.window.as_ref() {
            window.set_visible(true);
        }
    }

    fn resume_from_snooze(&mut self) -> bool {
        if self.activity_mode != ActivityMode::Snoozed {
            return false;
        }
        self.cancel_snooze();
        self.settings.paused = false;
        self.activity_mode = ActivityMode::Active;
        self.sync_window_visibility();
        self.sync_main_webview_paused_state(false);
        self.show_main_window_without_focus();
        true
    }

    fn expire_snooze(&mut self, generation: u64) {
        if self.activity_mode != ActivityMode::Snoozed || self.snooze_generation != generation {
            return;
        }
        self.resume_from_snooze();
        self.telemetry_activity_state(ActivityState::Active, ActivityTrigger::SnoozeExpired, None);
        self.save_settings();
    }

    fn handle_instance_activate(&mut self) {
        if self.resume_from_snooze() {
            self.telemetry_activity_state(ActivityState::Active, ActivityTrigger::Relaunch, None);
            self.save_settings();
        }
        self.show_main_window_without_focus();
    }

    fn set_paused_from_user_action(&mut self, paused: bool) {
        let action = if paused {
            MenuAction::Pause
        } else {
            MenuAction::Resume
        };
        self.telemetry_menu_action(action, None);
        self.apply_paused(paused);
        let next_state = if paused {
            ActivityState::Paused
        } else {
            ActivityState::Active
        };
        self.telemetry_activity_state(next_state, ActivityTrigger::Manual, None);
        self.save_settings();
    }

    fn set_snooze_from_user_action(&mut self, minutes: u64) {
        self.telemetry_menu_action(MenuAction::Snooze, None);
        if let Some(requested_duration_sec) = self.apply_snooze(minutes) {
            self.telemetry_activity_state(
                ActivityState::Snoozed,
                ActivityTrigger::SnoozeTimed,
                Some(requested_duration_sec),
            );
            self.save_settings();
        }
    }

    fn apply_size_slot_from_user_action(&mut self, size_slot: usize) {
        let presets = self.current_size_presets();
        let Some(size) = presets.get(size_slot).copied() else {
            return;
        };
        let size_target = size_target_label(size_slot);
        self.telemetry_menu_action(MenuAction::SizeChange, size_target);
        self.apply_size(size);
        self.save_settings();
    }

    fn apply_breathing_pattern_from_user_action(
        &mut self,
        preset_id: String,
        pattern: BreathingPattern,
        preset_name: Option<String>,
    ) {
        self.apply_breathing_pattern(preset_id, pattern);
        self.telemetry_breathing_pattern_change(
            "applied",
            &self.settings.active_breathing_preset_id,
            preset_name.as_deref(),
            &self.settings.breathing_pattern,
        );
        self.save_settings();
    }

    fn save_breathing_preset_from_user_action(&mut self, name: String, pattern: BreathingPattern) {
        if let Some(preset) = self.save_breathing_preset(name, pattern) {
            self.telemetry_breathing_pattern_change(
                "saved",
                &preset.id,
                Some(&preset.name),
                &self.settings.breathing_pattern,
            );
            self.close_breathing_pattern_window();
            self.save_settings();
        }
    }

    fn delete_breathing_preset_from_user_action(&mut self, preset_id: &str) {
        if let Some(preset) = self.delete_breathing_preset(preset_id) {
            self.telemetry_breathing_pattern_change(
                "deleted",
                &preset.id,
                Some(&preset.name),
                &preset.pattern,
            );
            self.save_settings();
        }
    }

    fn update_position_from_physical(&mut self, physical: PhysicalPosition<i32>) {
        if self.follow_cursor_active {
            return;
        }
        if let Some(window) = self.window.as_ref() {
            self.settings.physical_x = Some(physical.x);
            self.settings.physical_y = Some(physical.y);
            let current_monitor = window.current_monitor();
            self.settings.monitor = current_monitor.as_ref().map(persisted_monitor);
            if let Some(monitor) = current_monitor {
                self.apply_size_presets_for_monitor(&monitor);
            }
        }
    }

    fn reset_widget(&mut self, event_loop: &ActiveEventLoop) {
        if self.follow_cursor_active {
            return;
        }
        let monitor = self
            .window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .or_else(|| event_loop.primary_monitor())
            .or_else(|| event_loop.available_monitors().next());
        let monitor_snapshot = monitor.as_ref().map(snapshot_monitor);
        let reset_size = reset_size_for_monitor(monitor_snapshot.as_ref());
        self.apply_size(reset_size);
        self.apply_breathing_pattern(
            BREATHING_PRESET_ID_COHERENT.to_string(),
            BreathingPattern::coherent(),
        );
        self.close_custom_snooze_window();
        self.close_breathing_pattern_window();
        self.apply_paused(false);
        self.telemetry_activity_state(ActivityState::Active, ActivityTrigger::Manual, None);
        if let Some(window) = self.window.as_ref() {
            if let Some(monitor_snapshot) = monitor_snapshot {
                let policy_position =
                    default_corner_position(&monitor_snapshot, self.settings.size);
                let pos = PhysicalPosition::new(policy_position.x, policy_position.y);
                window.set_outer_position(pos);
                self.settings.physical_x = Some(pos.x);
                self.settings.physical_y = Some(pos.y);
                self.settings.monitor = Some(monitor_snapshot.persisted());
            }
        }
    }

    fn handle_ipc_command(&mut self, event_loop: &ActiveEventLoop, command: IpcCommand) {
        match command {
            IpcCommand::Quit => {
                self.quit_app(event_loop);
            }
            IpcCommand::SetPaused { paused } => {
                self.set_paused_from_user_action(paused);
            }
            IpcCommand::SetSnooze { minutes } => {
                self.set_snooze_from_user_action(minutes);
            }
            IpcCommand::ShowBreathingPattern => {
                self.open_breathing_pattern_window(event_loop);
            }
            IpcCommand::CloseBreathingPattern => self.cancel_breathing_pattern_window(),
            IpcCommand::ApplyBreathingPattern { preset_id, pattern } => {
                if preset_id == BREATHING_PRESET_ID_CUSTOM {
                    return;
                }
                let preset_name = built_in_breathing_preset(&preset_id)
                    .map(|preset| preset.name.to_string())
                    .or_else(|| {
                        self.settings
                            .saved_breathing_presets
                            .iter()
                            .find(|preset| preset.id == preset_id)
                            .map(|preset| preset.name.clone())
                    });
                self.apply_breathing_pattern_from_user_action(preset_id, pattern, preset_name);
            }
            IpcCommand::SaveBreathingPreset { name, pattern } => {
                self.save_breathing_preset_from_user_action(name, pattern);
            }
            IpcCommand::DeleteBreathingPreset { preset_id } => {
                self.delete_breathing_preset_from_user_action(&preset_id);
            }
            IpcCommand::SetUsageDataSharing { enabled } => {
                self.apply_usage_data_sharing(enabled);
            }
            IpcCommand::SetCrashReportsSharing { enabled } => {
                self.apply_crash_reports_sharing(enabled);
            }
            IpcCommand::AnalyticsMenuOpened => {
                self.telemetry_menu_action(MenuAction::AnalyticsMenu, None);
            }
            IpcCommand::ShowTelemetryInfo => {
                self.open_telemetry_info_window(event_loop);
            }
            IpcCommand::CloseTelemetryInfo => self.close_telemetry_info_window(),
            IpcCommand::ShowCustomSnooze => self.open_custom_snooze_window(event_loop),
            IpcCommand::CloseCustomSnooze => self.close_custom_snooze_window(),
            IpcCommand::UpdatePrimaryAction => {
                self.handle_update_primary_action(event_loop);
            }
            IpcCommand::ShowUpdateDialog => {
                self.open_update_dialog_window(event_loop);
                self.set_update_dialog_mode_result();
            }
            IpcCommand::DismissUpdateBadge => {
                self.dismiss_current_update_badge();
            }
            IpcCommand::SetIgnoreCurrentUpdate { ignored } => {
                self.apply_ignore_current_update(ignored);
            }
            IpcCommand::CloseUpdateDialog => self.close_update_dialog_window(),
            IpcCommand::DownloadUpdate => {
                self.close_update_dialog_window();
                self.launch_update_download("dialog");
            }
            IpcCommand::ShowContextMenu { x, y } => {
                self.telemetry_menu_action(MenuAction::ContextMenu, None);
                self.show_native_context_menu(x, y);
            }
            IpcCommand::Resize { delta, fine } => {
                let next = apply_resize_step(self.settings.size, delta, fine);
                self.apply_size(next);
                self.save_settings();
            }
            IpcCommand::SetSize { size } => {
                let size_target = self.size_target_for_value(size);
                self.telemetry_menu_action(MenuAction::SizeChange, size_target);
                self.apply_size(size);
                self.save_settings();
            }
            IpcCommand::StartDrag { screen_x, screen_y } => {
                self.start_manual_drag(screen_x, screen_y)
            }
            IpcCommand::DragTo { screen_x, screen_y } => self.drag_to(screen_x, screen_y),
            IpcCommand::EndDrag => self.stop_manual_drag(),
            IpcCommand::SetFollowCursor { enabled } => {
                self.set_follow_cursor(event_loop, enabled);
            }
            IpcCommand::SetAnimationBounds {
                x,
                y,
                width,
                height,
                badge_visible,
            } => self.set_animation_bounds(AnimationBounds {
                x,
                y,
                width,
                height,
                badge_visible,
            }),
            IpcCommand::Reset => {
                self.telemetry_menu_action(MenuAction::Reset, None);
                self.reset_widget(event_loop);
                self.save_settings();
            }
        }
    }

    fn apply_size_presets_for_monitor(&self, monitor: &MonitorHandle) {
        let presets = size_presets_for_monitor(&snapshot_monitor(monitor));
        self.apply_main_webview_state(serde_json::json!({
            "size_presets": presets,
        }));
    }

    fn apply_main_webview_state(&self, state: serde_json::Value) {
        let Some(webview) = self.webview.as_ref() else {
            return;
        };
        let js = format!("window.breathBallApplyState({state});");
        let _ = webview.evaluate_script(&js);
    }

    fn size_target_for_value(&self, size: f64) -> Option<&'static str> {
        self.window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .map(|monitor| size_presets_for_monitor(&snapshot_monitor(&monitor)))
            .and_then(|presets| {
                presets
                    .iter()
                    .enumerate()
                    .find(|(_, preset)| (**preset - size).abs() <= 0.5)
                    .map(|(index, _)| index)
            })
            .and_then(size_target_label)
    }

    fn current_size_presets(&self) -> [f64; 4] {
        self.window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .map(|monitor| size_presets_for_monitor(&snapshot_monitor(&monitor)))
            .unwrap_or(DEFAULT_SIZE_PRESETS)
    }

    fn show_native_context_menu(&mut self, x: i32, y: i32) {
        self.rebuild_native_context_menu();
        let Some(menu) = self.native_context_menu.as_ref() else {
            return;
        };
        let Some(window) = self.window.as_ref() else {
            return;
        };
        menu.show(window, x, y);
    }

    fn handle_native_menu_activation(&mut self, event_loop: &ActiveEventLoop, id: &str) {
        match id {
            MENU_ID_PAUSE => {
                self.set_paused_from_user_action(!self.settings.paused);
            }
            MENU_ID_FOLLOW_CURSOR => self.set_follow_cursor(event_loop, !self.follow_cursor_active),
            MENU_ID_SNOOZE_5 | MENU_ID_SNOOZE_10 | MENU_ID_SNOOZE_15 | MENU_ID_SNOOZE_30
            | MENU_ID_SNOOZE_60 => {
                let Some(minutes) = snooze_minutes_for_menu_id(id) else {
                    return;
                };
                self.set_snooze_from_user_action(minutes);
            }
            MENU_ID_SNOOZE_CUSTOM => self.open_custom_snooze_window(event_loop),
            MENU_ID_SIZE_S | MENU_ID_SIZE_M | MENU_ID_SIZE_L | MENU_ID_SIZE_XL => {
                let Some(size_slot) = size_slot_for_menu_id(id) else {
                    return;
                };
                self.apply_size_slot_from_user_action(size_slot);
            }
            MENU_ID_BREATHING_COHERENT => {
                self.apply_breathing_pattern_from_user_action(
                    BREATHING_PRESET_ID_COHERENT.to_string(),
                    BreathingPattern::coherent(),
                    Some("coherent breathing".to_string()),
                );
            }
            MENU_ID_BREATHING_BOX => {
                self.apply_breathing_pattern_from_user_action(
                    "box_breathing".to_string(),
                    BreathingPattern::box_breathing(),
                    Some("box breathing".to_string()),
                );
            }
            MENU_ID_BREATHING_479 => {
                self.apply_breathing_pattern_from_user_action(
                    "4_7_9".to_string(),
                    BreathingPattern::four_seven_nine(),
                    Some("4-7-9".to_string()),
                );
            }
            MENU_ID_BREATHING_EDIT => self.open_breathing_pattern_window(event_loop),
            MENU_ID_RESET => {
                self.telemetry_menu_action(MenuAction::Reset, None);
                self.reset_widget(event_loop);
                self.save_settings();
            }
            MENU_ID_LAUNCH_AT_LOGIN => {
                let enabled = !self.settings.launch_at_login;
                self.telemetry_launch_at_login_change(enabled);
                self.apply_launch_at_login(enabled);
            }
            MENU_ID_QUIT => {
                self.quit_app(event_loop);
            }
            MENU_ID_USAGE_ON => {
                self.apply_usage_data_sharing(true);
            }
            MENU_ID_USAGE_OFF => {
                self.apply_usage_data_sharing(false);
            }
            MENU_ID_CRASH_ON => {
                self.apply_crash_reports_sharing(true);
            }
            MENU_ID_CRASH_OFF => {
                self.apply_crash_reports_sharing(false);
            }
            MENU_ID_ANALYTICS_INFO => self.open_telemetry_info_window(event_loop),
            MENU_ID_UPDATE_PRIMARY => self.handle_update_primary_action(event_loop),
            MENU_ID_UPDATE_IGNORE_CURRENT => {
                let ignored = !self.updates.is_ignoring_current_update();
                self.apply_ignore_current_update(ignored);
            }
            #[cfg(debug_assertions)]
            MENU_ID_SIMULATE_PENDING_UPDATE => {
                self.update_check
                    .set_simulate_pending_update(!self.update_check.simulate_pending_update());
            }
            #[cfg(debug_assertions)]
            MENU_ID_FORCE_BACKGROUND_UPDATE_CHECK => self.run_forced_background_update_check(),
            #[cfg(debug_assertions)]
            MENU_ID_CLEAR_UPDATE_NOTIFICATION_DISMISSED => {
                self.clear_update_notification_dismissed();
            }
            MENU_ID_COPY_DIAGNOSTICS => self.copy_diagnostics_summary(),
            MENU_ID_FILE_BUG_GITHUB => open_external_url(&github_issues_url()),
            MENU_ID_FILE_BUG_EMAIL => open_external_url(&support_email_mailto()),
            _ => {
                if let Some(preset_id) = deleted_breathing_preset_id_from_menu_id(id) {
                    self.delete_breathing_preset_from_user_action(preset_id);
                    return;
                }
                if let Some(preset_id) = saved_breathing_preset_id_from_menu_id(id) {
                    let preset_name = self
                        .settings
                        .saved_breathing_presets
                        .iter()
                        .find(|preset| preset.id == preset_id)
                        .map(|preset| preset.name.clone());
                    let pattern = self.settings.breathing_pattern.clone();
                    self.apply_breathing_pattern_from_user_action(
                        preset_id.to_string(),
                        pattern,
                        preset_name,
                    );
                }
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            self.handle_app_resume();
            return;
        }
        self.config_path = Self::config_path();
        let settings_exist = self.config_path.as_ref().is_some_and(|path| path.exists());
        let settings_load_result = load_settings_result(self.config_path.as_deref());
        self.settings = settings_load_result.settings;
        self.settings_load_error = settings_load_result.load_error;
        self.settings_backup_pending = self.settings_load_error.is_some();
        self.startup_provenance = if self.settings_load_error.is_some() {
            "fallback_after_settings_error".to_string()
        } else if settings_exist {
            "restored_settings".to_string()
        } else {
            "fresh_defaults".to_string()
        };
        if let Some(error) = self.settings_load_error.as_ref() {
            log_stderr!("warning: {error}");
        }
        self.updates.badge_snoozed_version = self.settings.update_badge_snoozed_version.clone();
        self.updates.badge_snoozed_at_epoch_seconds =
            self.settings.update_badge_snoozed_at_epoch_seconds;
        self.updates.ignored_version = self.settings.ignored_update_version.clone();
        self.updates.latest_version = self.settings.cached_latest_update_version.clone();
        self.telemetry
            .set_usage_enabled(self.settings.usage_data_sharing);
        self.telemetry
            .set_crash_enabled(self.settings.crash_reports_sharing);
        self.reconcile_launch_at_login();
        if !settings_exist {
            if let Some(primary) = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())
            {
                let primary = snapshot_monitor(&primary);
                self.settings.size = default_size_for_monitor(&primary);
            }
        }
        self.activity_mode = if self.settings.paused {
            ActivityMode::Paused
        } else {
            ActivityMode::Active
        };
        self.snooze_deadline = None;

        let initial_window_size = self.widget_window_dimensions(self.settings.size);
        let mut window_attributes = host::configure_main_window(initial_window_size);

        if let Some(position) = self.choose_initial_position(event_loop, self.settings.size) {
            window_attributes = window_attributes.with_position(position);
            self.settings.physical_x = Some(position.x);
            self.settings.physical_y = Some(position.y);
        }

        let window = match create_main_window(event_loop, window_attributes) {
            Ok(window) => window,
            Err(error) => {
                self.telemetry.track_error(
                    EventName::AppError,
                    serde_json::json!({
                        "category": "window_create",
                        "severity": "error",
                        "recoverable": false,
                    }),
                );
                self.finish_session(SessionEndReason::StartupFailure);
                self.startup_error = Some(format!("failed to create app window: {error}"));
                event_loop.exit();
                return;
            }
        };
        let window_id = window.id();
        let cursor_provider = CursorProvider::for_window(&window);
        self.follow_cursor_supported = cursor_provider.is_supported();
        self.follow_cursor_unavailable_reason = if self.follow_cursor_supported {
            ""
        } else {
            cursor_provider.unavailable_reason()
        };
        self.cursor_source = Some(Box::new(cursor_provider));
        host::configure_created_window(&window);
        self.settings.monitor = window.current_monitor().as_ref().map(persisted_monitor);

        let startup_monitor = window
            .current_monitor()
            .or_else(|| event_loop.primary_monitor())
            .or_else(|| event_loop.available_monitors().next());
        let size_presets = startup_monitor
            .as_ref()
            .map(|monitor| size_presets_for_monitor(&snapshot_monitor(monitor)))
            .unwrap_or(DEFAULT_SIZE_PRESETS);
        let init_script = self.build_init_script(size_presets);
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
            self.telemetry.track_error(
                EventName::AppError,
                serde_json::json!({
                    "category": "event_proxy",
                    "severity": "error",
                    "recoverable": false,
                }),
            );
            self.finish_session(SessionEndReason::StartupFailure);
            self.startup_error = Some("failed to initialize event loop proxy".to_string());
            event_loop.exit();
            return;
        };
        let webview = match build_main_webview(&window, breath_html(), &init_script, &ipc_proxy) {
            Ok(webview) => webview,
            Err(error) => {
                self.telemetry.track_error(
                    EventName::AppError,
                    serde_json::json!({
                        "category": "webview_create",
                        "severity": "error",
                        "recoverable": false,
                    }),
                );
                self.finish_session(SessionEndReason::StartupFailure);
                self.startup_error = Some(format!("failed to create webview: {error}"));
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.window_id = Some(window_id);
        self.webview = Some(webview);
        self.rebuild_native_context_menu();
        self.sync_update_surfaces();
        emit_startup_telemetry(
            &self.telemetry,
            self.current_activity_state(),
            self.heartbeat_snapshot(),
        );
        if self.telemetry_install_first_run {
            self.telemetry.track(
                EventName::InstallFirstRun,
                serde_json::json!({
                    "usage_sharing_enabled_default": true,
                    "crash_sharing_enabled_default": true,
                }),
            );
            self.telemetry_install_first_run = false;
        }
        match create_tray_icon(self.native_context_menu.as_ref()) {
            Ok(tray_icon) => self.tray_icon = tray_icon,
            Err(error) => {
                self.telemetry.track_error(
                    EventName::AppError,
                    serde_json::json!({
                        "category": "tray_icon_create",
                        "severity": "warn",
                        "recoverable": true,
                    }),
                );
                log_stderr!("warning: failed to create tray icon: {error}");
            }
        }
        self.enforce_fixed_widget_size();
        self.sync_webview_bounds();
        if self.settings_load_error.is_none() {
            self.save_settings();
        }

        let update_check = self.update_check.clone();
        if let Some(proxy) = self.event_loop_proxy.as_ref().cloned() {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(UPDATE_CHECK_STARTUP_DELAY_SEC));
                loop {
                    let result = update_check.check();
                    let _ = proxy.send_event(AppEvent::UpdateCheckFinished(
                        result,
                        UpdateCheckSource::Background,
                    ));
                    std::thread::sleep(Duration::from_secs(UPDATE_CHECK_BACKGROUND_INTERVAL_SEC));
                }
            });
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.handle_app_suspend();
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.follow_cursor_active {
            self.poll_follow_cursor(event_loop);
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + FOLLOW_CURSOR_POLL_INTERVAL,
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) == self.custom_snooze_window_id {
            self.handle_custom_snooze_window_event(event);
            return;
        }
        if Some(window_id) == self.breathing_pattern_window_id {
            self.handle_breathing_pattern_window_event(event);
            return;
        }
        if Some(window_id) == self.telemetry_info_window_id {
            self.handle_telemetry_info_window_event(event);
            return;
        }
        if Some(window_id) == self.update_dialog_window_id {
            self.handle_update_dialog_window_event(event);
            return;
        }
        if Some(window_id) != self.window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.save_settings();
                self.finish_session(SessionEndReason::WindowClose);
                event_loop.exit();
            }
            WindowEvent::Moved(position) => {
                self.enforce_fixed_widget_size();
                if !self.follow_cursor_active {
                    self.update_position_from_physical(position);
                    self.save_settings();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.window.as_ref() {
                    let current_monitor = window.current_monitor();
                    if !self.follow_cursor_active {
                        self.settings.monitor = current_monitor.as_ref().map(persisted_monitor);
                    }
                    if let Some(monitor) = current_monitor {
                        self.apply_size_presets_for_monitor(&monitor);
                    }
                }
                if !self.follow_cursor_active {
                    self.save_settings();
                }
            }
            WindowEvent::Resized(_) => {
                self.enforce_fixed_widget_size();
                self.sync_webview_bounds();
                if let Some(window) = self.window.as_ref() {
                    if let Some(monitor) = window.current_monitor() {
                        self.apply_size_presets_for_monitor(&monitor);
                    }
                }
            }
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Released,
                ..
            } => self.stop_manual_drag(),
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ExitRequested => {
                self.save_settings();
                self.finish_session(SessionEndReason::CtrlC);
                event_loop.exit();
            }
            AppEvent::Ipc(payload) => {
                let command = match serde_json::from_str::<IpcCommand>(&payload) {
                    Ok(command) => command,
                    Err(error) => {
                        self.telemetry.track_error(
                            EventName::AppError,
                            serde_json::json!({
                                "category": "ipc_parse",
                                "severity": "warn",
                                "recoverable": true,
                            }),
                        );
                        log_stderr!("warning: ignored malformed ipc command: {error}");
                        return;
                    }
                };
                self.handle_ipc_command(event_loop, command);
            }
            AppEvent::InstanceActivate => self.handle_instance_activate(),
            AppEvent::TelemetryHeartbeat => self.telemetry_heartbeat(),
            AppEvent::SnoozeExpired(generation) => self.expire_snooze(generation),
            AppEvent::UpdateCheckFinished(result, source) => {
                let latest_version = result.latest_version.clone();
                let has_update_available = latest_version
                    .as_deref()
                    .map(|latest| is_newer_version(latest, env!("CARGO_PKG_VERSION")))
                    .unwrap_or(false);
                self.telemetry_update_flow(
                    "check_completed",
                    serde_json::json!({
                        "source": source.as_str(),
                        "latest_version": latest_version,
                        "has_update_available": has_update_available,
                    }),
                );
                self.apply_update_check_result(result);
                if source == UpdateCheckSource::Manual {
                    self.manual_update_check_in_flight = false;
                    self.set_update_dialog_mode_result();
                }
            }
            AppEvent::TrayIconClicked => {
                self.telemetry_menu_action(MenuAction::TrayMenu, None);
            }
            AppEvent::MenuActivated(id) => self.handle_native_menu_activation(event_loop, &id),
        }
    }
}

fn report_abnormal_exit(
    telemetry: &RuntimeTelemetryClient,
    reason: SessionEndReason,
    category: &str,
) -> std::process::ExitCode {
    telemetry.track_error(
        EventName::AppError,
        serde_json::json!({
            "category": category,
            "severity": "error",
            "recoverable": false,
        }),
    );
    telemetry.end_session(reason);
    telemetry.flush(Duration::from_secs(2));
    telemetry.shutdown(Duration::from_secs(2));
    std::process::ExitCode::from(1)
}

fn handle_child_window_event(
    app: &mut App,
    event: WindowEvent,
    close: fn(&mut App),
    sync_bounds: fn(&App),
) {
    match event {
        WindowEvent::CloseRequested => close(app),
        WindowEvent::Resized(_) => sync_bounds(app),
        _ => {}
    }
}

fn main() -> std::process::ExitCode {
    match diagnostics::init_logging() {
        Ok(path) => diagnostics::log_line(
            "INFO",
            &format!("logging initialized at {}", path.display()),
        ),
        Err(error) => eprintln!("failed to initialize diagnostics logging: {error}"),
    }

    let panic_telemetry = RuntimeTelemetryClient::from_env();
    let panic_telemetry_for_hook = panic_telemetry.clone();
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        log_stderr!("panic: {}", describe_panic(panic_info));
        panic_telemetry_for_hook.track_error(
            EventName::AppCrash,
            serde_json::json!({
                "category": "panic",
                "fatal": true,
            }),
        );
        panic_telemetry_for_hook.end_session(SessionEndReason::Panic);
        panic_telemetry_for_hook.flush(std::time::Duration::from_secs(2));
        panic_telemetry_for_hook.shutdown(std::time::Duration::from_secs(2));
        default_panic_hook(panic_info);
    }));

    let mut event_loop_builder = EventLoop::<AppEvent>::with_user_event();
    configure_event_loop_builder(&mut event_loop_builder);
    let event_loop = match event_loop_builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log_stderr!("error: failed to create event loop: {error}");
            return report_abnormal_exit(
                &panic_telemetry,
                SessionEndReason::StartupFailure,
                "event_loop_build",
            );
        }
    };
    let event_loop_proxy = event_loop.create_proxy();

    let _instance_guard = match host::start_instance(event_loop_proxy.clone()) {
        Ok(InstanceStart::Primary(guard)) => Some(guard),
        Ok(InstanceStart::AlreadyRunning) => return std::process::ExitCode::SUCCESS,
        Err(error) => {
            log_stderr!("warning: {error}");
            None
        }
    };

    let ctrlc_proxy = event_loop_proxy.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        let _ = ctrlc_proxy.send_event(AppEvent::ExitRequested);
    }) {
        log_stderr!("warning: failed to install ctrl-c handler: {error}");
    }
    install_menu_event_handler(event_loop_proxy.clone());
    install_tray_event_handler(event_loop_proxy.clone());
    let heartbeat_proxy = event_loop_proxy.clone();
    let heartbeat_interval = heartbeat_interval();
    std::thread::spawn(move || loop {
        std::thread::sleep(heartbeat_interval);
        if heartbeat_proxy
            .send_event(AppEvent::TelemetryHeartbeat)
            .is_err()
        {
            break;
        }
    });

    let mut app = App {
        event_loop_proxy: Some(event_loop_proxy),
        ..App::default()
    };

    if let Err(error) = event_loop.run_app(&mut app) {
        log_stderr!("error: app event loop failed: {error}");
        return report_abnormal_exit(
            &app.telemetry,
            SessionEndReason::EventLoopFailure,
            "event_loop",
        );
    }
    if let Some(error) = app.startup_error {
        log_stderr!("error: {error}");
        if !app.session_ended {
            return report_abnormal_exit(
                &app.telemetry,
                SessionEndReason::StartupFailure,
                "startup",
            );
        }
        return std::process::ExitCode::from(1);
    }
    app.finish_session(SessionEndReason::Unknown);
    std::process::ExitCode::SUCCESS
}

fn describe_panic(panic_info: &PanicHookInfo<'_>) -> String {
    let location = panic_info
        .location()
        .map(|location| format!("{}:{}", location.file(), location.line()))
        .unwrap_or_else(|| "unknown location".to_string());

    let payload = if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_string()
    };

    format!("{payload} ({location})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use downshift::telemetry::{Envelope, NoopSink, TelemetryError, TelemetrySink, TelemetryState};
    use serial_test::serial;
    #[cfg(any(unix, target_os = "macos"))]
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct CollectingSink {
        events: Arc<Mutex<Vec<Envelope>>>,
    }

    impl TelemetrySink for CollectingSink {
        fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError> {
            self.events
                .lock()
                .expect("collecting sink lock")
                .extend_from_slice(events);
            Ok(())
        }
    }

    fn telemetry_test_dir(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("downshift-main-{name}-{nanos}"))
    }

    fn telemetry_test_state() -> TelemetryState {
        TelemetryState {
            anon_user_id: uuid::Uuid::new_v4().to_string(),
            usage_enabled: true,
            crash_enabled: true,
            install_first_run: false,
        }
    }

    fn clear_external_contact_env() {
        std::env::remove_var("DOWNSHIFT_ENV");
        std::env::remove_var("DOWNSHIFT_DOWNLOAD_RELEASE_URL");
        std::env::remove_var("DOWNSHIFT_GITHUB_ISSUES_URL");
        std::env::remove_var("DOWNSHIFT_SUPPORT_EMAIL");
    }

    fn expected_download_release_url() -> &'static str {
        COMPILED_DOWNLOAD_RELEASE_URL.unwrap_or(UPDATE_DOWNLOAD_FALLBACK_URL)
    }

    fn expected_runtime_env() -> &'static str {
        COMPILED_ENV.unwrap_or("unset")
    }

    fn expected_github_issues_url() -> &'static str {
        COMPILED_GITHUB_ISSUES_URL.unwrap_or(DEFAULT_GITHUB_ISSUES_URL)
    }

    fn expected_support_email() -> &'static str {
        COMPILED_SUPPORT_EMAIL.unwrap_or(DEFAULT_SUPPORT_EMAIL)
    }

    #[test]
    fn heartbeat_interval_defaults_to_sixty_seconds() {
        assert_eq!(parse_heartbeat_interval_seconds(""), 60);
        assert_eq!(parse_heartbeat_interval_seconds("abc"), 60);
    }

    #[test]
    fn heartbeat_interval_uses_env_var_within_bounds() {
        assert_eq!(parse_heartbeat_interval_seconds("75"), 75);
    }

    #[test]
    fn heartbeat_interval_clamps_to_min_and_max() {
        assert_eq!(
            Duration::from_secs(parse_heartbeat_interval_seconds("1")),
            Duration::from_secs(MIN_HEARTBEAT_INTERVAL_SEC)
        );
        assert_eq!(
            Duration::from_secs(parse_heartbeat_interval_seconds("7200")),
            Duration::from_secs(MAX_HEARTBEAT_INTERVAL_SEC)
        );
    }

    #[test]
    #[serial]
    fn external_contact_values_use_dummy_defaults_outside_prod() {
        clear_external_contact_env();

        assert_eq!(download_release_url(), expected_download_release_url());
        assert_eq!(github_issues_url(), expected_github_issues_url());
        assert_eq!(support_email_address(), expected_support_email());
    }

    #[test]
    #[serial]
    fn runtime_prod_env_uses_compiled_defaults_instead_of_failing() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "prod");

        assert_eq!(download_release_url(), expected_download_release_url());
        assert_eq!(github_issues_url(), expected_github_issues_url());
        assert_eq!(support_email_address(), expected_support_email());
    }

    #[test]
    #[serial]
    fn external_contact_values_use_runtime_env_when_set() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "prod");
        std::env::set_var(
            "DOWNSHIFT_DOWNLOAD_RELEASE_URL",
            "https://example.com/download",
        );
        std::env::set_var("DOWNSHIFT_GITHUB_ISSUES_URL", "https://example.com/issues");
        std::env::set_var("DOWNSHIFT_SUPPORT_EMAIL", "support@example.com");

        assert_eq!(download_release_url(), expected_download_release_url());
        assert_eq!(github_issues_url(), "https://example.com/issues");
        assert_eq!(support_email_address(), "support@example.com");
    }

    #[test]
    #[serial]
    fn runtime_env_label_uses_compiled_default_when_runtime_var_missing() {
        clear_external_contact_env();

        assert_eq!(runtime_env_label(), expected_runtime_env());
    }

    #[test]
    #[serial]
    fn runtime_env_label_prefers_runtime_var_over_compiled_default() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "qa");

        assert_eq!(runtime_env_label(), "qa");
    }

    #[test]
    #[serial]
    fn startup_telemetry_emits_immediate_heartbeat() {
        let root = telemetry_test_dir("startup-heartbeat");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let captured_events = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let client = RuntimeTelemetryClient::new_with_sinks(
            telemetry_test_state(),
            Box::new(CollectingSink {
                events: captured_events.clone(),
            }),
            Box::new(NoopSink),
        );

        emit_startup_telemetry(
            &client,
            ActivityState::Active,
            HeartbeatSnapshot {
                state: "active".to_string(),
                paused: false,
                snoozed: false,
                active_breathing_preset_id: BREATHING_PRESET_ID_COHERENT.to_string(),
                breathing_pattern: BreathingPattern::coherent(),
                width_px: 240,
                height_px: 240,
                usage_enabled: true,
                crash_enabled: true,
            },
        );
        client.flush(Duration::from_millis(400));
        client.shutdown(Duration::from_millis(400));

        let event_names = captured_events
            .lock()
            .expect("captured events lock")
            .iter()
            .map(|event| event.event_name)
            .collect::<Vec<_>>();

        assert_eq!(
            event_names,
            vec![
                EventName::SessionStart,
                EventName::ActivityStateChanged,
                EventName::SessionHeartbeat,
            ]
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn abnormal_exit_reports_error_and_session_end() {
        let captured_usage = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let captured_crash = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let client = RuntimeTelemetryClient::new_with_sinks(
            telemetry_test_state(),
            Box::new(CollectingSink {
                events: captured_usage.clone(),
            }),
            Box::new(CollectingSink {
                events: captured_crash.clone(),
            }),
        );

        let exit_code = report_abnormal_exit(&client, SessionEndReason::StartupFailure, "startup");

        assert_eq!(exit_code, std::process::ExitCode::from(1));

        let usage_events = captured_usage.lock().expect("usage events lock");
        let crash_events = captured_crash.lock().expect("crash events lock");

        assert!(crash_events.iter().any(|event| {
            event.event_name == EventName::AppError
                && event.properties["category"] == serde_json::json!("startup")
                && event.properties["recoverable"] == serde_json::json!(false)
        }));
        assert!(usage_events.iter().any(|event| {
            event.event_name == EventName::SessionEnd
                && event.properties["reason"] == serde_json::json!("startup_failure")
                && event.properties["clean_exit"] == serde_json::json!(false)
        }));
    }

    #[test]
    #[serial]
    fn breathing_pattern_window_telemetry_emits_open_and_cancel_actions() {
        let root = telemetry_test_dir("breathing-pattern-window");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let captured_events = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let telemetry = RuntimeTelemetryClient::new_with_sinks(
            telemetry_test_state(),
            Box::new(CollectingSink {
                events: captured_events.clone(),
            }),
            Box::new(NoopSink),
        );

        let app = App {
            telemetry,
            ..App::default()
        };
        app.telemetry_breathing_pattern_window("add_new_opened");
        app.telemetry_breathing_pattern_window("add_new_canceled");
        app.telemetry.flush(Duration::from_millis(200));
        app.telemetry.shutdown(Duration::from_millis(200));

        let actions = captured_events
            .lock()
            .expect("captured events lock")
            .iter()
            .filter(|event| event.event_name == EventName::BreathingPatternChanged)
            .map(|event| {
                event.properties["action"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>();

        assert!(actions.contains(&"add_new_opened".to_string()));
        assert!(actions.contains(&"add_new_canceled".to_string()));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[serial]
    fn follow_cursor_telemetry_emits_toggle_action_and_state() {
        let root = telemetry_test_dir("follow-cursor");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let captured_events = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let telemetry = RuntimeTelemetryClient::new_with_sinks(
            telemetry_test_state(),
            Box::new(CollectingSink {
                events: captured_events.clone(),
            }),
            Box::new(NoopSink),
        );

        let app = App {
            telemetry,
            ..App::default()
        };
        app.telemetry_follow_cursor_change(true);
        app.telemetry_follow_cursor_change(false);
        app.telemetry.flush(Duration::from_millis(200));
        app.telemetry.shutdown(Duration::from_millis(200));

        let states = captured_events
            .lock()
            .expect("captured events lock")
            .iter()
            .filter(|event| event.event_name == EventName::MenuAction)
            .filter(|event| event.properties["action"] == serde_json::json!("follow_cursor"))
            .map(|event| event.properties["enabled"].as_bool())
            .collect::<Vec<_>>();

        assert_eq!(states, vec![Some(true), Some(false)]);

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn newer_version_detection_handles_v_prefix() {
        assert!(is_newer_version("v0.2.0", "0.1.5"));
        assert!(!is_newer_version("0.1.5", "0.1.5"));
        assert!(!is_newer_version("0.1.4", "0.1.5"));
    }

    #[test]
    fn update_badge_visibility_respects_daily_snooze() {
        let state = UpdateUiState {
            latest_version: Some("9.9.9".to_string()),
            badge_snoozed_version: Some("9.9.9".to_string()),
            badge_snoozed_at_epoch_seconds: Some(10_000),
            ..UpdateUiState::default()
        };

        assert!(!state.should_show_badge_at(10_000 + UPDATE_BADGE_REMINDER_INTERVAL_SEC - 1));
        assert!(state.should_show_badge_at(10_000 + UPDATE_BADGE_REMINDER_INTERVAL_SEC));
    }

    #[test]
    fn update_badge_visibility_is_version_scoped() {
        let state = UpdateUiState {
            latest_version: Some("9.9.10".to_string()),
            badge_snoozed_version: Some("9.9.9".to_string()),
            badge_snoozed_at_epoch_seconds: Some(10_000),
            ..UpdateUiState::default()
        };

        assert!(state.should_show_badge_at(10_001));
    }

    #[test]
    fn update_badge_visibility_respects_ignored_current_version() {
        let state = UpdateUiState {
            latest_version: Some("9.9.9".to_string()),
            ignored_version: Some("9.9.9".to_string()),
            ..UpdateUiState::default()
        };

        assert!(!state.should_show_badge_at(10_000));
        assert!(state.is_ignoring_current_update());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn developer_update_control_only_changes_future_check_policy() {
        let app = App::default();
        assert_eq!(app.widget_window_dimensions(320.0).height, 352.0);
        app.update_check.set_simulate_pending_update(true);

        assert!(app.update_check.simulate_pending_update());
        assert!(!app.updates.has_update_available());
        assert!(!app.updates.should_show_badge());
        assert_eq!(app.widget_window_dimensions(320.0).height, 352.0);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn simulated_update_result_flows_through_normal_update_state() {
        let mut app = App::default();
        app.settings.cached_latest_update_version = Some("0.1.0".to_string());

        app.apply_update_check_result(UpdateCheckResult {
            latest_version: Some("99.99.99".to_string()),
            download_url: UPDATE_DOWNLOAD_FALLBACK_URL.to_string(),
            simulated: true,
        });

        assert_eq!(app.updates.latest_version.as_deref(), Some("99.99.99"));
        assert!(app.updates.has_update_available());
        assert!(app.updates.should_show_badge());
        assert_eq!(
            app.settings.cached_latest_update_version.as_deref(),
            Some("0.1.0")
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn clear_update_notification_dismissed_only_clears_badge_snooze() {
        let mut app = App::default();
        app.settings.update_badge_snoozed_version = Some("9.9.9".to_string());
        app.settings.update_badge_snoozed_at_epoch_seconds = Some(10_000);
        app.settings.ignored_update_version = Some("9.9.9".to_string());
        app.updates.badge_snoozed_version = app.settings.update_badge_snoozed_version.clone();
        app.updates.badge_snoozed_at_epoch_seconds =
            app.settings.update_badge_snoozed_at_epoch_seconds;
        app.updates.ignored_version = app.settings.ignored_update_version.clone();

        app.clear_update_notification_dismissed();

        assert!(app.settings.update_badge_snoozed_version.is_none());
        assert!(app.settings.update_badge_snoozed_at_epoch_seconds.is_none());
        assert!(app.updates.badge_snoozed_version.is_none());
        assert!(app.updates.badge_snoozed_at_epoch_seconds.is_none());
        assert_eq!(
            app.settings.ignored_update_version.as_deref(),
            Some("9.9.9")
        );
    }

    #[test]
    fn update_check_result_clears_matching_ignore_and_snooze_after_upgrade() {
        let current = env!("CARGO_PKG_VERSION").to_string();
        let mut app = App::default();
        app.settings.cached_latest_update_version = Some(current.clone());
        app.settings.update_badge_snoozed_version = Some(current.clone());
        app.settings.update_badge_snoozed_at_epoch_seconds = Some(10_000);
        app.settings.ignored_update_version = Some(current.clone());

        app.apply_update_check_result(UpdateCheckResult {
            latest_version: Some(current),
            download_url: UPDATE_DOWNLOAD_FALLBACK_URL.to_string(),
            #[cfg(debug_assertions)]
            simulated: false,
        });

        assert!(app.settings.update_badge_snoozed_version.is_none());
        assert!(app.settings.update_badge_snoozed_at_epoch_seconds.is_none());
        assert!(app.settings.ignored_update_version.is_none());
        assert!(!app.updates.should_show_badge_at(10_001));
    }

    #[test]
    fn size_target_label_matches_menu_slots() {
        assert_eq!(size_target_label(0), Some("S"));
        assert_eq!(size_target_label(1), Some("M"));
        assert_eq!(size_target_label(2), Some("L"));
        assert_eq!(size_target_label(3), Some("XL"));
        assert_eq!(size_target_label(4), None);
    }

    #[test]
    fn instance_message_round_trips() {
        assert!(instance_message_is_activate("activate\n"));
        assert_eq!(INSTANCE_ACTIVATE_MESSAGE.as_bytes(), b"activate\n");
        assert!(!instance_message_is_activate("nope"));
    }

    #[cfg(unix)]
    #[test]
    fn instance_socket_path_is_scoped_to_executable_path() {
        let debug_path = instance_socket_path_for_executable(Path::new(
            "/Users/m1/src/downshift/target/debug/downshift",
        ))
        .expect("debug executable path should resolve a socket path");
        let app_path = instance_socket_path_for_executable(Path::new(
            "/Applications/Downshift.app/Contents/MacOS/downshift",
        ))
        .expect("app executable path should resolve a socket path");

        assert_ne!(debug_path, app_path);
        assert_eq!(debug_path.parent(), app_path.parent());
        assert!(debug_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("instance-") && name.ends_with(".sock")));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_instance_pipe_name_is_scoped_to_executable_path() {
        let pipe_name =
            windows_instance_pipe_name().expect("current executable path should resolve");

        assert!(pipe_name.starts_with(r"\\.\pipe\downshift-"));
        assert!(pipe_name.len() > r"\\.\pipe\downshift-".len());
    }

    #[test]
    fn inline_ui_assets_replaces_style_and_script_placeholders() {
        let html = inline_ui_assets(
            "<style>__DOWNSHIFT_INLINE_STYLE__</style><script>__DOWNSHIFT_INLINE_SCRIPT__</script>",
            "\nbody { color: red; }\n",
            "\nconsole.log('ok');\n",
        );

        assert_eq!(
            html,
            "<style>body { color: red; }</style><script>console.log('ok');</script>"
        );
    }

    #[test]
    fn breath_html_embeds_polygon_animation_before_breathing_consumer() {
        let html = breath_html();
        let polygon_module = html
            .find("window.downshiftPolygonAnimation")
            .expect("shared polygon module should be embedded");
        let breathing_consumer = html
            .find("terminalHitTargetSizePx")
            .expect("breathing consumer should be embedded");

        assert!(polygon_module < breathing_consumer);
    }

    #[test]
    fn animation_window_dimensions_use_reported_bounds() {
        let app = App::default();
        let dimensions = app.animation_window_dimensions_for(
            320.0,
            AnimationBounds {
                x: 25.0,
                y: 30.0,
                width: 50.0,
                height: 25.0,
                badge_visible: false,
            },
        );

        assert_eq!(dimensions.width, 164.0);
        assert_eq!(dimensions.height, 84.0);

        let badge_dimensions = app.animation_window_dimensions_for(
            320.0,
            AnimationBounds {
                x: 25.0,
                y: 30.0,
                width: 50.0,
                height: 25.0,
                badge_visible: true,
            },
        );
        assert_eq!(badge_dimensions.height, 116.0);
    }

    #[test]
    fn animation_shape_bottom_center_includes_hit_padding() {
        let app = App::default();
        let center = app.animation_shape_bottom_center(
            320.0,
            AnimationBounds {
                x: 25.0,
                y: 30.0,
                width: 50.0,
                height: 25.0,
                badge_visible: false,
            },
        );

        assert_eq!(center, LogicalPosition::new(82.0, 82.0));
    }

    #[test]
    fn follow_cursor_uses_rem_sized_artwork() {
        let app = App {
            animation_bounds: AnimationBounds {
                x: 3.0,
                y: 4.0,
                width: 94.0,
                height: 92.0,
                badge_visible: true,
            },
            follow_cursor_active: true,
            ..App::default()
        };

        let artwork_size = app.artwork_size_for_window();
        let dimensions = app.animation_window_dimensions_for(artwork_size, app.animation_bounds);

        assert_eq!(artwork_size, FOLLOW_CURSOR_ARTWORK_SIZE_LOGICAL);
        assert_eq!(dimensions.width, FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL);
        assert_eq!(dimensions.height, FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL);
        assert!(dimensions.height > artwork_size);
    }

    #[test]
    fn follow_cursor_position_does_not_persist_manual_position() {
        let mut app = App {
            follow_cursor_active: true,
            ..App::default()
        };
        app.settings.physical_x = Some(120);
        app.settings.physical_y = Some(240);

        app.update_position_from_physical(PhysicalPosition::new(900, 700));

        assert_eq!(app.settings.physical_x, Some(120));
        assert_eq!(app.settings.physical_y, Some(240));
    }

    #[test]
    fn follow_cursor_blocks_manual_drag_ipc() {
        let mut app = App {
            follow_cursor_active: true,
            ..App::default()
        };

        app.start_manual_drag(40, 60);

        assert!(app.drag_anchor_window_pos.is_none());
        assert!(app.drag_anchor_pointer_pos.is_none());
    }

    #[test]
    fn follow_cursor_source_can_report_unsupported_platforms() {
        struct FakeCursorSource {
            sample: Result<CursorPosition, CursorError>,
            supported: bool,
            reason: &'static str,
        }

        impl CursorSource for FakeCursorSource {
            fn sample(&mut self) -> Result<CursorPosition, CursorError> {
                self.sample.clone()
            }

            fn is_supported(&self) -> bool {
                self.supported
            }

            fn unavailable_reason(&self) -> &'static str {
                self.reason
            }
        }

        let wayland = FakeCursorSource {
            sample: Err(CursorError::Unavailable(
                "cursor following is unavailable on Wayland",
            )),
            supported: false,
            reason: "cursor following is unavailable on Wayland",
        };
        assert!(!wayland.is_supported());
        assert_eq!(
            wayland.unavailable_reason(),
            "cursor following is unavailable on Wayland"
        );

        let linux = FakeCursorSource {
            sample: Err(CursorError::Unavailable(
                "cursor following is unavailable on Linux",
            )),
            supported: false,
            reason: "cursor following is unavailable on Linux",
        };
        assert!(!linux.is_supported());
        assert_eq!(
            linux.unavailable_reason(),
            "cursor following is unavailable on Linux"
        );
    }

    #[test]
    fn follow_cursor_ui_assets_include_toggle_and_upright_artwork() {
        let html = breath_html();
        assert!(html.contains("cursor-halo"));
        assert!(html.contains("menu-follow-cursor"));
        assert!(html.contains("set_follow_cursor"));
        assert!(!html.contains("100 - y"));
    }

    #[test]
    fn resume_from_snooze_restores_active_state_without_pausing() {
        let mut app = App {
            activity_mode: ActivityMode::Snoozed,
            ..App::default()
        };
        app.settings.paused = true;
        app.snooze_deadline = Some(SystemTime::now() + Duration::from_secs(60));

        assert!(app.resume_from_snooze());
        assert_eq!(app.activity_mode, ActivityMode::Active);
        assert!(!app.settings.paused);
        assert!(app.snooze_deadline.is_none());
    }

    #[test]
    fn resume_from_snooze_is_noop_when_not_snoozed() {
        let mut app = App {
            activity_mode: ActivityMode::Paused,
            ..App::default()
        };
        app.settings.paused = true;

        assert!(!app.resume_from_snooze());
        assert_eq!(app.activity_mode, ActivityMode::Paused);
        assert!(app.settings.paused);
    }

    #[test]
    fn reconcile_snooze_after_resume_expires_elapsed_snooze() {
        let mut app = App {
            activity_mode: ActivityMode::Snoozed,
            ..App::default()
        };
        app.snooze_deadline = Some(SystemTime::now() - Duration::from_secs(1));

        app.reconcile_snooze_after_resume();

        assert_eq!(app.activity_mode, ActivityMode::Active);
        assert!(app.snooze_deadline.is_none());
    }

    #[test]
    fn reconcile_snooze_after_resume_keeps_pending_snooze() {
        let mut app = App {
            activity_mode: ActivityMode::Snoozed,
            ..App::default()
        };
        app.snooze_deadline = Some(SystemTime::now() + Duration::from_secs(60));

        app.reconcile_snooze_after_resume();

        assert_eq!(app.activity_mode, ActivityMode::Snoozed);
        assert!(app.snooze_deadline.is_some());
    }

    #[test]
    #[serial]
    fn save_settings_backs_up_corrupt_file_before_overwrite() {
        let root = telemetry_test_dir("settings-backup");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let config_dir = root.join("config");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        let settings_path = config_dir.join("settings.toml");
        std::fs::write(&settings_path, "this is not toml").expect("write corrupt settings");

        let mut app = App {
            config_path: Some(settings_path.clone()),
            settings: Settings::default(),
            ..App::default()
        };
        app.settings.size = 144.0;
        app.settings_load_error = Some("failed to parse settings".to_string());
        app.settings_backup_pending = true;

        app.save_settings();

        let backup_path = App::settings_backup_path(&settings_path);
        let backup = std::fs::read_to_string(&backup_path).expect("read settings backup");
        let saved = std::fs::read_to_string(&settings_path).expect("read saved settings");

        assert_eq!(backup, "this is not toml");
        assert!(saved.contains("size = 144.0"));
        assert!(!app.settings_backup_pending);
        assert!(app.settings_load_error.is_none());

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn snooze_menu_id_maps_to_expected_minutes() {
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_5), Some(5));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_10), Some(10));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_15), Some(15));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_30), Some(30));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_60), Some(60));
        assert_eq!(snooze_minutes_for_menu_id("nope"), None);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn size_menu_id_maps_to_expected_slots() {
        assert_eq!(size_slot_for_menu_id(MENU_ID_SIZE_S), Some(0));
        assert_eq!(size_slot_for_menu_id(MENU_ID_SIZE_M), Some(1));
        assert_eq!(size_slot_for_menu_id(MENU_ID_SIZE_L), Some(2));
        assert_eq!(size_slot_for_menu_id(MENU_ID_SIZE_XL), Some(3));
        assert_eq!(size_slot_for_menu_id("nope"), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn write_and_remove_launch_agent_round_trips() {
        let root = telemetry_test_dir("launch-agent");
        let path = root.join("Library/LaunchAgents/com.samm81.downshift.plist");
        let executable = Path::new("/Applications/Downshift.app/Contents/MacOS/downshift");

        write_launch_agent(&path, executable).expect("write launch agent");
        let content = std::fs::read_to_string(&path).expect("read launch agent");
        assert!(content.contains("<string>com.samm81.downshift</string>"));
        assert!(content.contains(executable.to_str().expect("utf8 executable path")));

        remove_launch_agent(&path).expect("remove launch agent");
        assert!(!path.exists());
        std::fs::remove_dir_all(root).ok();
    }
}
