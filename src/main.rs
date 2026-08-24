#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use downshift::telemetry::{
    menu_action_size_target, telemetry_state, ActivityState, ActivityTrigger, EventName,
    MenuAction, RuntimeTelemetryClient, SessionEndReason, SizeTarget, TelemetryClient,
};
use downshift::{
    apply_resize_step, built_in_breathing_preset, built_in_breathing_presets, clamp_size,
    diagnostics, load_settings_result, BreathingPattern, IpcCommand, PersistedMonitor,
    SavedBreathingPreset, Settings, BREATHING_PRESET_ID_COHERENT, BREATHING_PRESET_ID_CUSTOM,
    DEFAULT_SIZE,
};
#[cfg(target_os = "macos")]
use downshift::{launch_agent_path_from_home, launch_agent_plist};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use muda::dpi::PhysicalPosition as MenuPhysicalPosition;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use muda::{
    CheckMenuItem, ContextMenu, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSView, NSWindowCollectionBehavior};
use semver::Version;
use std::hash::{Hash, Hasher};
#[cfg(unix)]
use std::io::Read;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::panic::PanicHookInfo;
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowAttributesExtWindows;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};
use wry::{Rect, WebView, WebViewBuilder};

const DEFAULT_SIZE_SHORT_SIDE_RATIO: f64 = 0.10;
const DEFAULT_EDGE_MARGIN_RATIO: f64 = 0.05;
const SIZE_PRESET_RATIOS: [f64; 4] = [0.08, 0.10, 0.13, 0.16];
const DEFAULT_SIZE_PRESETS: [f64; 4] = [64.0, 96.0, 128.0, 160.0];
const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 60;
const MIN_HEARTBEAT_INTERVAL_SEC: u64 = 5;
const MAX_HEARTBEAT_INTERVAL_SEC: u64 = 3600;
const UPDATE_CHECK_STARTUP_DELAY_SEC: u64 = 8;
const UPDATE_CHECK_BACKGROUND_INTERVAL_SEC: u64 = 6 * 60 * 60;
const UPDATE_BADGE_REMINDER_INTERVAL_SEC: i64 = 24 * 60 * 60;
const UPDATE_RELEASE_API_URL: &str =
    "https://api.github.com/repos/samm81/downshift/releases/latest";
const UPDATE_DOWNLOAD_FALLBACK_URL: &str = "https://github.com/samm81/downshift/releases/latest";
const DEFAULT_GITHUB_ISSUES_URL: &str = "github-issues-url-not-set";
const DEFAULT_SUPPORT_EMAIL: &str = "email-not-set";
const UPDATE_TOOLTIP: &str = "new version available";
const SNOOZE_PRESET_MINUTES: [u64; 5] = [5, 10, 15, 30, 60];
const COMPILED_ENV: Option<&str> = option_env!("DOWNSHIFT_ENV");
const COMPILED_BUILD_CHANNEL: Option<&str> = option_env!("DOWNSHIFT_BUILD_CHANNEL");
const COMPILED_TELEMETRY_ENABLED: Option<&str> = option_env!("DOWNSHIFT_TELEMETRY_ENABLED");
const COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC: Option<&str> =
    option_env!("DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC");
const COMPILED_DOWNLOAD_RELEASE_URL: Option<&str> = option_env!("DOWNSHIFT_DOWNLOAD_RELEASE_URL");
const COMPILED_GITHUB_ISSUES_URL: Option<&str> = option_env!("DOWNSHIFT_GITHUB_ISSUES_URL");
const COMPILED_SUPPORT_EMAIL: Option<&str> = option_env!("DOWNSHIFT_SUPPORT_EMAIL");
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod menu_ids {
    pub(super) const MENU_ID_PAUSE: &str = "pause";
    pub(super) const MENU_ID_SNOOZE_ROOT: &str = "snooze_root";
    pub(super) const MENU_ID_SNOOZE_5: &str = "snooze_5";
    pub(super) const MENU_ID_SNOOZE_10: &str = "snooze_10";
    pub(super) const MENU_ID_SNOOZE_15: &str = "snooze_15";
    pub(super) const MENU_ID_SNOOZE_30: &str = "snooze_30";
    pub(super) const MENU_ID_SNOOZE_60: &str = "snooze_60";
    pub(super) const MENU_ID_SNOOZE_CUSTOM: &str = "snooze_custom";
    pub(super) const MENU_ID_SIZE_S: &str = "size_s";
    pub(super) const MENU_ID_SIZE_M: &str = "size_m";
    pub(super) const MENU_ID_SIZE_L: &str = "size_l";
    pub(super) const MENU_ID_SIZE_XL: &str = "size_xl";
    pub(super) const MENU_ID_BREATHING_PATTERN: &str = "breathing_pattern";
    pub(super) const MENU_ID_BREATHING_COHERENT: &str = "breathing_coherent";
    pub(super) const MENU_ID_BREATHING_BOX: &str = "breathing_box";
    pub(super) const MENU_ID_BREATHING_479: &str = "breathing_479";
    pub(super) const MENU_ID_BREATHING_EDIT: &str = "breathing_edit";
    pub(super) const MENU_ID_BREATHING_DELETE_ROOT: &str = "breathing_delete_root";
    pub(super) const MENU_ID_BREATHING_DELETE_PREFIX: &str = "breathing_delete:";
    pub(super) const MENU_ID_BREATHING_SAVED_PREFIX: &str = "breathing_saved:";
    pub(super) const MENU_ID_RESET: &str = "reset";
    pub(super) const MENU_ID_QUIT: &str = "quit";
    pub(super) const MENU_ID_ANALYTICS_ROOT: &str = "analytics_root";
    pub(super) const MENU_ID_USAGE_ON: &str = "usage_on";
    pub(super) const MENU_ID_USAGE_OFF: &str = "usage_off";
    pub(super) const MENU_ID_CRASH_ON: &str = "crash_on";
    pub(super) const MENU_ID_CRASH_OFF: &str = "crash_off";
    pub(super) const MENU_ID_ANALYTICS_INFO: &str = "analytics_info";
    pub(super) const MENU_ID_UPDATE_ROOT: &str = "update_root";
    pub(super) const MENU_ID_UPDATE_PRIMARY: &str = "update_primary";
    pub(super) const MENU_ID_UPDATE_IGNORE_CURRENT: &str = "update_ignore_current";
    pub(super) const MENU_ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
    pub(super) const MENU_ID_BUGS_ROOT: &str = "bugs_root";
    pub(super) const MENU_ID_COPY_DIAGNOSTICS: &str = "copy_diagnostics";
    pub(super) const MENU_ID_FILE_BUG_GITHUB: &str = "file_bug_github";
    pub(super) const MENU_ID_FILE_BUG_EMAIL: &str = "file_bug_email";
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
use menu_ids::*;

#[cfg(target_os = "macos")]
fn configure_window_for_all_spaces(window: &Window) {
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
macro_rules! log_stderr {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        diagnostics::log_line("ERROR", &message);
    }};
}

struct HeartbeatSnapshot {
    state: String,
    paused: bool,
    snoozed: bool,
    active_breathing_preset_id: String,
    breathing_pattern: BreathingPattern,
    width_px: u32,
    height_px: u32,
    usage_enabled: bool,
    crash_enabled: bool,
}

impl HeartbeatSnapshot {
    fn into_properties(self) -> serde_json::Value {
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

fn emit_startup_telemetry(
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

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| launch_agent_path_from_home(&home))
}

#[cfg(target_os = "macos")]
fn write_launch_agent(path: &Path, executable: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid launch agent path: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, launch_agent_plist(executable)).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
fn remove_launch_agent(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(target_os = "windows")]
const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const WINDOWS_RUN_VALUE: &str = "Downshift";

#[cfg(target_os = "windows")]
fn set_windows_launch_at_login(enabled: bool) -> Result<(), String> {
    let output = if enabled {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let command = format!("\"{}\"", executable.display());
        std::process::Command::new("reg.exe")
            .args([
                "add",
                WINDOWS_RUN_KEY,
                "/v",
                WINDOWS_RUN_VALUE,
                "/t",
                "REG_SZ",
                "/d",
                &command,
                "/f",
            ])
            .output()
            .map_err(|error| error.to_string())?
    } else {
        std::process::Command::new("reg.exe")
            .args(["delete", WINDOWS_RUN_KEY, "/v", WINDOWS_RUN_VALUE, "/f"])
            .output()
            .map_err(|error| error.to_string())?
    };

    if output.status.success()
        || (!enabled
            && String::from_utf8_lossy(&output.stderr)
                .to_ascii_lowercase()
                .contains("unable to find"))
    {
        return Ok(());
    }

    let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if details.is_empty() {
        format!("reg.exe exited with status {}", output.status)
    } else {
        details
    })
}

const INLINE_STYLE_PLACEHOLDER: &str = "__DOWNSHIFT_INLINE_STYLE__";
const INLINE_SCRIPT_PLACEHOLDER: &str = "__DOWNSHIFT_INLINE_SCRIPT__";

const BREATH_HTML_TEMPLATE: &str = include_str!("ui/breath.html");
const BREATH_CSS: &str = include_str!("ui/breath.css");
const BREATH_JS: &str = include_str!("ui/breath.js");

const TELEMETRY_INFO_HTML_TEMPLATE: &str = include_str!("ui/telemetry-info.html");
const TELEMETRY_INFO_CSS: &str = include_str!("ui/telemetry-info.css");
const TELEMETRY_INFO_JS: &str = include_str!("ui/telemetry-info.js");

const UPDATE_DIALOG_HTML_TEMPLATE: &str = include_str!("ui/update-dialog.html");
const UPDATE_DIALOG_CSS: &str = include_str!("ui/update-dialog.css");
const UPDATE_DIALOG_JS: &str = include_str!("ui/update-dialog.js");

const CUSTOM_SNOOZE_HTML_TEMPLATE: &str = include_str!("ui/custom-snooze.html");
const CUSTOM_SNOOZE_CSS: &str = include_str!("ui/custom-snooze.css");
const CUSTOM_SNOOZE_JS: &str = include_str!("ui/custom-snooze.js");

const BREATHING_PATTERN_HTML_TEMPLATE: &str = include_str!("ui/breathing-pattern.html");
const BREATHING_PATTERN_CSS: &str = include_str!("ui/breathing-pattern.css");
const BREATHING_PATTERN_JS: &str = include_str!("ui/breathing-pattern.js");

static BREATH_HTML: OnceLock<String> = OnceLock::new();
static TELEMETRY_INFO_HTML: OnceLock<String> = OnceLock::new();
static UPDATE_DIALOG_HTML: OnceLock<String> = OnceLock::new();
static CUSTOM_SNOOZE_HTML: OnceLock<String> = OnceLock::new();
static BREATHING_PATTERN_HTML: OnceLock<String> = OnceLock::new();

fn inline_ui_assets(template: &str, css: &str, js: &str) -> String {
    template
        .replace(INLINE_STYLE_PLACEHOLDER, css.trim())
        .replace(INLINE_SCRIPT_PLACEHOLDER, js.trim())
}

fn breath_html() -> &'static str {
    BREATH_HTML.get_or_init(|| inline_ui_assets(BREATH_HTML_TEMPLATE, BREATH_CSS, BREATH_JS))
}

fn telemetry_info_html() -> &'static str {
    TELEMETRY_INFO_HTML.get_or_init(|| {
        inline_ui_assets(
            TELEMETRY_INFO_HTML_TEMPLATE,
            TELEMETRY_INFO_CSS,
            TELEMETRY_INFO_JS,
        )
    })
}

fn update_dialog_html() -> &'static str {
    UPDATE_DIALOG_HTML.get_or_init(|| {
        inline_ui_assets(
            UPDATE_DIALOG_HTML_TEMPLATE,
            UPDATE_DIALOG_CSS,
            UPDATE_DIALOG_JS,
        )
    })
}

fn custom_snooze_html() -> &'static str {
    CUSTOM_SNOOZE_HTML.get_or_init(|| {
        inline_ui_assets(
            CUSTOM_SNOOZE_HTML_TEMPLATE,
            CUSTOM_SNOOZE_CSS,
            CUSTOM_SNOOZE_JS,
        )
    })
}

fn breathing_pattern_html() -> &'static str {
    BREATHING_PATTERN_HTML.get_or_init(|| {
        inline_ui_assets(
            BREATHING_PATTERN_HTML_TEMPLATE,
            BREATHING_PATTERN_CSS,
            BREATHING_PATTERN_JS,
        )
    })
}

#[derive(Debug, Clone)]
enum AppEvent {
    ExitRequested,
    Ipc(String),
    InstanceActivate,
    TelemetryHeartbeat,
    SnoozeExpired(u64),
    UpdateCheckFinished(UpdateCheckResult, UpdateCheckSource),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    MenuActivated(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivityMode {
    Active,
    Paused,
    Snoozed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceCommand {
    Activate,
}

impl InstanceCommand {
    fn as_bytes(self) -> &'static [u8] {
        match self {
            Self::Activate => b"activate\n",
        }
    }

    fn parse(input: &str) -> Option<Self> {
        match input.trim() {
            "activate" => Some(Self::Activate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpdateCheckSource {
    Background,
    Manual,
}

impl UpdateCheckSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone)]
struct UpdateCheckResult {
    latest_version: Option<String>,
    download_url: String,
}

#[derive(Debug, Clone)]
struct UpdateUiState {
    latest_version: Option<String>,
    download_url: String,
    checking: bool,
    checked_once: bool,
    badge_snoozed_version: Option<String>,
    badge_snoozed_at_epoch_seconds: Option<i64>,
    ignored_version: Option<String>,
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
    fn has_update_available(&self) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        is_newer_version(latest, env!("CARGO_PKG_VERSION"))
    }

    fn is_ignoring_current_update(&self) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        self.ignored_version.as_deref() == Some(latest.as_str())
    }

    fn ignore_current_update_enabled(&self) -> bool {
        if !self.has_update_available() {
            return false;
        }
        true
    }

    fn should_show_badge(&self) -> bool {
        self.should_show_badge_at(now_epoch_seconds())
    }

    fn should_show_badge_at(&self, now_epoch_seconds: i64) -> bool {
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

    fn menu_label(&self) -> String {
        let current = env!("CARGO_PKG_VERSION");
        if self.has_update_available() {
            let latest = self.latest_version.as_deref().unwrap_or("unknown");
            return format!("get newest version ({latest}) (current {current})");
        }
        format!("check for updates (version {current})")
    }
}

fn parse_version(input: &str) -> Option<Version> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    Version::parse(trimmed.trim_start_matches('v')).ok()
}

fn is_newer_version(latest: &str, current: &str) -> bool {
    let Some(latest_version) = parse_version(latest) else {
        return false;
    };
    let Some(current_version) = parse_version(current) else {
        return false;
    };
    latest_version > current_version
}

fn open_external_url(url: &str) {
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
        log_stderr!("warning: failed to open external url: {error}");
    }
}

fn optional_env_value(name: &str, compiled: Option<&str>) -> Option<String> {
    std::env::var(name)
        .ok()
        .or_else(|| compiled.map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn runtime_env_label() -> String {
    optional_env_value("DOWNSHIFT_ENV", COMPILED_ENV).unwrap_or_else(|| "unset".to_string())
}

fn build_channel_label() -> String {
    optional_env_value("DOWNSHIFT_BUILD_CHANNEL", COMPILED_BUILD_CHANNEL)
        .unwrap_or_else(|| "unset".to_string())
}

fn telemetry_globally_enabled() -> bool {
    optional_env_value("DOWNSHIFT_TELEMETRY_ENABLED", COMPILED_TELEMETRY_ENABLED)
        .map(|raw| !matches!(raw.to_ascii_lowercase().as_str(), "0" | "false" | "off"))
        .unwrap_or(true)
}

fn resolve_compiled_setting(compiled: Option<&str>, fallback: &str) -> Result<String, String> {
    if let Some(value) = compiled.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(value.to_string());
    }

    Ok(fallback.to_string())
}

fn download_release_url() -> Result<String, String> {
    resolve_compiled_setting(COMPILED_DOWNLOAD_RELEASE_URL, UPDATE_DOWNLOAD_FALLBACK_URL)
}

fn resolve_external_contact_value(
    env_name: &str,
    compiled: Option<&str>,
    fallback: &str,
) -> Result<String, String> {
    if let Some(value) = optional_env_value(env_name, compiled) {
        return Ok(value);
    }

    Ok(fallback.to_string())
}

fn telemetry_heartbeat_interval_seconds() -> Result<u64, String> {
    let raw = resolve_compiled_setting(COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC, "60")?;
    Ok(parse_heartbeat_interval_seconds(&raw))
}

fn github_issues_url() -> Result<String, String> {
    resolve_external_contact_value(
        "DOWNSHIFT_GITHUB_ISSUES_URL",
        COMPILED_GITHUB_ISSUES_URL,
        DEFAULT_GITHUB_ISSUES_URL,
    )
}

fn support_email_address() -> Result<String, String> {
    resolve_external_contact_value(
        "DOWNSHIFT_SUPPORT_EMAIL",
        COMPILED_SUPPORT_EMAIL,
        DEFAULT_SUPPORT_EMAIL,
    )
}

fn support_email_mailto() -> Result<String, String> {
    Ok(format!(
        "mailto:{}?subject=downshift%20bug%20report&body=please%20describe%20what%20happened%20and%20paste%20diagnostics%20if%20helpful.",
        support_email_address()?
    ))
}

fn current_os_version() -> String {
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

fn copy_text_to_clipboard(text: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut process = std::process::Command::new("pbcopy")
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
        return if status.success() {
            Ok(())
        } else {
            Err(format!("pbcopy exited with status {status}"))
        };
    }
    #[cfg(target_os = "windows")]
    {
        let mut process = std::process::Command::new("clip.exe")
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
        return if status.success() {
            Ok(())
        } else {
            Err(format!("clip.exe exited with status {status}"))
        };
    }
    #[allow(unreachable_code)]
    Err("clipboard copy is unsupported on this platform".to_string())
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

fn now_epoch_seconds() -> i64 {
    use std::time::UNIX_EPOCH;

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn check_latest_release() -> UpdateCheckResult {
    let response = ureq::get(UPDATE_RELEASE_API_URL)
        .set("User-Agent", "downshift")
        .call();
    let Ok(response) = response else {
        return UpdateCheckResult {
            latest_version: None,
            download_url: download_release_url()
                .unwrap_or_else(|_| UPDATE_DOWNLOAD_FALLBACK_URL.to_string()),
        };
    };
    let body = response.into_string().unwrap_or_default();
    let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let latest_version = data
        .get("tag_name")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let download_url = data
        .get("html_url")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            download_release_url().unwrap_or_else(|_| UPDATE_DOWNLOAD_FALLBACK_URL.to_string())
        });
    UpdateCheckResult {
        latest_version,
        download_url,
    }
}

fn spawn_update_check(proxy: EventLoopProxy<AppEvent>, source: UpdateCheckSource, delay_sec: u64) {
    std::thread::spawn(move || {
        if delay_sec > 0 {
            std::thread::sleep(Duration::from_secs(delay_sec));
        }
        let result = check_latest_release();
        let _ = proxy.send_event(AppEvent::UpdateCheckFinished(result, source));
    });
}

fn breathing_pattern_total_seconds(pattern: &BreathingPattern) -> f64 {
    pattern.expanding_seconds
        + pattern.expanded_hold_seconds
        + pattern.compressing_seconds
        + pattern.compressed_hold_seconds
}

fn format_breathing_seconds(value: f64) -> String {
    if (value.fract()).abs() <= 0.001 {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

fn breathing_pattern_summary(pattern: &BreathingPattern) -> String {
    format!(
        "{} / {} / {} / {}",
        format_breathing_seconds(pattern.expanding_seconds),
        format_breathing_seconds(pattern.expanded_hold_seconds),
        format_breathing_seconds(pattern.compressing_seconds),
        format_breathing_seconds(pattern.compressed_hold_seconds),
    )
}

fn breathing_pattern_payload(pattern: &BreathingPattern) -> serde_json::Value {
    serde_json::json!({
        "expanding_seconds": pattern.expanding_seconds,
        "expanded_hold_seconds": pattern.expanded_hold_seconds,
        "compressing_seconds": pattern.compressing_seconds,
        "compressed_hold_seconds": pattern.compressed_hold_seconds,
        "total_seconds": breathing_pattern_total_seconds(pattern),
    })
}

fn breathing_pattern_menu_label() -> String {
    "breathing pattern".to_string()
}

fn size_target_label(size_slot: usize) -> Option<&'static str> {
    menu_action_size_target(size_slot).map(|target| match target {
        SizeTarget::S => "S",
        SizeTarget::M => "M",
        SizeTarget::L => "L",
        SizeTarget::Xl => "XL",
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn breathing_delete_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_DELETE_PREFIX}{id}")
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn deleted_breathing_preset_id_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MENU_ID_BREATHING_DELETE_PREFIX)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn breathing_saved_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_SAVED_PREFIX}{id}")
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn saved_breathing_preset_id_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MENU_ID_BREATHING_SAVED_PREFIX)
}

fn slugify_preset_name(name: &str) -> String {
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Clone)]
struct NativeContextMenu {
    root: Submenu,
    pause: CheckMenuItem,
    launch_at_login: CheckMenuItem,
    snooze_menu: Submenu,
    snooze_5: MenuItem,
    snooze_10: MenuItem,
    snooze_15: MenuItem,
    snooze_30: MenuItem,
    snooze_60: MenuItem,
    snooze_custom: MenuItem,
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
    reset: MenuItem,
    quit: MenuItem,
    update_menu: Submenu,
    update_primary: MenuItem,
    update_ignore_current: CheckMenuItem,
    bugs_menu: Submenu,
    copy_diagnostics: MenuItem,
    file_bug_github: MenuItem,
    file_bug_email: MenuItem,
    analytics_menu: Submenu,
    usage_on: CheckMenuItem,
    usage_off: CheckMenuItem,
    crash_on: CheckMenuItem,
    crash_off: CheckMenuItem,
    analytics_info: MenuItem,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl NativeContextMenu {
    fn new(settings: &Settings) -> Option<Self> {
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
            breathing_pattern_menu_label(),
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
        let root = match Submenu::with_items(
            "menu",
            true,
            &[
                &pause,
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
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                log_stderr!("warning: failed to build native context menu: {error}");
                return None;
            }
        };
        Some(Self {
            root,
            pause,
            launch_at_login,
            snooze_menu: snooze_submenu,
            snooze_5,
            snooze_10,
            snooze_15,
            snooze_30,
            snooze_60,
            snooze_custom,
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
            reset,
            quit,
            update_menu,
            update_primary,
            update_ignore_current,
            bugs_menu,
            copy_diagnostics,
            file_bug_github,
            file_bug_email,
            analytics_menu,
            usage_on,
            usage_off,
            crash_on,
            crash_off,
            analytics_info,
        })
    }

    fn sync_from_settings(
        &self,
        settings: &Settings,
        size_presets: [f64; 4],
        update_label: &str,
        update_ignore_enabled: bool,
        update_ignore_checked: bool,
    ) {
        self.pause.set_checked(settings.paused);
        self.pause
            .set_text(if settings.paused { "paused" } else { "pause" });
        self.launch_at_login.set_checked(settings.launch_at_login);
        self.launch_at_login.set_enabled(true);
        self.snooze_menu.set_enabled(true);
        self.snooze_5.set_enabled(true);
        self.snooze_10.set_enabled(true);
        self.snooze_15.set_enabled(true);
        self.snooze_30.set_enabled(true);
        self.snooze_60.set_enabled(true);
        self.snooze_custom.set_enabled(true);
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
        let _ = settings;
        self.breathing_menu.set_text(breathing_pattern_menu_label());
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
        self.reset.set_enabled(true);
        self.quit.set_enabled(true);
        self.update_menu.set_enabled(true);
        self.update_primary.set_text(update_label);
        self.update_primary.set_enabled(true);
        self.update_ignore_current
            .set_enabled(update_ignore_enabled);
        self.update_ignore_current
            .set_checked(update_ignore_checked);
        self.bugs_menu.set_enabled(true);
        self.copy_diagnostics.set_enabled(true);
        self.file_bug_github.set_enabled(true);
        self.file_bug_email.set_enabled(true);
        self.analytics_menu.set_enabled(true);
    }

    fn sync_consent(&self, usage_enabled: bool, crash_enabled: bool) {
        self.usage_on.set_checked(usage_enabled);
        self.usage_off.set_checked(!usage_enabled);
        self.crash_on.set_checked(crash_enabled);
        self.crash_off.set_checked(!crash_enabled);
        self.analytics_info.set_enabled(true);
    }
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
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    native_context_menu: Option<NativeContextMenu>,
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
    manual_update_check_in_flight: bool,
}

impl App {
    fn new(telemetry: RuntimeTelemetryClient, telemetry_install_first_run: bool) -> Self {
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
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            native_context_menu: None,
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
            manual_update_check_in_flight: false,
        }
    }
}

#[cfg(test)]
impl Default for App {
    fn default() -> Self {
        let (telemetry, telemetry_install_first_run) = bootstrap_telemetry();
        Self::new(telemetry, telemetry_install_first_run)
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
        let window_position = self.current_window_logical_position().map(|position| {
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
            .map(snapshot_monitor)
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

    #[cfg(target_os = "macos")]
    fn sync_launch_at_login_setting(&mut self, enabled: bool) {
        let Some(path) = launch_agent_path() else {
            log_stderr!("warning: failed to resolve launch agent path");
            return;
        };

        let result = if enabled {
            match std::env::current_exe() {
                Ok(executable) => write_launch_agent(&path, &executable),
                Err(error) => Err(error.to_string()),
            }
        } else {
            remove_launch_agent(&path)
        };

        match result {
            Ok(()) => {
                self.settings.launch_at_login = enabled;
            }
            Err(error) => {
                log_stderr!("warning: failed to update launch-at-login setting: {error}");
            }
        }
    }

    #[cfg(target_os = "windows")]
    fn sync_launch_at_login_setting(&mut self, enabled: bool) {
        let result = set_windows_launch_at_login(enabled);
        match result {
            Ok(()) => {
                self.settings.launch_at_login = enabled;
            }
            Err(error) => {
                log_stderr!("warning: failed to update launch-at-login setting: {error}");
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn apply_launch_at_login(&mut self, enabled: bool) {
        self.sync_launch_at_login_setting(enabled);
        self.sync_update_menu_state();
        self.save_settings();
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn reconcile_launch_at_login(&mut self) {
        self.sync_launch_at_login_setting(self.settings.launch_at_login);
    }

    fn widget_dimensions_px(&self) -> (u32, u32) {
        if let Some(window) = self.window.as_ref() {
            let size = window.inner_size();
            return (size.width, size.height);
        }
        let size = self.settings.size.round().max(0.0) as u32;
        (size, size)
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
        self.drag_anchor_window_pos = self.current_window_logical_position();
        self.drag_anchor_pointer_pos = Some(LogicalPosition::new(screen_x as f64, screen_y as f64));
    }

    fn drag_to(&mut self, screen_x: i32, screen_y: i32) {
        let (Some(anchor_window), Some(anchor_pointer), Some(window)) = (
            self.drag_anchor_window_pos,
            self.drag_anchor_pointer_pos,
            self.window.as_ref(),
        ) else {
            return;
        };

        let dx = screen_x as f64 - anchor_pointer.x;
        let dy = screen_y as f64 - anchor_pointer.y;
        let next_x = (anchor_window.x + dx).round() as i32;
        let next_y = (anchor_window.y + dy).round() as i32;
        window.set_outer_position(LogicalPosition::new(next_x, next_y));
    }

    fn stop_manual_drag(&mut self) {
        self.drag_anchor_window_pos = None;
        self.drag_anchor_pointer_pos = None;
    }

    fn enforce_fixed_square_size(&self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let target = clamp_size(self.settings.size);
        let current = window.inner_size().to_logical::<f64>(window.scale_factor());
        let width_mismatch = (current.width - target).abs() > 0.5;
        let height_mismatch = (current.height - target).abs() > 0.5;

        window.set_resizable(false);
        window.set_min_inner_size(Some(LogicalSize::new(target, target)));
        window.set_max_inner_size(Some(LogicalSize::new(target, target)));
        if width_mismatch || height_mismatch {
            let _ = window.request_inner_size(LogicalSize::new(target, target));
        }
    }

    fn sync_webview_bounds(&self) {
        // Wry's non-child Windows WebView2 path subclasses the parent HWND and
        // resizes the controller directly from WM_SIZE. That path also avoids
        // racing request_inner_size with a second asynchronous SetWindowPos.
        // Keep the explicit bounds sync for macOS, where this app owns the
        // child view geometry.
        #[cfg(not(target_os = "windows"))]
        sync_child_webview_bounds(self.window.as_ref(), self.webview.as_ref(), "main webview");
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
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| format!("{name}.bak"))
            .unwrap_or_else(|| "settings.toml.bak".to_string());
        path.with_file_name(file_name)
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if let Some(menu) = self.native_context_menu.as_ref() {
            menu.sync_from_settings(
                &self.settings,
                self.current_size_presets(),
                &self.updates.menu_label(),
                self.updates.ignore_current_update_enabled(),
                self.updates.is_ignoring_current_update(),
            );
        }
    }

    fn sync_update_state_to_webview(&self) {
        self.apply_main_webview_state(serde_json::json!({
            "update_menu_label": self.updates.menu_label(),
            "update_has_new_version": self.updates.has_update_available(),
            "update_show_badge": self.updates.should_show_badge(),
            "update_ignore_current_enabled": self.updates.ignore_current_update_enabled(),
            "update_ignore_current_checked": self.updates.is_ignoring_current_update(),
        }));
    }

    fn sync_update_surfaces(&self) {
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
        if let Some(latest) = result.latest_version {
            self.updates.latest_version = Some(latest.clone());
            self.settings.cached_latest_update_version = Some(latest);
            self.save_settings();
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
        let (Some(window), Some(webview)) = (
            self.update_dialog_window.as_ref(),
            self.update_dialog_webview.as_ref(),
        ) else {
            return;
        };
        let size = window.inner_size().to_logical::<u32>(window.scale_factor());
        let bounds = Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(size.width, size.height).into(),
        };
        if let Err(error) = webview.set_bounds(bounds) {
            log_stderr!("warning: failed to sync update dialog webview bounds: {error}");
        }
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
            format!(
                "window.updateDialogApplyState({{ mode: 'available', latest_version: {} }});",
                serde_json::json!(self.updates.latest_version)
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
        spawn_update_check(proxy, UpdateCheckSource::Manual, 0);
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

        if let (Some(saved_x), Some(saved_y)) = (self.settings.physical_x, self.settings.physical_y)
        {
            let saved = PhysicalPosition::new(saved_x, saved_y);
            if let Some(saved_monitor) = self.settings.monitor.as_ref() {
                if let Some(current) = monitors
                    .iter()
                    .find(|monitor| monitor_matches_persisted(monitor, saved_monitor))
                {
                    if position_fits_monitor(saved, size, current) {
                        return Some(saved);
                    }
                } else {
                    // Display config changed (for example resolution), so reuse corner-relative spawn.
                    return Some(default_corner_position(&primary, size));
                }
            } else if monitors
                .iter()
                .any(|monitor| position_fits_monitor(saved, size, monitor))
            {
                return Some(saved);
            }
        }

        if let Some(saved) =
            self.choose_initial_position_from_legacy_logical(&monitors, &primary, size)
        {
            return Some(saved);
        }

        Some(default_corner_position(&primary, size))
    }

    fn choose_initial_position_from_legacy_logical(
        &self,
        monitors: &[MonitorHandle],
        primary: &MonitorHandle,
        size: f64,
    ) -> Option<PhysicalPosition<i32>> {
        let (Some(saved_x), Some(saved_y)) = (self.settings.x, self.settings.y) else {
            return None;
        };
        let saved = LogicalPosition::new(saved_x as f64, saved_y as f64);

        if let Some(saved_monitor) = self.settings.monitor.as_ref() {
            if let Some(current) = monitors
                .iter()
                .find(|monitor| monitor_matches_persisted(monitor, saved_monitor))
            {
                if position_fits_monitor_legacy(saved, size, current) {
                    return Some(logical_to_physical_position(saved, current.scale_factor()));
                }
            } else {
                return Some(default_corner_position(primary, size));
            }
        } else if let Some(current) = monitors
            .iter()
            .find(|monitor| position_fits_monitor_legacy(saved, size, monitor))
        {
            return Some(logical_to_physical_position(saved, current.scale_factor()));
        }

        None
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
          "update_ignore_current_enabled": self.updates.ignore_current_update_enabled(),
          "update_ignore_current_checked": self.updates.is_ignoring_current_update(),
          "update_tooltip": UPDATE_TOOLTIP,
          "size_presets": size_presets,
          "use_native_menu": cfg!(any(target_os = "macos", target_os = "windows")),
        });
        format!("window.__BB_INIT__ = {payload};")
    }

    fn current_window_logical_position(&self) -> Option<LogicalPosition<f64>> {
        let window = self.window.as_ref()?;
        let physical = window.outer_position().ok()?;
        Some(physical.to_logical(window.scale_factor()))
    }

    fn current_window_physical_position(&self) -> Option<PhysicalPosition<i32>> {
        let window = self.window.as_ref()?;
        window.outer_position().ok()
    }

    fn apply_size(&mut self, size: f64) {
        let window = match self.window.as_ref() {
            Some(window) => window,
            None => return,
        };
        let size = clamp_size(size);
        let old_size = self.settings.size;
        self.settings.size = size;
        window.set_min_inner_size(Some(LogicalSize::new(size, size)));
        window.set_max_inner_size(Some(LogicalSize::new(size, size)));
        let _ = window.request_inner_size(LogicalSize::new(size, size));

        if let Some(current_pos) = self.current_window_logical_position() {
            let center_x = current_pos.x + old_size / 2.0;
            let center_y = current_pos.y + old_size / 2.0;
            let next_x = (center_x - size / 2.0).round() as i32;
            let next_y = (center_y - size / 2.0).round() as i32;
            window.set_outer_position(LogicalPosition::new(next_x, next_y));
            if let Some(physical) = self.current_window_physical_position() {
                self.settings.physical_x = Some(physical.x);
                self.settings.physical_y = Some(physical.y);
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

    fn sync_breathing_pattern_surfaces(&self) {
        self.sync_breathing_pattern_to_webview();
        self.sync_update_menu_state();
        self.sync_breathing_pattern_editor_state();
    }

    fn next_saved_breathing_preset_id(&self, name: &str) -> String {
        let base = slugify_preset_name(name);
        let mut candidate = base.clone();
        let mut suffix = 2usize;
        while candidate == BREATHING_PRESET_ID_CUSTOM
            || built_in_breathing_preset(&candidate).is_some()
            || self
                .settings
                .saved_breathing_presets
                .iter()
                .any(|preset| preset.id == candidate)
        {
            candidate = format!("{base}_{suffix}");
            suffix += 1;
        }
        candidate
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
        if let Some(window) = self.window.as_ref() {
            self.settings.physical_x = Some(physical.x);
            self.settings.physical_y = Some(physical.y);
            let current_monitor = window.current_monitor();
            self.settings.monitor = current_monitor.clone().map(snapshot_monitor);
            if let Some(monitor) = current_monitor {
                self.apply_size_presets_for_monitor(&monitor);
            }
        }
    }

    fn reset_widget(&mut self, event_loop: &ActiveEventLoop) {
        let monitor = self
            .window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .or_else(|| event_loop.primary_monitor())
            .or_else(|| event_loop.available_monitors().next());
        let reset_size = monitor
            .as_ref()
            .map(default_size_for_monitor)
            .unwrap_or(DEFAULT_SIZE);
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
            if let Some(monitor) = monitor {
                let pos = default_corner_position(&monitor, self.settings.size);
                window.set_outer_position(pos);
                self.settings.physical_x = Some(pos.x);
                self.settings.physical_y = Some(pos.y);
                self.settings.monitor = Some(snapshot_monitor(monitor));
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
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                self.show_native_context_menu(x, y);
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                let _ = (x, y);
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
            IpcCommand::Reset => {
                self.telemetry_menu_action(MenuAction::Reset, None);
                self.reset_widget(event_loop);
                self.save_settings();
            }
        }
    }

    fn apply_size_presets_for_monitor(&self, monitor: &MonitorHandle) {
        let presets = size_presets_for_monitor(monitor);
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
            .map(|monitor| size_presets_for_monitor(&monitor))
            .and_then(|presets| {
                presets
                    .iter()
                    .enumerate()
                    .find(|(_, preset)| (**preset - size).abs() <= 0.5)
                    .map(|(index, _)| index)
            })
            .and_then(size_target_label)
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn current_size_presets(&self) -> [f64; 4] {
        self.window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .map(|monitor| size_presets_for_monitor(&monitor))
            .unwrap_or(DEFAULT_SIZE_PRESETS)
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn show_native_context_menu(&mut self, x: i32, y: i32) {
        self.native_context_menu = NativeContextMenu::new(&self.settings);
        let Some(menu) = self.native_context_menu.as_ref() else {
            return;
        };
        let Some(window) = self.window.as_ref() else {
            return;
        };
        menu.sync_from_settings(
            &self.settings,
            self.current_size_presets(),
            &self.updates.menu_label(),
            self.updates.ignore_current_update_enabled(),
            self.updates.is_ignoring_current_update(),
        );
        menu.sync_consent(
            self.settings.usage_data_sharing,
            self.settings.crash_reports_sharing,
        );

        #[cfg(target_os = "macos")]
        {
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
            let position = MenuPhysicalPosition::new(x as f64, y as f64).into();
            unsafe {
                let _ = menu
                    .root
                    .show_context_menu_for_nsview(view.cast_const(), Some(position));
            }
        }

        #[cfg(target_os = "windows")]
        {
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
            let position = MenuPhysicalPosition::new(x as f64, y as f64).into();
            unsafe {
                let _ = menu.root.show_context_menu_for_hwnd(hwnd, Some(position));
            }
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn handle_native_menu_activation(&mut self, event_loop: &ActiveEventLoop, id: &str) {
        match id {
            MENU_ID_PAUSE => {
                self.set_paused_from_user_action(!self.settings.paused);
            }
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
            MENU_ID_COPY_DIAGNOSTICS => self.copy_diagnostics_summary(),
            MENU_ID_FILE_BUG_GITHUB => match github_issues_url() {
                Ok(url) => open_external_url(&url),
                Err(error) => log_stderr!("error: {error}"),
            },
            MENU_ID_FILE_BUG_EMAIL => match support_email_mailto() {
                Ok(url) => open_external_url(&url),
                Err(error) => log_stderr!("error: {error}"),
            },
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        self.reconcile_launch_at_login();
        if !settings_exist {
            if let Some(primary) = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())
            {
                self.settings.size = default_size_for_monitor(&primary);
            }
        }
        self.activity_mode = if self.settings.paused {
            ActivityMode::Paused
        } else {
            ActivityMode::Active
        };
        self.snooze_deadline = None;

        let mut window_attributes = Window::default_attributes()
            .with_title("downshift")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_min_inner_size(LogicalSize::new(self.settings.size, self.settings.size))
            .with_max_inner_size(LogicalSize::new(self.settings.size, self.settings.size))
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_inner_size(LogicalSize::new(self.settings.size, self.settings.size));

        #[cfg(target_os = "windows")]
        {
            // A transparent WebView2 child can retain the previous opaque
            // DWM redirection bitmap after the host window is resized. The
            // no-redirection path keeps the transparent surface current.
            window_attributes = window_attributes
                .with_no_redirection_bitmap(true)
                .with_skip_taskbar(true);
        }

        if let Some(position) = self.choose_initial_position(event_loop, self.settings.size) {
            window_attributes = window_attributes.with_position(position);
            self.settings.physical_x = Some(position.x);
            self.settings.physical_y = Some(position.y);
        }

        let window = match event_loop.create_window(window_attributes) {
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
        #[cfg(target_os = "macos")]
        configure_window_for_all_spaces(&window);
        self.settings.monitor = window.current_monitor().map(snapshot_monitor);

        let startup_monitor = window
            .current_monitor()
            .or_else(|| event_loop.primary_monitor())
            .or_else(|| event_loop.available_monitors().next());
        let size_presets = startup_monitor
            .as_ref()
            .map(size_presets_for_monitor)
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
        let webview_builder = WebViewBuilder::new()
            .with_html(breath_html())
            .with_transparent(true)
            .with_initialization_script(&init_script)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            });
        #[cfg(target_os = "windows")]
        let webview_result = webview_builder.build(&window);
        #[cfg(not(target_os = "windows"))]
        let webview_result = webview_builder.build_as_child(&window);

        let webview = match webview_result {
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
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            self.native_context_menu = NativeContextMenu::new(&self.settings);
        }
        self.sync_analytics_menu_state();
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
        self.enforce_fixed_square_size();
        self.sync_webview_bounds();
        if self.settings_load_error.is_none() {
            self.save_settings();
        }

        if let Some(proxy) = self.event_loop_proxy.as_ref().cloned() {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(UPDATE_CHECK_STARTUP_DELAY_SEC));
                loop {
                    let result = check_latest_release();
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
                self.enforce_fixed_square_size();
                self.update_position_from_physical(position);
                self.save_settings();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = self.window.as_ref() {
                    let current_monitor = window.current_monitor();
                    self.settings.monitor = current_monitor.clone().map(snapshot_monitor);
                    if let Some(monitor) = current_monitor {
                        self.apply_size_presets_for_monitor(&monitor);
                    }
                }
                self.save_settings();
            }
            WindowEvent::Resized(_) => {
                self.enforce_fixed_square_size();
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
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            AppEvent::MenuActivated(id) => self.handle_native_menu_activation(event_loop, &id),
        }
    }
}

fn snapshot_monitor(monitor: MonitorHandle) -> PersistedMonitor {
    let size = monitor.size();
    PersistedMonitor {
        width: size.width,
        height: size.height,
        scale_factor: monitor.scale_factor(),
    }
}

fn monitor_matches_persisted(monitor: &MonitorHandle, persisted: &PersistedMonitor) -> bool {
    let size = monitor.size();
    size.width == persisted.width
        && size.height == persisted.height
        && (monitor.scale_factor() - persisted.scale_factor).abs() < 0.01
}

fn position_fits_monitor(
    position: PhysicalPosition<i32>,
    size: f64,
    monitor: &MonitorHandle,
) -> bool {
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let window_size = physical_size_for_monitor(size, monitor);
    let max_x = i64::from(monitor_pos.x) + i64::from(monitor_size.width) - i64::from(window_size);
    let max_y = i64::from(monitor_pos.y) + i64::from(monitor_size.height) - i64::from(window_size);

    i64::from(position.x) >= i64::from(monitor_pos.x)
        && i64::from(position.y) >= i64::from(monitor_pos.y)
        && i64::from(position.x) <= max_x
        && i64::from(position.y) <= max_y
}

fn position_fits_monitor_legacy(
    position: LogicalPosition<f64>,
    size: f64,
    monitor: &MonitorHandle,
) -> bool {
    let scale = monitor.scale_factor();
    let monitor_pos = monitor.position().to_logical::<f64>(scale);
    let monitor_size = monitor.size().to_logical::<f64>(scale);
    let max_x = monitor_pos.x + monitor_size.width - size;
    let max_y = monitor_pos.y + monitor_size.height - size;
    position.x >= monitor_pos.x
        && position.y >= monitor_pos.y
        && position.x <= max_x
        && position.y <= max_y
}

fn default_corner_position(monitor: &MonitorHandle, size: f64) -> PhysicalPosition<i32> {
    let monitor_pos = monitor.position();
    let monitor_size = monitor.size();
    let margin = (f64::from(monitor_size.width.min(monitor_size.height))
        * DEFAULT_EDGE_MARGIN_RATIO)
        .round() as i32;
    let window_size = physical_size_for_monitor(size, monitor);
    PhysicalPosition::new(
        monitor_pos.x + monitor_size.width as i32 - window_size - margin,
        monitor_pos.y + margin,
    )
}

fn default_size_for_monitor(monitor: &MonitorHandle) -> f64 {
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let shorter_side = size.width.min(size.height);
    clamp_size(shorter_side * DEFAULT_SIZE_SHORT_SIDE_RATIO)
}

fn physical_size_for_monitor(size: f64, monitor: &MonitorHandle) -> i32 {
    (size * monitor.scale_factor()).round() as i32
}

fn logical_to_physical_position(
    position: LogicalPosition<f64>,
    scale_factor: f64,
) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        (position.x * scale_factor).round() as i32,
        (position.y * scale_factor).round() as i32,
    )
}

fn heartbeat_interval() -> Duration {
    let value = telemetry_heartbeat_interval_seconds().unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SEC);
    Duration::from_secs(value)
}

fn report_abnormal_exit<T: TelemetryClient>(
    telemetry: &T,
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

fn bootstrap_telemetry() -> (RuntimeTelemetryClient, bool) {
    let state = telemetry_state();
    let install_first_run = state.install_first_run;
    (RuntimeTelemetryClient::from_state(state), install_first_run)
}

fn parse_heartbeat_interval_seconds(raw: &str) -> u64 {
    raw.trim()
        .parse::<u64>()
        .ok()
        .map(|seconds| seconds.clamp(MIN_HEARTBEAT_INTERVAL_SEC, MAX_HEARTBEAT_INTERVAL_SEC))
        .unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SEC)
}

fn size_presets_for_monitor(monitor: &MonitorHandle) -> [f64; 4] {
    let scale = monitor.scale_factor();
    let logical = monitor.size().to_logical::<f64>(scale);
    let shorter_side = logical.width.min(logical.height);
    let mut presets = [0.0; 4];
    for (index, ratio) in SIZE_PRESET_RATIOS.iter().enumerate() {
        presets[index] = clamp_size((shorter_side * ratio).round());
    }
    presets
}

fn sync_child_webview_bounds(window: Option<&Window>, webview: Option<&WebView>, label: &str) {
    let (Some(window), Some(webview)) = (window, webview) else {
        return;
    };
    let size = window.inner_size().to_logical::<u32>(window.scale_factor());
    let bounds = Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: LogicalSize::new(size.width, size.height).into(),
    };
    if let Err(error) = webview.set_bounds(bounds) {
        log_stderr!("warning: failed to sync {label} bounds: {error}");
    }
}

fn focus_existing_child_window(window: Option<&Window>) -> bool {
    let Some(window) = window else {
        return false;
    };
    window.focus_window();
    true
}

fn clear_child_window(
    window: &mut Option<Window>,
    window_id: &mut Option<WindowId>,
    webview: &mut Option<WebView>,
) {
    *webview = None;
    *window = None;
    *window_id = None;
}

fn create_fixed_child_window(
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

#[cfg(unix)]
fn instance_socket_path_for_executable(executable: &Path) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    executable.hash(&mut hasher);
    let executable_hash = hasher.finish();
    let mut path = dirs::config_dir()?;
    path.push("downshift");
    path.push(format!("instance-{executable_hash:016x}.sock"));
    Some(path)
}

#[cfg(unix)]
fn instance_socket_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    instance_socket_path_for_executable(&executable)
}

#[cfg(unix)]
fn connect_to_existing_instance(path: &PathBuf, command: InstanceCommand) -> bool {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    stream.write_all(command.as_bytes()).is_ok()
}

#[cfg(unix)]
fn spawn_instance_server(path: PathBuf, proxy: EventLoopProxy<AppEvent>) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        match connect_to_existing_instance(&path, InstanceCommand::Activate) {
            true => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "instance already running",
                ))
            }
            false => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let listener = UnixListener::bind(&path)?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = String::new();
            if stream.read_to_string(&mut buffer).is_err() {
                continue;
            }
            if matches!(
                InstanceCommand::parse(&buffer),
                Some(InstanceCommand::Activate)
            ) {
                let _ = proxy.send_event(AppEvent::InstanceActivate);
            }
        }
    });
    Ok(())
}

#[cfg(target_os = "windows")]
struct WindowsInstanceGuard {
    mutex: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl Drop for WindowsInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_instance_pipe_name() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    executable.hash(&mut hasher);
    Some(format!(r"\\.\pipe\downshift-{:#016x}", hasher.finish()))
}

#[cfg(target_os = "windows")]
fn windows_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn connect_to_existing_windows_instance(pipe_name: &str, command: InstanceCommand) -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_PIPE_BUSY, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, WriteFile, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

    let pipe_name = windows_wide(pipe_name);
    for _ in 0..20 {
        let handle = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
            let bytes = command.as_bytes();
            let mut written = 0u32;
            let result = unsafe {
                WriteFile(
                    handle,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    &mut written,
                    std::ptr::null_mut(),
                )
            };
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
            }
            return result != 0 && written == bytes.len() as u32;
        }

        if unsafe { GetLastError() } == ERROR_PIPE_BUSY {
            let _ = unsafe { WaitNamedPipeW(pipe_name.as_ptr(), 100) };
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[cfg(target_os = "windows")]
fn spawn_windows_instance_server(
    pipe_name: String,
    proxy: EventLoopProxy<AppEvent>,
) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{
        GetLastError, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
        PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    let pipe_name_wide = windows_wide(&pipe_name);
    std::thread::Builder::new()
        .name("downshift-instance-server".to_string())
        .spawn(move || loop {
            let pipe = unsafe {
                CreateNamedPipeW(
                    pipe_name_wide.as_ptr(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    128,
                    128,
                    0,
                    std::ptr::null(),
                )
            };
            if pipe.is_null() || pipe == INVALID_HANDLE_VALUE {
                log_stderr!("warning: failed to create Windows instance pipe");
                return;
            }

            let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if connected {
                let mut buffer = [0u8; 128];
                let mut read = 0u32;
                let result = unsafe {
                    ReadFile(
                        pipe,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                if result != 0 {
                    let command = String::from_utf8_lossy(&buffer[..read as usize]);
                    if matches!(
                        InstanceCommand::parse(&command),
                        Some(InstanceCommand::Activate)
                    ) && proxy.send_event(AppEvent::InstanceActivate).is_err()
                    {
                        unsafe {
                            let _ = windows_sys::Win32::Foundation::CloseHandle(pipe);
                        }
                        return;
                    }
                }
            }
            unsafe {
                let _ = DisconnectNamedPipe(pipe);
                let _ = windows_sys::Win32::Foundation::CloseHandle(pipe);
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn start_windows_instance(
    proxy: EventLoopProxy<AppEvent>,
) -> Result<Option<WindowsInstanceGuard>, String> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let pipe_name = windows_instance_pipe_name().ok_or_else(|| {
        "failed to resolve executable path for Windows single-instance guard".to_string()
    })?;
    let mutex_name = format!("Local\\downshift-{:016x}", {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pipe_name.hash(&mut hasher);
        hasher.finish()
    });
    let mutex_name_wide = windows_wide(&mutex_name);
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name_wide.as_ptr()) };
    if mutex.is_null() {
        return Err("CreateMutexW returned a null handle".to_string());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(mutex);
        }
        let _ = connect_to_existing_windows_instance(&pipe_name, InstanceCommand::Activate);
        return Ok(None);
    }

    spawn_windows_instance_server(pipe_name, proxy)?;
    Ok(Some(WindowsInstanceGuard { mutex }))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn snooze_minutes_for_menu_id(id: &str) -> Option<u64> {
    match id {
        MENU_ID_SNOOZE_5 => Some(SNOOZE_PRESET_MINUTES[0]),
        MENU_ID_SNOOZE_10 => Some(SNOOZE_PRESET_MINUTES[1]),
        MENU_ID_SNOOZE_15 => Some(SNOOZE_PRESET_MINUTES[2]),
        MENU_ID_SNOOZE_30 => Some(SNOOZE_PRESET_MINUTES[3]),
        MENU_ID_SNOOZE_60 => Some(SNOOZE_PRESET_MINUTES[4]),
        _ => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn size_slot_for_menu_id(id: &str) -> Option<usize> {
    match id {
        MENU_ID_SIZE_S => Some(0),
        MENU_ID_SIZE_M => Some(1),
        MENU_ID_SIZE_L => Some(2),
        MENU_ID_SIZE_XL => Some(3),
        _ => None,
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

    let (telemetry, telemetry_install_first_run) = bootstrap_telemetry();
    let startup_telemetry = telemetry.clone();
    let panic_telemetry_for_hook = startup_telemetry.clone();
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
    #[cfg(target_os = "macos")]
    event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
    let event_loop = match event_loop_builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log_stderr!("error: failed to create event loop: {error}");
            return report_abnormal_exit(
                &startup_telemetry,
                SessionEndReason::StartupFailure,
                "event_loop_build",
            );
        }
    };
    let event_loop_proxy = event_loop.create_proxy();

    #[cfg(unix)]
    if let Some(path) = instance_socket_path() {
        if connect_to_existing_instance(&path, InstanceCommand::Activate) {
            return std::process::ExitCode::SUCCESS;
        }
        if let Err(error) = spawn_instance_server(path, event_loop_proxy.clone()) {
            if error.kind() == std::io::ErrorKind::AddrInUse {
                return std::process::ExitCode::SUCCESS;
            }
            log_stderr!("warning: failed to start instance server: {error}");
        }
    }

    #[cfg(target_os = "windows")]
    let _windows_instance_guard = match start_windows_instance(event_loop_proxy.clone()) {
        Ok(Some(guard)) => Some(guard),
        Ok(None) => return std::process::ExitCode::SUCCESS,
        Err(error) => {
            log_stderr!("warning: failed to start Windows instance guard: {error}");
            None
        }
    };

    let ctrlc_proxy = event_loop_proxy.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        let _ = ctrlc_proxy.send_event(AppEvent::ExitRequested);
    }) {
        log_stderr!("warning: failed to install ctrl-c handler: {error}");
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let menu_proxy = event_loop_proxy.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = menu_proxy.send_event(AppEvent::MenuActivated(event.id().as_ref().to_string()));
        }));
    }
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

    let mut app = App::new(telemetry, telemetry_install_first_run);
    app.event_loop_proxy = Some(event_loop_proxy);

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
    #[serial]
    fn telemetry_bootstrap_preserves_first_run_boundary() {
        let root = telemetry_test_dir("bootstrap");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let (first_client, first_run) = bootstrap_telemetry();
        let app = App::new(first_client, first_run);
        assert!(app.telemetry_install_first_run);

        let (second_client, second_run) = bootstrap_telemetry();
        assert!(!second_run);

        app.telemetry.shutdown(Duration::from_millis(200));
        second_client.shutdown(Duration::from_millis(200));
        std::fs::remove_dir_all(root).ok();
        std::env::remove_var("DOWNSHIFT_TELEMETRY_DIR");
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

        assert_eq!(
            download_release_url().expect("download release url"),
            expected_download_release_url()
        );
        assert_eq!(
            github_issues_url().expect("github issues url"),
            expected_github_issues_url()
        );
        assert_eq!(
            support_email_address().expect("support email"),
            expected_support_email()
        );
    }

    #[test]
    #[serial]
    fn runtime_prod_env_uses_compiled_defaults_instead_of_failing() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "prod");

        assert_eq!(
            download_release_url().expect("download release url"),
            expected_download_release_url()
        );
        assert_eq!(
            github_issues_url().expect("github issues url"),
            expected_github_issues_url()
        );
        assert_eq!(
            support_email_address().expect("support email"),
            expected_support_email()
        );
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

        assert_eq!(
            download_release_url().expect("download release url"),
            expected_download_release_url()
        );
        assert_eq!(
            github_issues_url().expect("github issues url"),
            "https://example.com/issues"
        );
        assert_eq!(
            support_email_address().expect("support email"),
            "support@example.com"
        );
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
        assert!(state.ignore_current_update_enabled());
        assert!(state.is_ignoring_current_update());
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
    fn instance_command_round_trips() {
        assert_eq!(
            InstanceCommand::parse("activate\n"),
            Some(InstanceCommand::Activate)
        );
        assert_eq!(InstanceCommand::Activate.as_bytes(), b"activate\n");
        assert_eq!(InstanceCommand::parse("nope"), None);
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
