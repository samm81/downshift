use downshift::telemetry::{
    menu_action_size_target, ActivityState, EventName, RuntimeTelemetryClient, SizeTarget,
    TelemetryClient,
};
use downshift::{BreathingPattern, SavedBreathingPreset, BREATHING_PRESET_ID_CUSTOM};
use semver::Version;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::update_check::{UpdateCheckResult, UpdateCheckSource};

pub(crate) const DEFAULT_SIZE_SHORT_SIDE_RATIO: f64 = 0.10;
pub(crate) const DEFAULT_EDGE_MARGIN_RATIO: f64 = 0.05;
pub(crate) const SIZE_PRESET_RATIOS: [f64; 4] = [0.08, 0.10, 0.13, 0.16];
pub(crate) const DEFAULT_SIZE_PRESETS: [f64; 4] = [64.0, 96.0, 128.0, 160.0];
pub(crate) const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 60;
pub(crate) const MIN_HEARTBEAT_INTERVAL_SEC: u64 = 5;
pub(crate) const MAX_HEARTBEAT_INTERVAL_SEC: u64 = 3600;
pub(crate) const UPDATE_CHECK_STARTUP_DELAY_SEC: u64 = 8;
pub(crate) const UPDATE_CHECK_BACKGROUND_INTERVAL_SEC: u64 = 6 * 60 * 60;
pub(crate) const UPDATE_BADGE_REMINDER_INTERVAL_SEC: i64 = 24 * 60 * 60;
pub(crate) const UPDATE_DOWNLOAD_FALLBACK_URL: &str =
    "https://github.com/samm81/downshift/releases/latest";
pub(crate) const DEFAULT_GITHUB_ISSUES_URL: &str = "github-issues-url-not-set";
pub(crate) const DEFAULT_SUPPORT_EMAIL: &str = "email-not-set";
pub(crate) const UPDATE_TOOLTIP: &str = "new version available";
pub(crate) const UPDATE_BADGE_WINDOW_RESERVE_PX: f64 = 32.0;
pub(crate) const SNOOZE_PRESET_MINUTES: [u64; 5] = [5, 10, 15, 30, 60];

pub(crate) const COMPILED_ENV: Option<&str> = option_env!("DOWNSHIFT_ENV");
pub(crate) const COMPILED_BUILD_CHANNEL: Option<&str> = option_env!("DOWNSHIFT_BUILD_CHANNEL");
pub(crate) const COMPILED_TELEMETRY_ENABLED: Option<&str> =
    option_env!("DOWNSHIFT_TELEMETRY_ENABLED");
pub(crate) const COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC: Option<&str> =
    option_env!("DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC");
pub(crate) const COMPILED_DOWNLOAD_RELEASE_URL: Option<&str> =
    option_env!("DOWNSHIFT_DOWNLOAD_RELEASE_URL");
pub(crate) const COMPILED_GITHUB_ISSUES_URL: Option<&str> =
    option_env!("DOWNSHIFT_GITHUB_ISSUES_URL");
pub(crate) const COMPILED_SUPPORT_EMAIL: Option<&str> = option_env!("DOWNSHIFT_SUPPORT_EMAIL");

#[derive(Debug, Clone)]
pub(crate) enum AppEvent {
    ExitRequested,
    Ipc(String),
    InstanceActivate,
    TelemetryHeartbeat,
    SnoozeExpired(u64),
    UpdateCheckFinished(UpdateCheckResult, UpdateCheckSource),
    // Menu events are produced only by hosts that expose native menus. Keeping
    // the event in the shared protocol lets the event loop stay platform-neutral.
    MenuActivated(String),
    TrayIconClicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivityMode {
    Active,
    Paused,
    Snoozed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstanceCommand {
    Activate,
}

impl InstanceCommand {
    pub(crate) fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Activate => b"activate\n",
        }
    }

    pub(crate) fn parse(input: &str) -> Option<Self> {
        match input.trim() {
            "activate" => Some(Self::Activate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateUiState {
    pub(crate) latest_version: Option<String>,
    pub(crate) download_url: String,
    pub(crate) checking: bool,
    pub(crate) checked_once: bool,
    pub(crate) badge_snoozed_version: Option<String>,
    pub(crate) badge_snoozed_at_epoch_seconds: Option<i64>,
    pub(crate) ignored_version: Option<String>,
}

impl Default for UpdateUiState {
    fn default() -> Self {
        Self {
            latest_version: None,
            download_url: download_release_url()
                .unwrap_or_else(|_| UPDATE_DOWNLOAD_FALLBACK_URL.to_string()),
            checking: false,
            checked_once: false,
            badge_snoozed_version: None,
            badge_snoozed_at_epoch_seconds: None,
            ignored_version: None,
        }
    }
}

impl UpdateUiState {
    pub(crate) fn has_update_available(&self) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        is_newer_version(latest, env!("CARGO_PKG_VERSION"))
    }

    pub(crate) fn is_ignoring_current_update(&self) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        self.ignored_version.as_deref() == Some(latest.as_str())
    }

    pub(crate) fn ignore_current_update_enabled(&self) -> bool {
        self.has_update_available()
    }

    pub(crate) fn should_show_badge(&self) -> bool {
        self.should_show_badge_at(now_epoch_seconds())
    }

    pub(crate) fn should_show_badge_at(&self, now_epoch_seconds: i64) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        if !self.has_update_available() || self.is_ignoring_current_update() {
            return false;
        }
        if self.badge_snoozed_version.as_deref() != Some(latest.as_str()) {
            return true;
        }
        let Some(snoozed_at) = self.badge_snoozed_at_epoch_seconds else {
            return true;
        };
        now_epoch_seconds - snoozed_at >= UPDATE_BADGE_REMINDER_INTERVAL_SEC
    }

    pub(crate) fn menu_label(&self) -> String {
        let current = env!("CARGO_PKG_VERSION");
        if self.has_update_available() {
            let latest = self.latest_version.as_deref().unwrap_or("unknown");
            return format!("get newest version ({latest}) (current {current})");
        }
        format!("check for updates (version {current})")
    }
}

#[derive(Debug, Clone)]
pub(crate) struct HeartbeatSnapshot {
    pub(crate) state: String,
    pub(crate) paused: bool,
    pub(crate) snoozed: bool,
    pub(crate) active_breathing_preset_id: String,
    pub(crate) breathing_pattern: BreathingPattern,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) usage_enabled: bool,
    pub(crate) crash_enabled: bool,
}

impl HeartbeatSnapshot {
    pub(crate) fn into_properties(self) -> serde_json::Value {
        serde_json::json!({
            "state": self.state,
            "config": {
                "paused": self.paused,
                "snoozed": self.snoozed,
                "active_breathing_preset_id": self.active_breathing_preset_id,
                "breathing_pattern": {
                    "expanding_seconds": self.breathing_pattern.expanding_seconds,
                    "expanded_hold_seconds": self.breathing_pattern.expanded_hold_seconds,
                    "compressing_seconds": self.breathing_pattern.compressing_seconds,
                    "compressed_hold_seconds": self.breathing_pattern.compressed_hold_seconds,
                    "total_seconds": self.breathing_pattern.expanding_seconds
                        + self.breathing_pattern.expanded_hold_seconds
                        + self.breathing_pattern.compressing_seconds
                        + self.breathing_pattern.compressed_hold_seconds,
                },
                "width_px": self.width_px,
                "height_px": self.height_px,
                "usage_enabled": self.usage_enabled,
                "crash_enabled": self.crash_enabled,
            }
        })
    }
}

pub(crate) fn emit_startup_telemetry(
    telemetry: &RuntimeTelemetryClient,
    initial_state: ActivityState,
    heartbeat_snapshot: HeartbeatSnapshot,
) {
    telemetry.start_session(initial_state);
    telemetry.track(
        EventName::SessionHeartbeat,
        heartbeat_snapshot.into_properties(),
    );
}

pub(crate) fn parse_version(input: &str) -> Option<Version> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Version::parse(trimmed.trim_start_matches('v')).ok()
}

pub(crate) fn is_newer_version(latest: &str, current: &str) -> bool {
    let Some(latest_version) = parse_version(latest) else {
        return false;
    };
    let Some(current_version) = parse_version(current) else {
        return false;
    };
    latest_version > current_version
}

pub(crate) fn optional_env_value(name: &str, compiled: Option<&str>) -> Option<String> {
    std::env::var(name)
        .ok()
        .or_else(|| compiled.map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn runtime_env_label() -> String {
    optional_env_value("DOWNSHIFT_ENV", COMPILED_ENV).unwrap_or_else(|| "unset".to_string())
}

pub(crate) fn build_channel_label() -> String {
    optional_env_value("DOWNSHIFT_BUILD_CHANNEL", COMPILED_BUILD_CHANNEL)
        .unwrap_or_else(|| "unset".to_string())
}

pub(crate) fn telemetry_globally_enabled() -> bool {
    optional_env_value("DOWNSHIFT_TELEMETRY_ENABLED", COMPILED_TELEMETRY_ENABLED)
        .map(|raw| !matches!(raw.to_ascii_lowercase().as_str(), "0" | "false" | "off"))
        .unwrap_or(true)
}

pub(crate) fn resolve_compiled_setting(
    compiled: Option<&str>,
    fallback: &str,
) -> Result<String, String> {
    if let Some(value) = compiled.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(value.to_string());
    }

    Ok(fallback.to_string())
}

pub(crate) fn download_release_url() -> Result<String, String> {
    resolve_compiled_setting(COMPILED_DOWNLOAD_RELEASE_URL, UPDATE_DOWNLOAD_FALLBACK_URL)
}

pub(crate) fn resolve_external_contact_value(
    env_name: &str,
    compiled: Option<&str>,
    fallback: &str,
) -> Result<String, String> {
    if let Some(value) = optional_env_value(env_name, compiled) {
        return Ok(value);
    }

    Ok(fallback.to_string())
}

pub(crate) fn telemetry_heartbeat_interval_seconds() -> Result<u64, String> {
    let raw = resolve_compiled_setting(COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC, "60")?;
    Ok(parse_heartbeat_interval_seconds(&raw))
}

pub(crate) fn github_issues_url() -> Result<String, String> {
    resolve_external_contact_value(
        "DOWNSHIFT_GITHUB_ISSUES_URL",
        COMPILED_GITHUB_ISSUES_URL,
        DEFAULT_GITHUB_ISSUES_URL,
    )
}

pub(crate) fn support_email_address() -> Result<String, String> {
    resolve_external_contact_value(
        "DOWNSHIFT_SUPPORT_EMAIL",
        COMPILED_SUPPORT_EMAIL,
        DEFAULT_SUPPORT_EMAIL,
    )
}

pub(crate) fn support_email_mailto() -> Result<String, String> {
    Ok(format!(
        "mailto:{}?subject=downshift%20bug%20report&body=please%20describe%20what%20happened%20and%20paste%20diagnostics%20if%20helpful.",
        support_email_address()?
    ))
}

pub(crate) fn now_epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

pub(crate) fn heartbeat_interval() -> Duration {
    Duration::from_secs(
        telemetry_heartbeat_interval_seconds().unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SEC),
    )
}

pub(crate) fn parse_heartbeat_interval_seconds(raw: &str) -> u64 {
    raw.trim()
        .parse::<u64>()
        .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SEC)
        .clamp(MIN_HEARTBEAT_INTERVAL_SEC, MAX_HEARTBEAT_INTERVAL_SEC)
}

pub(crate) fn breathing_pattern_total_seconds(pattern: &BreathingPattern) -> f64 {
    pattern.expanding_seconds
        + pattern.expanded_hold_seconds
        + pattern.compressing_seconds
        + pattern.compressed_hold_seconds
}

pub(crate) fn format_breathing_seconds(value: f64) -> String {
    if (value.fract()).abs() <= 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

pub(crate) fn breathing_pattern_summary(pattern: &BreathingPattern) -> String {
    format!(
        "{} / {} / {} / {}",
        format_breathing_seconds(pattern.expanding_seconds),
        format_breathing_seconds(pattern.expanded_hold_seconds),
        format_breathing_seconds(pattern.compressing_seconds),
        format_breathing_seconds(pattern.compressed_hold_seconds),
    )
}

pub(crate) fn breathing_pattern_payload(pattern: &BreathingPattern) -> serde_json::Value {
    serde_json::json!({
        "expanding_seconds": pattern.expanding_seconds,
        "expanded_hold_seconds": pattern.expanded_hold_seconds,
        "compressing_seconds": pattern.compressing_seconds,
        "compressed_hold_seconds": pattern.compressed_hold_seconds,
        "total_seconds": breathing_pattern_total_seconds(pattern),
    })
}

pub(crate) fn breathing_pattern_menu_label() -> String {
    "breathing pattern".to_string()
}

pub(crate) fn size_target_label(size_slot: usize) -> Option<&'static str> {
    menu_action_size_target(size_slot).map(|target| match target {
        SizeTarget::S => "S",
        SizeTarget::M => "M",
        SizeTarget::L => "L",
        SizeTarget::Xl => "XL",
    })
}

pub(crate) fn slugify_preset_name(name: &str) -> String {
    let mut id = String::new();
    let mut last_was_separator = false;
    for ch in name.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            id.push(normalized);
            last_was_separator = false;
        } else if !last_was_separator {
            id.push('_');
            last_was_separator = true;
        }
    }
    let id = id.trim_matches('_').to_string();
    if id.is_empty() {
        "preset".to_string()
    } else {
        id
    }
}

pub(crate) fn next_saved_preset_id(
    name: &str,
    saved_presets: &[SavedBreathingPreset],
    built_in_id_exists: impl Fn(&str) -> bool,
) -> String {
    let base = slugify_preset_name(name);
    let mut candidate = base.clone();
    let mut suffix = 2usize;
    while candidate == BREATHING_PRESET_ID_CUSTOM
        || built_in_id_exists(&candidate)
        || saved_presets.iter().any(|preset| preset.id == candidate)
    {
        candidate = format!("{base}_{suffix}");
        suffix += 1;
    }
    candidate
}

pub(crate) fn settings_backup_path(path: &std::path::Path) -> std::path::PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}.bak"))
        .unwrap_or_else(|| "settings.toml.bak".to_string());
    path.with_file_name(file_name)
}

pub(crate) fn drag_position(
    anchor_window: (f64, f64),
    anchor_pointer: (f64, f64),
    pointer: (f64, f64),
) -> (i32, i32) {
    let dx = pointer.0 - anchor_pointer.0;
    let dy = pointer.1 - anchor_pointer.1;
    (
        (anchor_window.0 + dx).round() as i32,
        (anchor_window.1 + dy).round() as i32,
    )
}

pub(crate) fn widget_window_dimensions(size: f64) -> (f64, f64) {
    (size, size + UPDATE_BADGE_WINDOW_RESERVE_PX)
}
