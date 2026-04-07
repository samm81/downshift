use serde::{Deserialize, Serialize};
use std::path::Path;
pub mod diagnostics;
pub mod telemetry;

pub const DEFAULT_SIZE: f64 = 96.0;
pub const MIN_SIZE: f64 = 8.0;
pub const MAX_SIZE: f64 = 320.0;
pub const WHEEL_STEP: f64 = 4.0;
pub const WHEEL_FINE_STEP: f64 = 1.0;
pub const DEFAULT_HALF_CYCLE_SECONDS: f64 = 5.5;
pub const FAST_HALF_CYCLE_SECONDS: f64 = 4.5;
pub const SLOW_HALF_CYCLE_SECONDS: f64 = 6.5;
pub const MIN_ACTIVE_PHASE_SECONDS: f64 = 0.5;
pub const MAX_PHASE_SECONDS: f64 = 60.0;
pub const DEFAULT_MARGIN: f64 = 24.0;
pub const LAUNCH_AGENT_LABEL: &str = "com.samm81.downshift";
pub const LAUNCH_AGENT_FILENAME: &str = "com.samm81.downshift.plist";
pub const BREATHING_PRESET_ID_COHERENT: &str = "coherent_breathing";
pub const BREATHING_PRESET_ID_BOX: &str = "box_breathing";
pub const BREATHING_PRESET_ID_479: &str = "4_7_9";
pub const BREATHING_PRESET_ID_CUSTOM: &str = "custom";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PersistedMonitor {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BreathingPattern {
    pub expanding_seconds: f64,
    #[serde(default)]
    pub expanded_hold_seconds: f64,
    pub compressing_seconds: f64,
    #[serde(default)]
    pub compressed_hold_seconds: f64,
}

impl BreathingPattern {
    pub fn coherent() -> Self {
        Self {
            expanding_seconds: DEFAULT_HALF_CYCLE_SECONDS,
            expanded_hold_seconds: 0.0,
            compressing_seconds: DEFAULT_HALF_CYCLE_SECONDS,
            compressed_hold_seconds: 0.0,
        }
    }

    pub fn box_breathing() -> Self {
        Self {
            expanding_seconds: 4.0,
            expanded_hold_seconds: 4.0,
            compressing_seconds: 4.0,
            compressed_hold_seconds: 4.0,
        }
    }

    pub fn four_seven_nine() -> Self {
        Self {
            expanding_seconds: 4.0,
            expanded_hold_seconds: 7.0,
            compressing_seconds: 9.0,
            compressed_hold_seconds: 0.0,
        }
    }

    pub fn sanitize(&mut self) {
        let default = Self::coherent();
        self.expanding_seconds =
            normalize_pattern_phase(self.expanding_seconds, default.expanding_seconds, false);
        self.expanded_hold_seconds = normalize_pattern_phase(
            self.expanded_hold_seconds,
            default.expanded_hold_seconds,
            true,
        );
        self.compressing_seconds =
            normalize_pattern_phase(self.compressing_seconds, default.compressing_seconds, false);
        self.compressed_hold_seconds = normalize_pattern_phase(
            self.compressed_hold_seconds,
            default.compressed_hold_seconds,
            true,
        );
    }
}

impl Default for BreathingPattern {
    fn default() -> Self {
        Self::coherent()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedBreathingPreset {
    pub id: String,
    pub name: String,
    pub pattern: BreathingPattern,
}

impl SavedBreathingPreset {
    pub fn sanitize(&mut self) -> bool {
        self.id = self.id.trim().to_string();
        self.name = self.name.trim().to_string();
        self.pattern.sanitize();
        !(self.id.is_empty() || self.name.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltInBreathingPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub pattern: BreathingPattern,
}

pub fn built_in_breathing_presets() -> [BuiltInBreathingPreset; 3] {
    [
        BuiltInBreathingPreset {
            id: BREATHING_PRESET_ID_COHERENT,
            name: "coherent breathing",
            pattern: BreathingPattern::coherent(),
        },
        BuiltInBreathingPreset {
            id: BREATHING_PRESET_ID_BOX,
            name: "box breathing",
            pattern: BreathingPattern::box_breathing(),
        },
        BuiltInBreathingPreset {
            id: BREATHING_PRESET_ID_479,
            name: "4-7-9",
            pattern: BreathingPattern::four_seven_nine(),
        },
    ]
}

pub fn built_in_breathing_preset(id: &str) -> Option<BuiltInBreathingPreset> {
    built_in_breathing_presets()
        .into_iter()
        .find(|preset| preset.id == id)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub size: f64,
    #[serde(default)]
    pub breathing_pattern: BreathingPattern,
    #[serde(default = "default_active_breathing_preset_id")]
    pub active_breathing_preset_id: String,
    #[serde(default)]
    pub saved_breathing_presets: Vec<SavedBreathingPreset>,
    #[serde(default)]
    pub hidden_breathing_preset_ids: Vec<String>,
    #[serde(rename = "half_cycle_seconds", default, skip_serializing)]
    legacy_half_cycle_seconds: Option<f64>,
    pub paused: bool,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default = "default_true")]
    pub usage_data_sharing: bool,
    #[serde(default = "default_true")]
    pub crash_reports_sharing: bool,
    #[serde(default)]
    pub update_badge_snoozed_version: Option<String>,
    #[serde(default)]
    pub update_badge_snoozed_at_epoch_seconds: Option<i64>,
    #[serde(default)]
    pub ignored_update_version: Option<String>,
    #[serde(rename = "dismissed_update_version", default, skip_serializing)]
    legacy_dismissed_update_version: Option<String>,
    #[serde(default)]
    pub cached_latest_update_version: Option<String>,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub monitor: Option<PersistedMonitor>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadSettingsResult {
    pub settings: Settings,
    pub load_error: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            size: DEFAULT_SIZE,
            breathing_pattern: BreathingPattern::default(),
            active_breathing_preset_id: default_active_breathing_preset_id(),
            saved_breathing_presets: Vec::new(),
            hidden_breathing_preset_ids: Vec::new(),
            legacy_half_cycle_seconds: None,
            paused: false,
            launch_at_login: true,
            usage_data_sharing: true,
            crash_reports_sharing: true,
            update_badge_snoozed_version: None,
            update_badge_snoozed_at_epoch_seconds: None,
            ignored_update_version: None,
            legacy_dismissed_update_version: None,
            cached_latest_update_version: None,
            x: None,
            y: None,
            monitor: None,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_active_breathing_preset_id() -> String {
    BREATHING_PRESET_ID_COHERENT.to_string()
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.size = self.size.clamp(MIN_SIZE, MAX_SIZE);
        self.breathing_pattern.sanitize();
        sanitize_optional_string(&mut self.update_badge_snoozed_version);
        sanitize_optional_string(&mut self.ignored_update_version);
        sanitize_optional_string(&mut self.legacy_dismissed_update_version);
        if self
            .update_badge_snoozed_at_epoch_seconds
            .is_some_and(|value| value < 0)
        {
            self.update_badge_snoozed_at_epoch_seconds = None;
        }
        if self.ignored_update_version.is_none() {
            self.ignored_update_version = self.legacy_dismissed_update_version.clone();
        }
        self.legacy_dismissed_update_version = None;

        if let Some(legacy_half_cycle_seconds) = self.legacy_half_cycle_seconds.take() {
            self.breathing_pattern = legacy_pattern_from_half_cycle(legacy_half_cycle_seconds);
        }

        self.saved_breathing_presets
            .retain_mut(SavedBreathingPreset::sanitize);
        dedupe_saved_preset_ids(&mut self.saved_breathing_presets);
        self.hidden_breathing_preset_ids = self
            .hidden_breathing_preset_ids
            .iter()
            .map(|id| id.trim().to_string())
            .filter(|id| built_in_breathing_preset(id).is_some())
            .collect();
        self.hidden_breathing_preset_ids.sort();
        self.hidden_breathing_preset_ids.dedup();

        let active_id = self.active_breathing_preset_id.trim().to_string();
        let should_preserve_custom_pattern = active_id == BREATHING_PRESET_ID_CUSTOM
            || (active_id == BREATHING_PRESET_ID_COHERENT
                && !patterns_match(&self.breathing_pattern, &BreathingPattern::coherent()));

        self.active_breathing_preset_id = if should_preserve_custom_pattern {
            if let Some(matching_id) = self.matching_preset_id_for_pattern() {
                matching_id
            } else {
                BREATHING_PRESET_ID_CUSTOM.to_string()
            }
        } else if active_id == BREATHING_PRESET_ID_CUSTOM
            || (built_in_breathing_preset(&active_id).is_some()
                && !self.hidden_breathing_preset_ids.contains(&active_id))
            || self
                .saved_breathing_presets
                .iter()
                .any(|preset| preset.id == active_id)
        {
            active_id
        } else {
            self.matching_preset_id_for_pattern()
                .unwrap_or_else(|| BREATHING_PRESET_ID_CUSTOM.to_string())
        };

        if let Some(pattern) = self.active_pattern_from_presets() {
            self.breathing_pattern = pattern;
        }
    }

    pub fn active_pattern_from_presets(&self) -> Option<BreathingPattern> {
        if let Some(preset) = built_in_breathing_preset(&self.active_breathing_preset_id) {
            if self
                .hidden_breathing_preset_ids
                .contains(&self.active_breathing_preset_id)
            {
                return None;
            }
            return Some(preset.pattern);
        }
        self.saved_breathing_presets
            .iter()
            .find(|preset| preset.id == self.active_breathing_preset_id)
            .map(|preset| preset.pattern.clone())
    }

    fn matching_preset_id_for_pattern(&self) -> Option<String> {
        if let Some(preset) = built_in_breathing_presets()
            .into_iter()
            .filter(|preset| {
                !self
                    .hidden_breathing_preset_ids
                    .iter()
                    .any(|id| id == preset.id)
            })
            .find(|preset| patterns_match(&self.breathing_pattern, &preset.pattern))
        {
            return Some(preset.id.to_string());
        }
        self.saved_breathing_presets
            .iter()
            .find(|preset| patterns_match(&self.breathing_pattern, &preset.pattern))
            .map(|preset| preset.id.clone())
    }
}

fn sanitize_optional_string(value: &mut Option<String>) {
    if let Some(current) = value.as_mut() {
        *current = current.trim().to_string();
        if current.is_empty() {
            *value = None;
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcCommand {
    Quit,
    SetPaused {
        paused: bool,
    },
    SetSnooze {
        minutes: u64,
    },
    ShowBreathingPattern,
    CloseBreathingPattern,
    ApplyBreathingPattern {
        preset_id: String,
        pattern: BreathingPattern,
    },
    SaveBreathingPreset {
        name: String,
        pattern: BreathingPattern,
    },
    DeleteBreathingPreset {
        preset_id: String,
    },
    SetUsageDataSharing {
        enabled: bool,
    },
    SetCrashReportsSharing {
        enabled: bool,
    },
    AnalyticsMenuOpened,
    ShowTelemetryInfo,
    CloseTelemetryInfo,
    ShowCustomSnooze,
    CloseCustomSnooze,
    UpdatePrimaryAction,
    DismissUpdateBadge,
    SetIgnoreCurrentUpdate {
        ignored: bool,
    },
    CloseUpdateDialog,
    DownloadUpdate,
    ShowContextMenu {
        x: i32,
        y: i32,
    },
    Resize {
        delta: i32,
        fine: bool,
    },
    SetSize {
        size: f64,
    },
    StartDrag {
        screen_x: i32,
        screen_y: i32,
    },
    DragTo {
        screen_x: i32,
        screen_y: i32,
    },
    EndDrag,
    Reset,
}

pub fn legacy_pattern_from_half_cycle(value: f64) -> BreathingPattern {
    let half_cycle_seconds = normalize_half_cycle(value);
    BreathingPattern {
        expanding_seconds: half_cycle_seconds,
        expanded_hold_seconds: 0.0,
        compressing_seconds: half_cycle_seconds,
        compressed_hold_seconds: 0.0,
    }
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

fn normalize_pattern_phase(value: f64, default: f64, allow_zero: bool) -> f64 {
    if !value.is_finite() {
        return default;
    }
    let minimum = if allow_zero {
        0.0
    } else {
        MIN_ACTIVE_PHASE_SECONDS
    };
    if value < minimum {
        return default;
    }
    value.clamp(minimum, MAX_PHASE_SECONDS)
}

fn dedupe_saved_preset_ids(presets: &mut Vec<SavedBreathingPreset>) {
    let mut seen = std::collections::HashSet::new();
    presets.retain(|preset| seen.insert(preset.id.clone()));
}

fn patterns_match(left: &BreathingPattern, right: &BreathingPattern) -> bool {
    (left.expanding_seconds - right.expanding_seconds).abs() <= 0.001
        && (left.expanded_hold_seconds - right.expanded_hold_seconds).abs() <= 0.001
        && (left.compressing_seconds - right.compressing_seconds).abs() <= 0.001
        && (left.compressed_hold_seconds - right.compressed_hold_seconds).abs() <= 0.001
}

pub fn launch_agent_path_from_home(home: &Path) -> std::path::PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(LAUNCH_AGENT_FILENAME)
}

pub fn launch_agent_plist(executable: &Path) -> String {
    let executable = executable.display().to_string();
    let working_directory = executable
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(|| "/".to_string());

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{executable}</string>
  </array>
  <key>ProcessType</key>
  <string>Interactive</string>
  <key>RunAtLoad</key>
  <true/>
  <key>WorkingDirectory</key>
  <string>{working_directory}</string>
</dict>
</plist>
"#,
        label = LAUNCH_AGENT_LABEL,
        executable = xml_escape(&executable),
        working_directory = xml_escape(&working_directory),
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn load_settings_result(path: Option<&Path>) -> LoadSettingsResult {
    let (mut settings, load_error) = match path {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(raw) => match toml::from_str::<Settings>(&raw) {
                Ok(settings) => (settings, None),
                Err(error) => (
                    Settings::default(),
                    Some(format!(
                        "failed to parse settings {}: {error}",
                        path.display()
                    )),
                ),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (Settings::default(), None)
            }
            Err(error) => (
                Settings::default(),
                Some(format!(
                    "failed to read settings {}: {error}",
                    path.display()
                )),
            ),
        },
        None => (Settings::default(), None),
    };
    settings.sanitize();
    LoadSettingsResult {
        settings,
        load_error,
    }
}

pub fn load_settings(path: Option<&Path>) -> Settings {
    load_settings_result(path).settings
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
    fn breathing_pattern_sanitize_normalizes_invalid_phases() {
        let mut pattern = BreathingPattern {
            expanding_seconds: f64::NAN,
            expanded_hold_seconds: -1.0,
            compressing_seconds: 999.0,
            compressed_hold_seconds: 90.0,
        };

        pattern.sanitize();

        assert_eq!(pattern.expanding_seconds, DEFAULT_HALF_CYCLE_SECONDS);
        assert_eq!(pattern.expanded_hold_seconds, 0.0);
        assert_eq!(pattern.compressing_seconds, MAX_PHASE_SECONDS);
        assert_eq!(pattern.compressed_hold_seconds, MAX_PHASE_SECONDS);
    }

    #[test]
    fn legacy_pattern_from_half_cycle_maps_to_symmetric_pattern() {
        assert_eq!(
            legacy_pattern_from_half_cycle(4.54),
            BreathingPattern {
                expanding_seconds: FAST_HALF_CYCLE_SECONDS,
                expanded_hold_seconds: 0.0,
                compressing_seconds: FAST_HALF_CYCLE_SECONDS,
                compressed_hold_seconds: 0.0,
            }
        );
    }

    #[test]
    fn settings_sanitize_clamps_size_and_normalizes_pattern_state() {
        let mut settings = Settings {
            size: 999.0,
            breathing_pattern: BreathingPattern {
                expanding_seconds: 3.0,
                expanded_hold_seconds: -4.0,
                compressing_seconds: 6.47,
                compressed_hold_seconds: 1.0,
            },
            active_breathing_preset_id: "missing".to_string(),
            saved_breathing_presets: vec![
                SavedBreathingPreset {
                    id: " focus ".to_string(),
                    name: " focus ".to_string(),
                    pattern: BreathingPattern::four_seven_nine(),
                },
                SavedBreathingPreset {
                    id: "focus".to_string(),
                    name: "duplicate".to_string(),
                    pattern: BreathingPattern::box_breathing(),
                },
                SavedBreathingPreset {
                    id: "  ".to_string(),
                    name: "ignored".to_string(),
                    pattern: BreathingPattern::box_breathing(),
                },
            ],
            hidden_breathing_preset_ids: vec![
                " box_breathing ".to_string(),
                "box_breathing".to_string(),
                "missing".to_string(),
            ],
            legacy_half_cycle_seconds: None,
            paused: true,
            launch_at_login: true,
            usage_data_sharing: false,
            crash_reports_sharing: true,
            update_badge_snoozed_version: Some(" 0.1.2 ".to_string()),
            update_badge_snoozed_at_epoch_seconds: Some(-1),
            ignored_update_version: None,
            legacy_dismissed_update_version: Some(" 0.1.4 ".to_string()),
            cached_latest_update_version: Some("0.1.5".to_string()),
            x: Some(10),
            y: Some(20),
            monitor: None,
        };

        settings.sanitize();

        assert_eq!(settings.size, MAX_SIZE);
        assert_eq!(
            settings.breathing_pattern,
            BreathingPattern {
                expanding_seconds: 3.0,
                expanded_hold_seconds: 0.0,
                compressing_seconds: 6.47,
                compressed_hold_seconds: 1.0,
            }
        );
        assert_eq!(
            settings.active_breathing_preset_id,
            BREATHING_PRESET_ID_CUSTOM
        );
        assert_eq!(settings.saved_breathing_presets.len(), 1);
        assert_eq!(settings.saved_breathing_presets[0].id, "focus");
        assert_eq!(settings.saved_breathing_presets[0].name, "focus");
        assert_eq!(
            settings.hidden_breathing_preset_ids,
            vec!["box_breathing".to_string()]
        );
        assert!(settings.paused);
        assert!(settings.launch_at_login);
        assert!(!settings.usage_data_sharing);
        assert!(settings.crash_reports_sharing);
        assert_eq!(
            settings.update_badge_snoozed_version.as_deref(),
            Some("0.1.2")
        );
        assert!(settings.update_badge_snoozed_at_epoch_seconds.is_none());
        assert_eq!(settings.ignored_update_version.as_deref(), Some("0.1.4"));
        assert_eq!(
            settings.cached_latest_update_version.as_deref(),
            Some("0.1.5")
        );
        assert_eq!(settings.x, Some(10));
        assert_eq!(settings.y, Some(20));
    }

    #[test]
    fn sanitize_preserves_explicit_ignored_update_version_over_legacy_value() {
        let mut settings = Settings {
            ignored_update_version: Some("0.2.0".to_string()),
            legacy_dismissed_update_version: Some("0.1.9".to_string()),
            ..Settings::default()
        };

        settings.sanitize();

        assert_eq!(settings.ignored_update_version.as_deref(), Some("0.2.0"));
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
    fn launch_agent_path_uses_standard_launch_agents_directory() {
        let home = Path::new("/Users/tester");
        assert_eq!(
            launch_agent_path_from_home(home),
            home.join("Library/LaunchAgents/com.samm81.downshift.plist")
        );
    }

    #[test]
    fn launch_agent_plist_escapes_xml_sensitive_characters() {
        let plist = launch_agent_plist(Path::new(
            "/Applications/Down&shift <alpha>.app/Contents/MacOS/Down\"shift'",
        ));

        assert!(plist.contains("<string>com.samm81.downshift</string>"));
        assert!(plist.contains(
            "/Applications/Down&amp;shift &lt;alpha&gt;.app/Contents/MacOS/Down&quot;shift&apos;"
        ));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<string>Interactive</string>"));
    }

    #[test]
    fn ipc_command_serde_uses_snake_case_tagged_format() {
        let raw = r#"{"cmd":"apply_breathing_pattern","preset_id":"custom","pattern":{"expanding_seconds":4.0,"expanded_hold_seconds":1.0,"compressing_seconds":6.0,"compressed_hold_seconds":0.0}}"#;
        let command: IpcCommand =
            serde_json::from_str(raw).expect("valid apply_breathing_pattern command");
        assert_eq!(
            command,
            IpcCommand::ApplyBreathingPattern {
                preset_id: "custom".to_string(),
                pattern: BreathingPattern {
                    expanding_seconds: 4.0,
                    expanded_hold_seconds: 1.0,
                    compressing_seconds: 6.0,
                    compressed_hold_seconds: 0.0,
                }
            }
        );

        let encoded = serde_json::to_string(&IpcCommand::SetPaused { paused: true })
            .expect("serialize set_paused command");
        assert!(encoded.contains("\"cmd\":\"set_paused\""));
        assert!(encoded.contains("\"paused\":true"));

        let snooze = serde_json::to_string(&IpcCommand::SetSnooze { minutes: 15 })
            .expect("serialize set_snooze command");
        assert!(snooze.contains("\"cmd\":\"set_snooze\""));
        assert!(snooze.contains("\"minutes\":15"));

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

        let delete_preset: IpcCommand =
            serde_json::from_str(r#"{"cmd":"delete_breathing_preset","preset_id":"focus"}"#)
                .expect("valid delete_breathing_preset command");
        assert_eq!(
            delete_preset,
            IpcCommand::DeleteBreathingPreset {
                preset_id: "focus".to_string()
            }
        );
    }
}
