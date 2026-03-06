use serde::{Deserialize, Serialize};
use std::path::Path;
pub mod telemetry;

pub const DEFAULT_SIZE: f64 = 96.0;
pub const MIN_SIZE: f64 = 8.0;
pub const MAX_SIZE: f64 = 320.0;
pub const WHEEL_STEP: f64 = 4.0;
pub const WHEEL_FINE_STEP: f64 = 1.0;
pub const DEFAULT_HALF_CYCLE_SECONDS: f64 = 5.5;
pub const FAST_HALF_CYCLE_SECONDS: f64 = 4.5;
pub const SLOW_HALF_CYCLE_SECONDS: f64 = 6.5;
pub const DEFAULT_MARGIN: f64 = 24.0;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PersistedMonitor {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub size: f64,
    pub half_cycle_seconds: f64,
    pub paused: bool,
    #[serde(default = "default_usage_data_sharing")]
    pub usage_data_sharing: bool,
    #[serde(default = "default_crash_reports_sharing")]
    pub crash_reports_sharing: bool,
    #[serde(default)]
    pub dismissed_update_version: Option<String>,
    #[serde(default)]
    pub cached_latest_update_version: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub monitor: Option<PersistedMonitor>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            half_cycle_seconds: DEFAULT_HALF_CYCLE_SECONDS,
            paused: false,
            usage_data_sharing: true,
            crash_reports_sharing: true,
            dismissed_update_version: None,
            cached_latest_update_version: None,
            x: None,
            y: None,
            monitor: None,
        }
    }
}

fn default_usage_data_sharing() -> bool {
    true
}

fn default_crash_reports_sharing() -> bool {
    true
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.size = self.size.clamp(MIN_SIZE, MAX_SIZE);
        self.half_cycle_seconds = normalize_half_cycle(self.half_cycle_seconds);
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Quit,
    SetPaused { paused: bool },
    SetSpeed { half_cycle_seconds: f64 },
    SetUsageDataSharing { enabled: bool },
    SetCrashReportsSharing { enabled: bool },
    AnalyticsMenuOpened,
    ShowTelemetryInfo,
    CloseTelemetryInfo,
    UpdatePrimaryAction,
    DismissUpdateBadge,
    CloseUpdateDialog,
    DownloadUpdate,
    ShowContextMenu { x: i32, y: i32 },
    Resize { delta: i32, fine: bool },
    SetSize { size: f64 },
    StartDrag { screen_x: i32, screen_y: i32 },
    DragTo { screen_x: i32, screen_y: i32 },
    EndDrag,
    Reset,
}

pub fn normalize_half_cycle(value: f64) -> f64 {
    const EPSILON: f64 = 0.05;
    if (value - FAST_HALF_CYCLE_SECONDS).abs() <= EPSILON {
        return FAST_HALF_CYCLE_SECONDS;
    }
    if (value - SLOW_HALF_CYCLE_SECONDS).abs() <= EPSILON {
        return SLOW_HALF_CYCLE_SECONDS;
    }
    DEFAULT_HALF_CYCLE_SECONDS
}

pub fn apply_resize_step(current_size: f64, delta: i32, fine: bool) -> f64 {
    let step = if fine { WHEEL_FINE_STEP } else { WHEEL_STEP };
    clamp_size(current_size + (delta as f64 * step))
}

pub fn clamp_size(size: f64) -> f64 {
    size.clamp(MIN_SIZE, MAX_SIZE)
}

pub fn load_settings(path: Option<&Path>) -> Settings {
    let mut settings = match path {
        Some(path) => std::fs::read_to_string(path)
            .ok()
            .and_then(|raw| toml::from_str::<Settings>(&raw).ok())
            .unwrap_or_default(),
        None => Settings::default(),
    };
    settings.sanitize();
    settings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_half_cycle_snaps_to_fast_near_target() {
        assert_eq!(normalize_half_cycle(4.54), FAST_HALF_CYCLE_SECONDS);
        assert_eq!(normalize_half_cycle(4.46), FAST_HALF_CYCLE_SECONDS);
    }

    #[test]
    fn normalize_half_cycle_snaps_to_slow_near_target() {
        assert_eq!(normalize_half_cycle(6.54), SLOW_HALF_CYCLE_SECONDS);
        assert_eq!(normalize_half_cycle(6.46), SLOW_HALF_CYCLE_SECONDS);
    }

    #[test]
    fn normalize_half_cycle_falls_back_to_default_for_other_values() {
        assert_eq!(normalize_half_cycle(5.0), DEFAULT_HALF_CYCLE_SECONDS);
        assert_eq!(normalize_half_cycle(7.2), DEFAULT_HALF_CYCLE_SECONDS);
        assert_eq!(normalize_half_cycle(f64::NAN), DEFAULT_HALF_CYCLE_SECONDS);
    }

    #[test]
    fn settings_sanitize_clamps_size_and_normalizes_speed() {
        let mut settings = Settings {
            size: 999.0,
            half_cycle_seconds: 6.47,
            paused: true,
            usage_data_sharing: false,
            crash_reports_sharing: true,
            dismissed_update_version: Some("0.1.2".to_string()),
            cached_latest_update_version: Some("0.1.5".to_string()),
            x: Some(10),
            y: Some(20),
            monitor: None,
        };

        settings.sanitize();

        assert_eq!(settings.size, MAX_SIZE);
        assert_eq!(settings.half_cycle_seconds, SLOW_HALF_CYCLE_SECONDS);
        assert!(settings.paused);
        assert!(!settings.usage_data_sharing);
        assert!(settings.crash_reports_sharing);
        assert_eq!(settings.dismissed_update_version.as_deref(), Some("0.1.2"));
        assert_eq!(
            settings.cached_latest_update_version.as_deref(),
            Some("0.1.5")
        );
        assert_eq!(settings.x, Some(10));
        assert_eq!(settings.y, Some(20));
    }

    #[test]
    fn apply_resize_step_respects_fine_and_coarse_step_sizes() {
        assert_eq!(apply_resize_step(32.0, 1, false), 36.0);
        assert_eq!(apply_resize_step(32.0, -1, false), 28.0);
        assert_eq!(apply_resize_step(32.0, 1, true), 33.0);
        assert_eq!(apply_resize_step(32.0, -1, true), 31.0);
    }

    #[test]
    fn apply_resize_step_clamps_to_bounds() {
        assert_eq!(apply_resize_step(MIN_SIZE, -10, false), MIN_SIZE);
        assert_eq!(apply_resize_step(MAX_SIZE, 10, true), MAX_SIZE);
    }

    #[test]
    fn ipc_command_serde_uses_snake_case_tagged_format() {
        let raw = r#"{"cmd":"set_speed","half_cycle_seconds":4.5}"#;
        let command: IpcCommand = serde_json::from_str(raw).expect("valid set_speed command");
        assert_eq!(
            command,
            IpcCommand::SetSpeed {
                half_cycle_seconds: 4.5
            }
        );

        let encoded = serde_json::to_string(&IpcCommand::SetPaused { paused: true })
            .expect("serialize set_paused command");
        assert!(encoded.contains("\"cmd\":\"set_paused\""));
        assert!(encoded.contains("\"paused\":true"));

        let show_menu: IpcCommand =
            serde_json::from_str(r#"{"cmd":"show_context_menu","x":15,"y":23}"#)
                .expect("valid show_context_menu command");
        assert_eq!(show_menu, IpcCommand::ShowContextMenu { x: 15, y: 23 });

        let drag_start: IpcCommand =
            serde_json::from_str(r#"{"cmd":"start_drag","screen_x":100,"screen_y":200}"#)
                .expect("valid start_drag command");
        assert_eq!(
            drag_start,
            IpcCommand::StartDrag {
                screen_x: 100,
                screen_y: 200
            }
        );

        let drag_to: IpcCommand =
            serde_json::from_str(r#"{"cmd":"drag_to","screen_x":120,"screen_y":230}"#)
                .expect("valid drag_to command");
        assert_eq!(
            drag_to,
            IpcCommand::DragTo {
                screen_x: 120,
                screen_y: 230
            }
        );

        let drag_end: IpcCommand =
            serde_json::from_str(r#"{"cmd":"end_drag"}"#).expect("valid end_drag command");
        assert_eq!(drag_end, IpcCommand::EndDrag);
    }
}
