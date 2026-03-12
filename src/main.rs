use downshift::telemetry::{
    menu_action_size_target, telemetry_state, ActivityState, ActivityTrigger, EventName, MenuAction,
    RuntimeTelemetryClient, SessionEndReason, SizeTarget, TelemetryClient,
};
use downshift::{
    apply_resize_step, built_in_breathing_preset, built_in_breathing_presets, clamp_size,
    diagnostics, load_settings_result, BreathingPattern, IpcCommand, PersistedMonitor,
    SavedBreathingPreset, Settings, BREATHING_PRESET_ID_COHERENT, BREATHING_PRESET_ID_CUSTOM,
    DEFAULT_SIZE,
};
#[cfg(target_os = "macos")]
use downshift::{launch_agent_path_from_home, launch_agent_plist};
#[cfg(target_os = "macos")]
use muda::dpi::PhysicalPosition as MenuPhysicalPosition;
#[cfg(target_os = "macos")]
use muda::{
    CheckMenuItem, ContextMenu, IsMenuItem, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use semver::Version;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
#[cfg(target_os = "macos")]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};
use wry::{Rect, WebView, WebViewBuilder};

const DEFAULT_SIZE_SHORT_SIDE_RATIO: f64 = 0.10;
const DEFAULT_EDGE_MARGIN_RATIO: f64 = 0.05;
const SIZE_PRESET_RATIOS: [f64; 4] = [0.08, 0.10, 0.13, 0.16];
const DEFAULT_HEARTBEAT_INTERVAL_SEC: u64 = 60;
const MIN_HEARTBEAT_INTERVAL_SEC: u64 = 5;
const MAX_HEARTBEAT_INTERVAL_SEC: u64 = 3600;
const UPDATE_CHECK_STARTUP_DELAY_SEC: u64 = 8;
const UPDATE_CHECK_BACKGROUND_INTERVAL_SEC: u64 = 6 * 60 * 60;
const UPDATE_RELEASE_API_URL: &str =
    "https://api.github.com/repos/samm81/downshift/releases/latest";
const UPDATE_DOWNLOAD_FALLBACK_URL: &str = "https://github.com/samm81/downshift/releases/latest";
const DEFAULT_GITHUB_ISSUES_URL: &str = "github-issues-url-not-set";
const DEFAULT_SUPPORT_EMAIL: &str = "email-not-set";
const UPDATE_TOOLTIP: &str = "new version available";
const SNOOZE_PRESET_MINUTES: [u64; 5] = [5, 10, 15, 30, 60];
const COMPILED_DOWNSHIFT_ENV: Option<&str> = option_env!("DOWNSHIFT_ENV");
const COMPILED_TELEMETRY_ENABLED: Option<&str> = option_env!("DOWNSHIFT_TELEMETRY_ENABLED");
const COMPILED_BETTERSTACK_LOGS_TOKEN: Option<&str> =
    option_env!("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN");
const COMPILED_BETTERSTACK_LOGS_HOST: Option<&str> = option_env!("DOWNSHIFT_BETTERSTACK_LOGS_HOST");
const COMPILED_BETTERSTACK_ERRORS_DSN: Option<&str> =
    option_env!("DOWNSHIFT_BETTERSTACK_ERRORS_DSN");
const COMPILED_BUILD_CHANNEL: Option<&str> = option_env!("DOWNSHIFT_BUILD_CHANNEL");
const COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC: Option<&str> =
    option_env!("DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC");
const COMPILED_DOWNLOAD_RELEASE_URL: Option<&str> = option_env!("DOWNSHIFT_DOWNLOAD_RELEASE_URL");
const COMPILED_GITHUB_ISSUES_URL: Option<&str> = option_env!("DOWNSHIFT_GITHUB_ISSUES_URL");
const COMPILED_SUPPORT_EMAIL: Option<&str> = option_env!("DOWNSHIFT_SUPPORT_EMAIL");
#[cfg(target_os = "macos")]
const MENU_ID_PAUSE: &str = "pause";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_ROOT: &str = "snooze_root";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_5: &str = "snooze_5";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_10: &str = "snooze_10";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_15: &str = "snooze_15";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_30: &str = "snooze_30";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_60: &str = "snooze_60";
#[cfg(target_os = "macos")]
const MENU_ID_SNOOZE_CUSTOM: &str = "snooze_custom";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_S: &str = "size_s";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_M: &str = "size_m";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_L: &str = "size_l";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_XL: &str = "size_xl";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_PATTERN: &str = "breathing_pattern";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_COHERENT: &str = "breathing_coherent";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_BOX: &str = "breathing_box";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_479: &str = "breathing_479";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_EDIT: &str = "breathing_edit";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_DELETE_ROOT: &str = "breathing_delete_root";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_DELETE_PREFIX: &str = "breathing_delete:";
#[cfg(target_os = "macos")]
const MENU_ID_BREATHING_SAVED_PREFIX: &str = "breathing_saved:";
#[cfg(target_os = "macos")]
const MENU_ID_RESET: &str = "reset";
#[cfg(target_os = "macos")]
const MENU_ID_QUIT: &str = "quit";
#[cfg(target_os = "macos")]
const MENU_ID_ANALYTICS_ROOT: &str = "analytics_root";
#[cfg(target_os = "macos")]
const MENU_ID_USAGE_ON: &str = "usage_on";
#[cfg(target_os = "macos")]
const MENU_ID_USAGE_OFF: &str = "usage_off";
#[cfg(target_os = "macos")]
const MENU_ID_CRASH_ON: &str = "crash_on";
#[cfg(target_os = "macos")]
const MENU_ID_CRASH_OFF: &str = "crash_off";
#[cfg(target_os = "macos")]
const MENU_ID_ANALYTICS_INFO: &str = "analytics_info";
#[cfg(target_os = "macos")]
const MENU_ID_UPDATE_PRIMARY: &str = "update_primary";
#[cfg(target_os = "macos")]
const MENU_ID_LAUNCH_AT_LOGIN: &str = "launch_at_login";
#[cfg(target_os = "macos")]
const MENU_ID_BUGS_ROOT: &str = "bugs_root";
#[cfg(target_os = "macos")]
const MENU_ID_COPY_DIAGNOSTICS: &str = "copy_diagnostics";
#[cfg(target_os = "macos")]
const MENU_ID_FILE_BUG_GITHUB: &str = "file_bug_github";
#[cfg(target_os = "macos")]
const MENU_ID_FILE_BUG_EMAIL: &str = "file_bug_email";

macro_rules! log_stderr {
    ($($arg:tt)*) => {{
        let message = format!($($arg)*);
        eprintln!("{message}");
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

const BREATH_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light;
      }
      html,
      body {
        width: 100%;
        height: 100%;
        margin: 0;
        background: transparent;
        overflow: hidden;
      }
      body {
        display: grid;
        place-items: center;
        user-select: none;
        cursor: default;
      }
      .ball {
        position: relative;
        width: 100%;
        aspect-ratio: 1 / 1;
        border-radius: 9999px;
        background: rgba(124, 182, 255, 0.52);
        box-shadow: inset 0 0 0 1px rgba(124, 182, 255, 0.35);
        transform: scale(0.8);
        transform-origin: center;
      }
      .ball.paused {
        opacity: 0.55;
      }
      .badge {
        position: fixed;
        width: 16px;
        height: 16px;
        border-radius: 9999px;
        border: 0;
        background: rgba(148, 204, 255, 0.9);
        box-shadow: inset 0 0 0 1px rgba(120, 183, 244, 0.95);
        color: rgba(30, 75, 120, 0.92);
        font: 700 10px/1 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        display: grid;
        place-items: center;
        padding: 0;
        cursor: default;
        transform-origin: center;
        z-index: 10000;
      }
      .badge[hidden] {
        display: none;
      }
      .badge.is-appearing {
        animation: badge-boing 420ms cubic-bezier(0.2, 1.1, 0.25, 1);
      }
      .badge.is-dismissing {
        animation: badge-dismiss 240ms cubic-bezier(0.3, 0.9, 0.4, 1) forwards;
      }
      .menu {
        position: fixed;
        min-width: 176px;
        border-radius: 10px;
        background: rgba(24, 28, 35, 0.94);
        color: rgba(245, 248, 255, 0.95);
        box-shadow: 0 10px 24px rgba(0, 0, 0, 0.28);
        padding: 6px;
        border: 1px solid rgba(255, 255, 255, 0.09);
        font: 12px/1.3 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
        z-index: 9999;
      }
      .menu[hidden] {
        display: none;
      }
      .menu button {
        width: 100%;
        text-align: left;
        border: 0;
        border-radius: 6px;
        margin: 0;
        padding: 6px 8px;
        background: transparent;
        color: inherit;
      }
      .menu button:hover {
        background: rgba(124, 182, 255, 0.25);
      }
      .menu .divider {
        border-top: 1px solid rgba(255, 255, 255, 0.12);
        margin: 6px 0;
      }
      .menu .group {
        margin: 4px 0;
      }
      .menu .label {
        opacity: 0.75;
        padding: 2px 8px 4px;
        font-size: 11px;
      }
      @keyframes badge-boing {
        0% {
          transform: scale(0.6);
        }
        65% {
          transform: scale(1.18);
        }
        100% {
          transform: scale(1);
        }
      }
      @keyframes badge-dismiss {
        0% {
          transform: scale(1);
          opacity: 1;
        }
        100% {
          transform: scale(0.35) rotate(-12deg);
          opacity: 0;
        }
      }
    </style>
  </head>
  <body>
    <div class="ball" id="ball"></div>
    <button class="badge" id="update-badge" title="new version available" hidden>↓</button>
    <div class="menu" id="menu" hidden>
      <button id="menu-pause">pause</button>
      <div class="divider"></div>
      <div class="group">
        <div class="label">snooze</div>
        <button data-snooze-minutes="5">snooze for 5 minutes</button>
        <button data-snooze-minutes="10">snooze for 10 minutes</button>
        <button data-snooze-minutes="15">snooze for 15 minutes</button>
        <button data-snooze-minutes="30">snooze for 30 minutes</button>
        <button data-snooze-minutes="60">snooze for 60 minutes</button>
        <button id="menu-snooze-custom">snooze for custom minutes…</button>
      </div>
      <div class="divider"></div>
      <div class="group">
        <div class="label">size</div>
        <button data-size-slot="0">S</button>
        <button data-size-slot="1">M</button>
        <button data-size-slot="2">L</button>
        <button data-size-slot="3">XL</button>
      </div>
      <div class="divider"></div>
      <button id="menu-breathing-pattern">breathing pattern</button>
      <div class="group" id="breathing-submenu" hidden>
        <div id="breathing-preset-list"></div>
        <div class="divider"></div>
        <button id="menu-breathing-edit">add new…</button>
        <button id="menu-breathing-delete">delete</button>
        <div class="group" id="breathing-delete-submenu" hidden>
          <div id="breathing-delete-list"></div>
        </div>
      </div>
      <div class="divider"></div>
      <button id="menu-reset">reset</button>
      <button id="menu-quit">quit</button>
      <div class="divider"></div>
      <button id="menu-update-primary">check for updates</button>
      <div class="divider"></div>
      <button id="menu-analytics-toggle">help improve downshift</button>
      <div class="group" id="analytics-submenu" hidden>
        <button id="menu-usage-on">share anonymous usage data</button>
        <button id="menu-usage-off">don’t share usage data</button>
        <div class="divider"></div>
        <button id="menu-crash-on">share anonymous crash reports</button>
        <button id="menu-crash-off">don't share crash reports</button>
        <div class="divider"></div>
        <button id="menu-what-we-collect">what we collect…</button>
      </div>
    </div>
    <script>
      (() => {
        const ball = document.getElementById("ball");
        const menu = document.getElementById("menu");
        const pauseButton = document.getElementById("menu-pause");
        const resetButton = document.getElementById("menu-reset");
        const quitButton = document.getElementById("menu-quit");
        const updatePrimaryButton = document.getElementById("menu-update-primary");
        const updateBadge = document.getElementById("update-badge");
        const customSnoozeButton = document.getElementById("menu-snooze-custom");
        const analyticsToggleButton = document.getElementById("menu-analytics-toggle");
        const analyticsSubmenu = document.getElementById("analytics-submenu");
        const breathingPatternButton = document.getElementById("menu-breathing-pattern");
        const breathingSubmenu = document.getElementById("breathing-submenu");
        const breathingPresetList = document.getElementById("breathing-preset-list");
        const breathingEditButton = document.getElementById("menu-breathing-edit");
        const breathingDeleteButton = document.getElementById("menu-breathing-delete");
        const breathingDeleteSubmenu = document.getElementById("breathing-delete-submenu");
        const breathingDeleteList = document.getElementById("breathing-delete-list");
        const usageOnButton = document.getElementById("menu-usage-on");
        const usageOffButton = document.getElementById("menu-usage-off");
        const crashOnButton = document.getElementById("menu-crash-on");
        const crashOffButton = document.getElementById("menu-crash-off");
        const whatWeCollectButton = document.getElementById("menu-what-we-collect");
        const sizeButtons = Array.from(document.querySelectorAll("[data-size-slot]"));
        const snoozeButtons = Array.from(document.querySelectorAll("[data-snooze-minutes]"));
        const init = window.__BB_INIT__ || { paused: false, use_native_menu: false };
        const useNativeMenu = Boolean(init.use_native_menu);
        let breathAnimation = null;

        function normalizePattern(pattern) {
          const fallback = {
            expanding_seconds: 5.5,
            expanded_hold_seconds: 0,
            compressing_seconds: 5.5,
            compressed_hold_seconds: 0,
          };
          const candidate = pattern && typeof pattern === "object" ? pattern : fallback;
          const next = {
            expanding_seconds: Number(candidate.expanding_seconds),
            expanded_hold_seconds: Number(candidate.expanded_hold_seconds),
            compressing_seconds: Number(candidate.compressing_seconds),
            compressed_hold_seconds: Number(candidate.compressed_hold_seconds),
          };
          if (!Number.isFinite(next.expanding_seconds) || next.expanding_seconds <= 0) {
            next.expanding_seconds = fallback.expanding_seconds;
          }
          if (!Number.isFinite(next.expanded_hold_seconds) || next.expanded_hold_seconds < 0) {
            next.expanded_hold_seconds = fallback.expanded_hold_seconds;
          }
          if (!Number.isFinite(next.compressing_seconds) || next.compressing_seconds <= 0) {
            next.compressing_seconds = fallback.compressing_seconds;
          }
          if (!Number.isFinite(next.compressed_hold_seconds) || next.compressed_hold_seconds < 0) {
            next.compressed_hold_seconds = fallback.compressed_hold_seconds;
          }
          return next;
        }

        function totalPatternSeconds(pattern) {
          return pattern.expanding_seconds
            + pattern.expanded_hold_seconds
            + pattern.compressing_seconds
            + pattern.compressed_hold_seconds;
        }

        function keyframesForPattern(pattern) {
          const total = Math.max(totalPatternSeconds(pattern), 0.1);
          const expandEnd = pattern.expanding_seconds / total;
          const topHoldEnd = (pattern.expanding_seconds + pattern.expanded_hold_seconds) / total;
          const compressEnd = (pattern.expanding_seconds + pattern.expanded_hold_seconds + pattern.compressing_seconds) / total;
          return [
            { transform: "scale(0.8)", offset: 0 },
            { transform: "scale(1)", offset: expandEnd },
            { transform: "scale(1)", offset: topHoldEnd },
            { transform: "scale(0.8)", offset: compressEnd },
            { transform: "scale(0.8)", offset: 1 },
          ];
        }

        const state = {
          paused: Boolean(init.paused),
          breathingPattern: normalizePattern(init.breathing_pattern),
          activeBreathingPresetId: String(init.active_breathing_preset_id || "coherent_breathing"),
          breathingPresets: Array.isArray(init.breathing_presets) ? init.breathing_presets : [],
          usageDataSharing: Object.prototype.hasOwnProperty.call(init, "usage_data_sharing") ? Boolean(init.usage_data_sharing) : true,
          crashReportsSharing: Object.prototype.hasOwnProperty.call(init, "crash_reports_sharing") ? Boolean(init.crash_reports_sharing) : true,
          analyticsOpen: false,
          breathingOpen: false,
          updateLabel: String(init.update_menu_label || "check for updates"),
          updateHasNewVersion: Boolean(init.update_has_new_version),
          updateShowBadge: Boolean(init.update_show_badge),
          sizePresets: Array.isArray(init.size_presets) && init.size_presets.length === 4
            ? init.size_presets.map((value) => Number(value)).filter((value) => Number.isFinite(value) && value > 0)
            : [64, 96, 128, 160],
        };
        updateBadge.title = String(init.update_tooltip || "new version available");
        if (state.sizePresets.length !== 4) {
          state.sizePresets = [64, 96, 128, 160];
        }

        function post(payload) {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify(payload));
          }
        }

        function hideMenu() {
          menu.hidden = true;
          analyticsSubmenu.hidden = true;
          breathingSubmenu.hidden = true;
          breathingDeleteSubmenu.hidden = true;
          state.analyticsOpen = false;
          state.breathingOpen = false;
        }

        function showMenu(x, y) {
          menu.style.left = `${x}px`;
          menu.style.top = `${y}px`;
          menu.hidden = false;
        }

        function syncAnimationPauseState() {
          if (!breathAnimation) return;
          if (state.paused) {
            breathAnimation.pause();
          } else {
            breathAnimation.play();
          }
        }

        function restartBreathingAnimation() {
          if (breathAnimation) {
            breathAnimation.cancel();
          }
          breathAnimation = ball.animate(keyframesForPattern(state.breathingPattern), {
            duration: Math.round(totalPatternSeconds(state.breathingPattern) * 1000),
            iterations: Infinity,
            easing: "linear",
          });
          syncAnimationPauseState();
        }

        function applyBallState() {
          ball.classList.toggle("paused", state.paused);
          pauseButton.textContent = state.paused ? "resume" : "pause";
          updatePrimaryButton.textContent = state.updateLabel;
          updatePrimaryButton.dataset.newVersion = state.updateHasNewVersion ? "1" : "0";
          syncAnimationPauseState();
          positionBadge();
        }

        function applyAnalyticsButtons() {
          usageOnButton.textContent = `share anonymous usage data ${state.usageDataSharing ? "✓" : ""}`.trim();
          usageOffButton.textContent = `don’t share usage data ${!state.usageDataSharing ? "✓" : ""}`.trim();
          crashOnButton.textContent = `share anonymous crash reports ${state.crashReportsSharing ? "✓" : ""}`.trim();
          crashOffButton.textContent = `don't share crash reports ${!state.crashReportsSharing ? "✓" : ""}`.trim();
          analyticsToggleButton.textContent = "help improve downshift";
        }

        function breathingSummary(pattern) {
          return `${pattern.expanding_seconds} / ${pattern.expanded_hold_seconds} / ${pattern.compressing_seconds} / ${pattern.compressed_hold_seconds}`;
        }

        function applyBreathingButtons() {
          const activeId = state.activeBreathingPresetId;
          breathingPatternButton.textContent = "breathing pattern";
          breathingPresetList.textContent = "";
          breathingDeleteList.textContent = "";
          state.breathingPresets.forEach((preset) => {
            const button = document.createElement("button");
            button.dataset.breathingPreset = preset.id;
            const isActive = preset.id === activeId;
            button.textContent = `${preset.name}${isActive ? " ✓" : ""}`;
            button.addEventListener("click", () => {
              post({ cmd: "apply_breathing_pattern", preset_id: preset.id, pattern: state.breathingPattern });
              hideMenu();
            });
            breathingPresetList.appendChild(button);

            const deleteButton = document.createElement("button");
            deleteButton.textContent = preset.name;
            deleteButton.addEventListener("click", () => {
              post({ cmd: "delete_breathing_preset", preset_id: preset.id });
              hideMenu();
            });
            breathingDeleteList.appendChild(deleteButton);
          });
          breathingDeleteButton.disabled = state.breathingPresets.length === 0;
        }

        function applySizePresetButtons() {
          const labels = ["S", "M", "L", "XL"];
          sizeButtons.forEach((button) => {
            const rawIndex = Number(button.dataset.sizeSlot);
            const index = Number.isFinite(rawIndex) ? rawIndex : -1;
            const value = state.sizePresets[index];
            if (!Number.isFinite(value) || value <= 0) return;
            const rounded = Math.round(value);
            button.dataset.size = String(rounded);
            button.textContent = `${labels[index] || "size"} (${rounded}px)`;
          });
        }

        function positionBadge() {
          if (updateBadge.hidden) return;
          const rect = ball.getBoundingClientRect();
          const badgeSize = 16;
          const inset = 1;
          const x = Math.round(rect.right - badgeSize - inset);
          const y = Math.round(rect.top + inset);
          updateBadge.style.left = `${x}px`;
          updateBadge.style.top = `${y}px`;
        }

        function dismissBadge(withAnimation) {
          if (updateBadge.hidden) return;
          if (withAnimation) {
            updateBadge.classList.remove("is-appearing");
            updateBadge.classList.add("is-dismissing");
            window.setTimeout(() => {
              updateBadge.classList.remove("is-dismissing");
              updateBadge.hidden = true;
            }, 240);
          } else {
            updateBadge.classList.remove("is-dismissing");
            updateBadge.hidden = true;
          }
        }

        function applyUpdateBadge(animateIn) {
          if (!state.updateShowBadge) {
            dismissBadge(false);
            return;
          }
          updateBadge.hidden = false;
          positionBadge();
          updateBadge.classList.remove("is-dismissing");
          if (animateIn) {
            updateBadge.classList.remove("is-appearing");
            void updateBadge.offsetWidth;
            updateBadge.classList.add("is-appearing");
            window.setTimeout(() => {
              updateBadge.classList.remove("is-appearing");
            }, 420);
          }
        }

        window.breathBallApplyState = function(next) {
          if (Object.prototype.hasOwnProperty.call(next, "paused")) {
            state.paused = Boolean(next.paused);
          }
          if (Object.prototype.hasOwnProperty.call(next, "breathing_pattern")) {
            state.breathingPattern = normalizePattern(next.breathing_pattern);
            restartBreathingAnimation();
          }
          if (Object.prototype.hasOwnProperty.call(next, "active_breathing_preset_id")) {
            state.activeBreathingPresetId = String(next.active_breathing_preset_id || "custom");
          }
          if (Object.prototype.hasOwnProperty.call(next, "breathing_presets")) {
            state.breathingPresets = Array.isArray(next.breathing_presets) ? next.breathing_presets : [];
          }
          if (Object.prototype.hasOwnProperty.call(next, "size_presets")) {
            const values = Array.isArray(next.size_presets)
              ? next.size_presets.map((value) => Number(value)).filter((value) => Number.isFinite(value) && value > 0)
              : [];
            if (values.length === 4) {
              state.sizePresets = values;
              applySizePresetButtons();
            }
          }
          if (Object.prototype.hasOwnProperty.call(next, "usage_data_sharing")) {
            state.usageDataSharing = Boolean(next.usage_data_sharing);
          }
          if (Object.prototype.hasOwnProperty.call(next, "crash_reports_sharing")) {
            state.crashReportsSharing = Boolean(next.crash_reports_sharing);
          }
          let animateBadge = false;
          if (Object.prototype.hasOwnProperty.call(next, "update_menu_label")) {
            state.updateLabel = String(next.update_menu_label || "check for updates");
          }
          if (Object.prototype.hasOwnProperty.call(next, "update_has_new_version")) {
            state.updateHasNewVersion = Boolean(next.update_has_new_version);
          }
          if (Object.prototype.hasOwnProperty.call(next, "update_show_badge")) {
            const previous = state.updateShowBadge;
            state.updateShowBadge = Boolean(next.update_show_badge);
            animateBadge = !previous && state.updateShowBadge;
            if (previous && !state.updateShowBadge) {
              dismissBadge(true);
            }
          }
          applyBallState();
          applyAnalyticsButtons();
          applyBreathingButtons();
          applyUpdateBadge(animateBadge);
        };

        ball.addEventListener("wheel", (event) => {
          event.preventDefault();
          const direction = event.deltaY < 0 ? 1 : -1;
          post({ cmd: "resize", delta: direction, fine: event.shiftKey });
        }, { passive: false });

        ball.addEventListener("contextmenu", (event) => {
          event.preventDefault();
          if (useNativeMenu) {
            post({ cmd: "show_context_menu", x: Math.round(event.clientX), y: Math.round(event.clientY) });
            return;
          }
          post({ cmd: "analytics_menu_opened" });
          applyBallState();
          applyAnalyticsButtons();
          showMenu(event.clientX, event.clientY);
        });

        pauseButton.addEventListener("click", () => {
          state.paused = !state.paused;
          applyBallState();
          post({ cmd: "set_paused", paused: state.paused });
          hideMenu();
        });

        sizeButtons.forEach((button) => {
          button.addEventListener("click", () => {
            const size = Number(button.dataset.size);
            if (!Number.isFinite(size) || size <= 0) return;
            post({ cmd: "set_size", size });
            hideMenu();
          });
        });

        snoozeButtons.forEach((button) => {
          button.addEventListener("click", () => {
            const minutes = Number(button.dataset.snoozeMinutes);
            if (!Number.isFinite(minutes) || minutes <= 0) return;
            post({ cmd: "set_snooze", minutes: Math.round(minutes) });
            hideMenu();
          });
        });

        customSnoozeButton.addEventListener("click", () => {
          post({ cmd: "show_custom_snooze" });
          hideMenu();
        });

        resetButton.addEventListener("click", () => {
          state.paused = false;
          applyBallState();
          post({ cmd: "reset" });
          hideMenu();
        });

        breathingPatternButton.addEventListener("click", () => {
          state.breathingOpen = !state.breathingOpen;
          breathingSubmenu.hidden = !state.breathingOpen;
        });

        breathingEditButton.addEventListener("click", () => {
          post({ cmd: "show_breathing_pattern" });
          hideMenu();
        });

        breathingDeleteButton.addEventListener("click", () => {
          breathingDeleteSubmenu.hidden = !breathingDeleteSubmenu.hidden;
        });

        quitButton.addEventListener("click", () => {
          post({ cmd: "quit" });
          hideMenu();
        });

        updatePrimaryButton.addEventListener("click", () => {
          post({ cmd: "update_primary_action" });
          hideMenu();
        });

        updateBadge.addEventListener("click", (event) => {
          event.preventDefault();
          event.stopPropagation();
          state.updateShowBadge = false;
          dismissBadge(true);
          post({ cmd: "dismiss_update_badge" });
          if (useNativeMenu) {
            post({ cmd: "show_context_menu", x: Math.round(event.clientX), y: Math.round(event.clientY) });
          } else {
            showMenu(event.clientX, event.clientY);
          }
        });

        analyticsToggleButton.addEventListener("click", () => {
          state.analyticsOpen = !state.analyticsOpen;
          analyticsSubmenu.hidden = !state.analyticsOpen;
          applyAnalyticsButtons();
          if (state.analyticsOpen) {
            post({ cmd: "analytics_menu_opened" });
          }
        });

        usageOnButton.addEventListener("click", () => {
          state.usageDataSharing = true;
          applyAnalyticsButtons();
          post({ cmd: "set_usage_data_sharing", enabled: true });
        });

        usageOffButton.addEventListener("click", () => {
          state.usageDataSharing = false;
          applyAnalyticsButtons();
          post({ cmd: "set_usage_data_sharing", enabled: false });
        });

        crashOnButton.addEventListener("click", () => {
          state.crashReportsSharing = true;
          applyAnalyticsButtons();
          post({ cmd: "set_crash_reports_sharing", enabled: true });
        });

        crashOffButton.addEventListener("click", () => {
          state.crashReportsSharing = false;
          applyAnalyticsButtons();
          post({ cmd: "set_crash_reports_sharing", enabled: false });
        });

        whatWeCollectButton.addEventListener("click", () => {
          post({ cmd: "show_telemetry_info" });
        });

        document.addEventListener("mousedown", (event) => {
          if (!menu.hidden && !menu.contains(event.target)) {
            hideMenu();
          }
        });

        document.addEventListener("blur", hideMenu);
        window.addEventListener("resize", () => {
          hideMenu();
          positionBadge();
        });

        const drag = {
          active: false,
          pointerId: null,
        };

        ball.addEventListener("pointerdown", (event) => {
          if (event.button !== 0) return;
          drag.active = true;
          drag.pointerId = event.pointerId;
          if (typeof ball.setPointerCapture === "function") {
            ball.setPointerCapture(event.pointerId);
          }
          post({
            cmd: "start_drag",
            screen_x: Math.round(event.screenX),
            screen_y: Math.round(event.screenY),
          });
        });

        ball.addEventListener("pointermove", (event) => {
          if (!drag.active || event.pointerId !== drag.pointerId) return;
          post({
            cmd: "drag_to",
            screen_x: Math.round(event.screenX),
            screen_y: Math.round(event.screenY),
          });
        });

        function endDrag(event) {
          if (!drag.active) return;
          if (event && drag.pointerId !== null && event.pointerId !== drag.pointerId) return;
          if (event && typeof ball.releasePointerCapture === "function") {
            ball.releasePointerCapture(event.pointerId);
          }
          drag.active = false;
          drag.pointerId = null;
          post({ cmd: "end_drag" });
        }

        ball.addEventListener("pointerup", endDrag);
        ball.addEventListener("pointercancel", endDrag);

        restartBreathingAnimation();
        applyBallState();
        applyAnalyticsButtons();
        applyBreathingButtons();
        applySizePresetButtons();
        applyUpdateBadge(false);
      })();
    </script>
  </body>
</html>"#;

const TELEMETRY_INFO_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light;
      }
      html,
      body {
        margin: 0;
        width: 100%;
        height: 100%;
        background: #f7f9fc;
        color: #18202a;
        font: 13px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      body {
        display: grid;
        align-content: center;
        justify-items: stretch;
        padding: 18px;
        box-sizing: border-box;
      }
      h1 {
        margin: 0 0 10px;
        font-size: 15px;
      }
      p {
        margin: 0 0 12px;
      }
      button {
        justify-self: end;
        border: 0;
        border-radius: 8px;
        padding: 7px 14px;
        background: #2f6bdb;
        color: #fff;
        font: inherit;
      }
    </style>
  </head>
  <body>
    <h1>Anonymous usage data</h1>
    <p>
      We collect basic app usage (first run, session length, menu interactions) and anonymous
      error reports to improve Downshift. No camera/mic. No window titles, text, or browsing data.
    </p>
    <button id="ok">OK</button>
    <script>
      (() => {
        const ok = document.getElementById("ok");
        ok.addEventListener("click", () => {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify({ cmd: "close_telemetry_info" }));
          }
        });
      })();
    </script>
  </body>
</html>"#;

const UPDATE_DIALOG_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light;
      }
      html,
      body {
        margin: 0;
        width: 100%;
        height: 100%;
        background: #f7f9fc;
        color: #18202a;
        font: 13px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      body {
        display: grid;
        align-content: center;
        justify-items: stretch;
        gap: 14px;
        padding: 18px;
        box-sizing: border-box;
      }
      .row {
        display: flex;
        align-items: center;
        gap: 10px;
        min-height: 42px;
      }
      .spinner {
        width: 14px;
        height: 14px;
        border-radius: 9999px;
        border: 2px solid #b9c8e9;
        border-top-color: #2f6bdb;
        animation: spin 0.9s linear infinite;
      }
      .spinner[hidden] {
        display: none;
      }
      .message {
        margin: 0;
      }
      .actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
      }
      button {
        border: 0;
        border-radius: 8px;
        padding: 7px 14px;
        font: inherit;
        cursor: default;
      }
      button.secondary {
        background: #dce5f7;
        color: #24324d;
      }
      button.primary {
        background: #2f6bdb;
        color: #fff;
      }
      button[hidden] {
        display: none;
      }
      @keyframes spin {
        to {
          transform: rotate(360deg);
        }
      }
    </style>
  </head>
  <body>
    <div class="row">
      <div class="spinner" id="spinner" hidden></div>
      <p class="message" id="message">checking for updates...</p>
    </div>
    <div class="actions">
      <button class="secondary" id="ok">ok</button>
      <button class="primary" id="download" hidden>download</button>
    </div>
    <script>
      (() => {
        const spinner = document.getElementById("spinner");
        const message = document.getElementById("message");
        const okButton = document.getElementById("ok");
        const downloadButton = document.getElementById("download");

        function post(payload) {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify(payload));
          }
        }

        window.updateDialogApplyState = function(next) {
          const state = next || {};
          const mode = String(state.mode || "checking");
          if (mode === "checking") {
            spinner.hidden = false;
            message.textContent = "checking for updates...";
            downloadButton.hidden = true;
            return;
          }
          spinner.hidden = true;
          if (mode === "available") {
            const latest = state.latest_version || "latest";
            message.textContent = `new update available (${latest})`;
            downloadButton.hidden = false;
            return;
          }
          message.textContent = "you are on the latest version!";
          downloadButton.hidden = true;
        };

        okButton.addEventListener("click", () => {
          post({ cmd: "close_update_dialog" });
        });

        downloadButton.addEventListener("click", () => {
          post({ cmd: "download_update" });
        });
      })();
    </script>
  </body>
</html>"#;

const CUSTOM_SNOOZE_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light;
      }
      html,
      body {
        margin: 0;
        width: 100%;
        height: 100%;
        background: #f7f9fc;
        color: #18202a;
        font: 13px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      body {
        display: grid;
        align-content: center;
        gap: 14px;
        padding: 18px;
        box-sizing: border-box;
      }
      label {
        display: grid;
        gap: 6px;
      }
      input {
        border: 1px solid #c8d3ea;
        border-radius: 8px;
        padding: 8px 10px;
        font: inherit;
      }
      .actions {
        display: flex;
        justify-content: flex-end;
        gap: 8px;
      }
      button {
        border: 0;
        border-radius: 8px;
        padding: 7px 14px;
        font: inherit;
        cursor: default;
      }
      button.secondary {
        background: #dce5f7;
        color: #24324d;
      }
      button.primary {
        background: #2f6bdb;
        color: #fff;
      }
    </style>
  </head>
  <body>
    <label>
      <span>snooze for how many minutes?</span>
      <input id="minutes" type="number" min="1" step="1" value="20" autofocus />
    </label>
    <div class="actions">
      <button class="secondary" id="cancel">cancel</button>
      <button class="primary" id="confirm">snooze</button>
    </div>
    <script>
      (() => {
        const input = document.getElementById("minutes");
        const cancelButton = document.getElementById("cancel");
        const confirmButton = document.getElementById("confirm");

        function post(payload) {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify(payload));
          }
        }

        function submit() {
          const minutes = Number(input.value);
          if (!Number.isFinite(minutes) || minutes < 1) {
            input.focus();
            input.select();
            return;
          }
          post({ cmd: "set_snooze", minutes: Math.round(minutes) });
        }

        cancelButton.addEventListener("click", () => {
          post({ cmd: "close_custom_snooze" });
        });
        confirmButton.addEventListener("click", submit);
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            submit();
          }
        });
      })();
    </script>
  </body>
</html>"#;

const BREATHING_PATTERN_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <style>
      :root {
        color-scheme: light;
      }
      html,
      body {
        margin: 0;
        width: 100%;
        height: 100%;
        background: #f7f9fc;
        color: #18202a;
        font: 13px/1.45 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      }
      body {
        display: grid;
        gap: 14px;
        padding: 18px;
        box-sizing: border-box;
      }
      .field {
        display: grid;
        gap: 6px;
      }
      .grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 12px;
      }
      input {
        border: 1px solid #c8d3ea;
        border-radius: 8px;
        padding: 8px 10px;
        background: #fff;
        color: inherit;
        font: inherit;
      }
      .hint {
        color: #58657d;
        font-size: 12px;
      }
      .save-row,
      .actions {
        display: flex;
        gap: 8px;
        align-items: center;
      }
      .save-row input {
        flex: 1;
      }
      .actions {
        justify-content: flex-end;
      }
      button {
        border: 0;
        border-radius: 8px;
        padding: 7px 14px;
        font: inherit;
        cursor: default;
      }
      button.secondary {
        background: #dce5f7;
        color: #24324d;
      }
      button.primary {
        background: #2f6bdb;
        color: #fff;
      }
    </style>
  </head>
  <body>
    <div class="grid">
      <label class="field">
        <span>breath in</span>
        <input id="expand" type="number" min="0.5" step="0.5" />
      </label>
      <label class="field">
        <span>hold at top</span>
        <input id="expand-hold" type="number" min="0" step="0.5" />
      </label>
      <label class="field">
        <span>breath out</span>
        <input id="compress" type="number" min="0.5" step="0.5" />
      </label>
      <label class="field">
        <span>hold at bottom</span>
        <input id="compress-hold" type="number" min="0" step="0.5" />
      </label>
    </div>
    <div class="hint" id="summary"></div>
    <div class="save-row">
      <input id="preset-name" type="text" maxlength="40" placeholder="name required" />
    </div>
    <div class="actions">
      <button class="secondary" id="cancel">close</button>
      <button class="primary" id="apply">add new</button>
    </div>
    <script>
      (() => {
        const expandInput = document.getElementById("expand");
        const expandHoldInput = document.getElementById("expand-hold");
        const compressInput = document.getElementById("compress");
        const compressHoldInput = document.getElementById("compress-hold");
        const summary = document.getElementById("summary");
        const presetNameInput = document.getElementById("preset-name");
        const cancelButton = document.getElementById("cancel");
        const applyButton = document.getElementById("apply");
        const state = {
          pattern: {
            expanding_seconds: 5.5,
            expanded_hold_seconds: 0,
            compressing_seconds: 5.5,
            compressed_hold_seconds: 0,
          },
        };

        function post(payload) {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify(payload));
          }
        }

        function normalizePattern(pattern) {
          const next = {
            expanding_seconds: Number(pattern && pattern.expanding_seconds),
            expanded_hold_seconds: Number(pattern && pattern.expanded_hold_seconds),
            compressing_seconds: Number(pattern && pattern.compressing_seconds),
            compressed_hold_seconds: Number(pattern && pattern.compressed_hold_seconds),
          };
          if (!Number.isFinite(next.expanding_seconds) || next.expanding_seconds <= 0) next.expanding_seconds = 5.5;
          if (!Number.isFinite(next.expanded_hold_seconds) || next.expanded_hold_seconds < 0) next.expanded_hold_seconds = 0;
          if (!Number.isFinite(next.compressing_seconds) || next.compressing_seconds <= 0) next.compressing_seconds = 5.5;
          if (!Number.isFinite(next.compressed_hold_seconds) || next.compressed_hold_seconds < 0) next.compressed_hold_seconds = 0;
          return next;
        }

        function readInputs() {
          return normalizePattern({
            expanding_seconds: expandInput.value,
            expanded_hold_seconds: expandHoldInput.value,
            compressing_seconds: compressInput.value,
            compressed_hold_seconds: compressHoldInput.value,
          });
        }

        function writeInputs(pattern) {
          expandInput.value = String(pattern.expanding_seconds);
          expandHoldInput.value = String(pattern.expanded_hold_seconds);
          compressInput.value = String(pattern.compressing_seconds);
          compressHoldInput.value = String(pattern.compressed_hold_seconds);
        }

        function updateSummary(pattern) {
          const total = pattern.expanding_seconds + pattern.expanded_hold_seconds + pattern.compressing_seconds + pattern.compressed_hold_seconds;
          summary.textContent = `cycle: ${pattern.expanding_seconds} / ${pattern.expanded_hold_seconds} / ${pattern.compressing_seconds} / ${pattern.compressed_hold_seconds} (${total}s total)`;
        }

        window.breathingPatternApplyState = function(next) {
          const payload = next || {};
          state.pattern = normalizePattern(payload.pattern || state.pattern);
          writeInputs(state.pattern);
          updateSummary(state.pattern);
        };

        [expandInput, expandHoldInput, compressInput, compressHoldInput].forEach((input) => {
          input.addEventListener("input", () => {
            const pattern = readInputs();
            state.pattern = pattern;
            updateSummary(pattern);
          });
        });

        applyButton.addEventListener("click", () => {
          const pattern = readInputs();
          state.pattern = pattern;
          const name = String(presetNameInput.value || "").trim();
          if (!name) {
            presetNameInput.focus();
            return;
          }
          post({ cmd: "save_breathing_preset", name, pattern });
          presetNameInput.value = "";
        });

        cancelButton.addEventListener("click", () => {
          post({ cmd: "close_breathing_pattern" });
        });
        presetNameInput.addEventListener("keydown", (event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            applyButton.click();
          }
        });
      })();
    </script>
  </body>
</html>"#;

#[derive(Debug, Clone)]
enum AppEvent {
    ExitRequested,
    Ipc(String),
    InstanceActivate,
    TelemetryHeartbeat,
    SnoozeExpired(u64),
    UpdateCheckFinished(UpdateCheckResult, UpdateCheckSource),
    #[cfg(target_os = "macos")]
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
    dismissed_badge_version: Option<String>,
}

impl Default for UpdateUiState {
    fn default() -> Self {
        Self {
            latest_version: None,
            download_url: download_release_url()
                .unwrap_or_else(|_| UPDATE_DOWNLOAD_FALLBACK_URL.to_string()),
            checking: false,
            checked_once: false,
            dismissed_badge_version: None,
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

    fn should_show_badge(&self) -> bool {
        let Some(latest) = self.latest_version.as_ref() else {
            return false;
        };
        if !self.has_update_available() {
            return false;
        }
        self.dismissed_badge_version.as_deref() != Some(latest.as_str())
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

fn downshift_env() -> String {
    optional_env_value("DOWNSHIFT_ENV", COMPILED_DOWNSHIFT_ENV)
        .unwrap_or_else(|| "unset".to_string())
}

fn resolve_compiled_setting(
    env_name: &str,
    compiled: Option<&str>,
    fallback: &str,
) -> Result<String, String> {
    resolve_compiled_setting_for_env(env_name, compiled, fallback, is_prod_env())
}

fn resolve_compiled_setting_for_env(
    env_name: &str,
    compiled: Option<&str>,
    fallback: &str,
    prod: bool,
) -> Result<String, String> {
    if let Some(value) = compiled.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(value.to_string());
    }

    if prod {
        return Err(format!("{env_name} is required when DOWNSHIFT_ENV=prod"));
    }

    Ok(fallback.to_string())
}

fn download_release_url() -> Result<String, String> {
    resolve_compiled_setting(
        "DOWNSHIFT_DOWNLOAD_RELEASE_URL",
        COMPILED_DOWNLOAD_RELEASE_URL,
        UPDATE_DOWNLOAD_FALLBACK_URL,
    )
}

fn is_prod_env() -> bool {
    downshift_env() == "prod"
}

fn resolve_external_contact_value(
    env_name: &str,
    compiled: Option<&str>,
    fallback: &str,
) -> Result<String, String> {
    if let Some(value) = optional_env_value(env_name, compiled) {
        return Ok(value);
    }

    if is_prod_env() {
        return Err(format!("{env_name} is required when DOWNSHIFT_ENV=prod"));
    }

    Ok(fallback.to_string())
}

fn validate_build_metadata_config() -> Result<(), String> {
    download_release_url()?;
    build_channel()?;
    github_issues_url()?;
    support_email_address()?;
    if telemetry_enabled()? {
        betterstack_logs_token()?;
        betterstack_logs_host()?;
        betterstack_errors_dsn()?;
        telemetry_heartbeat_interval_seconds()?;
    }
    Ok(())
}

fn build_channel() -> Result<String, String> {
    resolve_compiled_setting("DOWNSHIFT_BUILD_CHANNEL", COMPILED_BUILD_CHANNEL, "dev")
}

fn telemetry_enabled() -> Result<bool, String> {
    telemetry_enabled_for_env(COMPILED_TELEMETRY_ENABLED, is_prod_env())
}

fn telemetry_enabled_for_env(compiled: Option<&str>, prod: bool) -> Result<bool, String> {
    Ok(!matches!(
        resolve_compiled_setting_for_env("DOWNSHIFT_TELEMETRY_ENABLED", compiled, "true", prod)?
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off"
    ))
}

fn betterstack_logs_token() -> Result<String, String> {
    resolve_compiled_setting(
        "DOWNSHIFT_BETTERSTACK_LOGS_TOKEN",
        COMPILED_BETTERSTACK_LOGS_TOKEN,
        "",
    )
}

fn betterstack_logs_host() -> Result<String, String> {
    resolve_compiled_setting(
        "DOWNSHIFT_BETTERSTACK_LOGS_HOST",
        COMPILED_BETTERSTACK_LOGS_HOST,
        "",
    )
}

fn betterstack_errors_dsn() -> Result<String, String> {
    resolve_compiled_setting(
        "DOWNSHIFT_BETTERSTACK_ERRORS_DSN",
        COMPILED_BETTERSTACK_ERRORS_DSN,
        "",
    )
}

fn telemetry_heartbeat_interval_seconds() -> Result<u64, String> {
    let raw = resolve_compiled_setting(
        "DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC",
        COMPILED_TELEMETRY_HEARTBEAT_INTERVAL_SEC,
        "60",
    )?;
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

#[cfg(target_os = "macos")]
fn breathing_delete_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_DELETE_PREFIX}{id}")
}

#[cfg(target_os = "macos")]
fn deleted_breathing_preset_id_from_menu_id(id: &str) -> Option<&str> {
    id.strip_prefix(MENU_ID_BREATHING_DELETE_PREFIX)
}

#[cfg(target_os = "macos")]
fn breathing_saved_menu_id(id: &str) -> String {
    format!("{MENU_ID_BREATHING_SAVED_PREFIX}{id}")
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
    update_primary: MenuItem,
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

#[cfg(target_os = "macos")]
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
            &format!(
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
            &format!(
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
            &format!(
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
                        &breathing_saved_menu_id(&preset.id),
                        &format!(
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
                        &breathing_delete_menu_id(preset.id),
                        &format!(
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
                        &breathing_delete_menu_id(&preset.id),
                        &format!(
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
            &breathing_pattern_menu_label(),
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
                &update_primary,
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
            update_primary,
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

    fn sync_from_settings(&self, settings: &Settings, size_presets: [f64; 4], update_label: &str) {
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
        self.breathing_menu.set_text(&breathing_pattern_menu_label());
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
        self.update_primary.set_text(update_label);
        self.update_primary.set_enabled(true);
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
    #[cfg(target_os = "macos")]
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
    updates: UpdateUiState,
    manual_update_check_in_flight: bool,
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
            #[cfg(target_os = "macos")]
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
            updates: UpdateUiState::default(),
            manual_update_check_in_flight: false,
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
        diagnostics::DiagnosticsSnapshot {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os_version: current_os_version(),
            arch: std::env::consts::ARCH.to_string(),
            runtime_state: self.current_activity_label().to_string(),
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

    #[cfg(target_os = "macos")]
    fn apply_launch_at_login(&mut self, enabled: bool) {
        self.sync_launch_at_login_setting(enabled);
        self.sync_update_menu_state();
        self.save_settings();
    }

    #[cfg(target_os = "macos")]
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
        sync_child_webview_bounds(
            self.window.as_ref(),
            self.webview.as_ref(),
            "main webview",
        );
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
        #[cfg(target_os = "macos")]
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
        #[cfg(target_os = "macos")]
        if let Some(menu) = self.native_context_menu.as_ref() {
            menu.sync_from_settings(
                &self.settings,
                self.current_size_presets(),
                &self.updates.menu_label(),
            );
        }
    }

    fn sync_update_state_to_webview(&self) {
        let Some(webview) = self.webview.as_ref() else {
            return;
        };
        let js = format!(
            "window.breathBallApplyState({{ update_menu_label: {}, update_has_new_version: {}, update_show_badge: {} }});",
            serde_json::json!(self.updates.menu_label()),
            self.updates.has_update_available(),
            self.updates.should_show_badge()
        );
        let _ = webview.evaluate_script(&js);
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
        self.settings.dismissed_update_version = Some(latest.clone());
        self.updates.dismissed_badge_version = Some(latest.clone());
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
        self.updates.dismissed_badge_version = self.settings.dismissed_update_version.clone();

        if let Some(latest) = self.updates.latest_version.as_ref() {
            if self.settings.dismissed_update_version.as_deref() == Some(latest.as_str())
                && !self.updates.has_update_available()
            {
                self.settings.dismissed_update_version = None;
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
        if self.update_dialog_window.is_some() {
            if let Some(window) = self.update_dialog_window.as_ref() {
                window.focus_window();
            }
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("updates")
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(360.0, 168.0))
            .with_min_inner_size(LogicalSize::new(360.0, 168.0))
            .with_max_inner_size(LogicalSize::new(360.0, 168.0));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(error) => {
                log_stderr!("warning: failed to create update dialog window: {error}");
                return;
            }
        };
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
            log_stderr!("warning: missing event loop proxy for update dialog window");
            return;
        };
        let window_id = window.id();
        let webview = match WebViewBuilder::new()
            .with_html(UPDATE_DIALOG_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            })
            .build_as_child(&window)
        {
            Ok(webview) => webview,
            Err(error) => {
                log_stderr!("warning: failed to create update dialog webview: {error}");
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
        self.update_dialog_webview = None;
        self.update_dialog_window = None;
        self.update_dialog_window_id = None;
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
        if self.custom_snooze_window.is_some() {
            if let Some(window) = self.custom_snooze_window.as_ref() {
                window.focus_window();
            }
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("custom snooze")
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(320.0, 144.0))
            .with_min_inner_size(LogicalSize::new(320.0, 144.0))
            .with_max_inner_size(LogicalSize::new(320.0, 144.0));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(error) => {
                log_stderr!("warning: failed to create custom snooze window: {error}");
                return;
            }
        };
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
            log_stderr!("warning: missing event loop proxy for custom snooze window");
            return;
        };
        let window_id = window.id();
        let webview = match WebViewBuilder::new()
            .with_html(CUSTOM_SNOOZE_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            })
            .build_as_child(&window)
        {
            Ok(webview) => webview,
            Err(error) => {
                log_stderr!("warning: failed to create custom snooze webview: {error}");
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
        if self.breathing_pattern_window.is_some() {
            if let Some(window) = self.breathing_pattern_window.as_ref() {
                window.focus_window();
            }
            self.sync_breathing_pattern_editor_state();
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("add breathing pattern")
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(420.0, 340.0))
            .with_min_inner_size(LogicalSize::new(420.0, 340.0))
            .with_max_inner_size(LogicalSize::new(420.0, 340.0));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(error) => {
                log_stderr!("warning: failed to create breathing pattern window: {error}");
                return;
            }
        };
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
            log_stderr!("warning: missing event loop proxy for breathing pattern window");
            return;
        };
        let window_id = window.id();
        let webview = match WebViewBuilder::new()
            .with_html(BREATHING_PATTERN_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            })
            .build_as_child(&window)
        {
            Ok(webview) => webview,
            Err(error) => {
                log_stderr!("warning: failed to create breathing pattern webview: {error}");
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
        self.breathing_pattern_webview = None;
        self.breathing_pattern_window = None;
        self.breathing_pattern_window_id = None;
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
        self.custom_snooze_webview = None;
        self.custom_snooze_window = None;
        self.custom_snooze_window_id = None;
    }

    fn open_telemetry_info_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.telemetry_info_window.is_some() {
            if let Some(window) = self.telemetry_info_window.as_ref() {
                window.focus_window();
            }
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("what we collect")
            .with_resizable(false)
            .with_inner_size(LogicalSize::new(420.0, 240.0))
            .with_min_inner_size(LogicalSize::new(420.0, 240.0))
            .with_max_inner_size(LogicalSize::new(420.0, 240.0));
        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(error) => {
                log_stderr!("warning: failed to create telemetry info window: {error}");
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
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
            log_stderr!("warning: missing event loop proxy for telemetry info window");
            return;
        };
        let window_id = window.id();
        let webview = match WebViewBuilder::new()
            .with_html(TELEMETRY_INFO_HTML)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            })
            .build_as_child(&window)
        {
            Ok(webview) => webview,
            Err(error) => {
                log_stderr!("warning: failed to create telemetry info webview: {error}");
                return;
            }
        };
        self.telemetry_info_window = Some(window);
        self.telemetry_info_window_id = Some(window_id);
        self.telemetry_info_webview = Some(webview);
        self.sync_telemetry_info_webview_bounds();
    }

    fn close_telemetry_info_window(&mut self) {
        self.telemetry_info_webview = None;
        self.telemetry_info_window = None;
        self.telemetry_info_window_id = None;
    }

    fn show_analytics_modal(&mut self, event_loop: &ActiveEventLoop) {
        self.open_telemetry_info_window(event_loop);
    }

    fn handle_telemetry_info_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_telemetry_info_window(),
            WindowEvent::Resized(_) => self.sync_telemetry_info_webview_bounds(),
            _ => {}
        }
    }

    fn handle_custom_snooze_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_custom_snooze_window(),
            WindowEvent::Resized(_) => self.sync_custom_snooze_webview_bounds(),
            _ => {}
        }
    }

    fn handle_breathing_pattern_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.cancel_breathing_pattern_window(),
            WindowEvent::Resized(_) => self.sync_breathing_pattern_webview_bounds(),
            _ => {}
        }
    }

    fn handle_update_dialog_window_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.close_update_dialog_window(),
            WindowEvent::Resized(_) => self.sync_update_dialog_webview_bounds(),
            _ => {}
        }
    }

    fn sync_privacy_state_to_webview(&self) {
        if let Some(webview) = self.webview.as_ref() {
            let js = format!(
                "window.breathBallApplyState({{ usage_data_sharing: {}, crash_reports_sharing: {} }});",
                self.settings.usage_data_sharing, self.settings.crash_reports_sharing
            );
            let _ = webview.evaluate_script(&js);
        }
    }

    fn choose_initial_position(
        &self,
        event_loop: &ActiveEventLoop,
        size: f64,
    ) -> Option<LogicalPosition<f64>> {
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        if monitors.is_empty() {
            return None;
        }
        let primary = event_loop
            .primary_monitor()
            .or_else(|| monitors.first().cloned())?;

        if let (Some(saved_x), Some(saved_y)) = (self.settings.x, self.settings.y) {
            let saved = LogicalPosition::new(saved_x as f64, saved_y as f64);
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
        Some(default_corner_position(&primary, size))
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
          "update_tooltip": UPDATE_TOOLTIP,
          "size_presets": size_presets,
          "use_native_menu": cfg!(target_os = "macos"),
        });
        format!("window.__BB_INIT__ = {payload};")
    }

    fn current_window_logical_position(&self) -> Option<LogicalPosition<f64>> {
        let window = self.window.as_ref()?;
        let physical = window.outer_position().ok()?;
        Some(physical.to_logical(window.scale_factor()))
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
            self.settings.x = Some(next_x);
            self.settings.y = Some(next_y);
        }
    }

    fn sync_breathing_pattern_to_webview(&self) {
        if let Some(webview) = self.webview.as_ref() {
            let js = format!(
                "window.breathBallApplyState({{ breathing_pattern: {}, active_breathing_preset_id: {}, breathing_presets: {} }});",
                serde_json::json!(self.settings.breathing_pattern),
                serde_json::json!(self.settings.active_breathing_preset_id),
                self.breathing_pattern_menu_presets_payload()
            );
            let _ = webview.evaluate_script(&js);
        }
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
        if let Some(webview) = self.webview.as_ref() {
            let js = format!("window.breathBallApplyState({{ paused: {} }});", paused);
            let _ = webview.evaluate_script(&js);
        }
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
            let logical = physical.to_logical::<f64>(window.scale_factor());
            self.settings.x = Some(logical.x.round() as i32);
            self.settings.y = Some(logical.y.round() as i32);
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
                let x = pos.x.round() as i32;
                let y = pos.y.round() as i32;
                window.set_outer_position(LogicalPosition::new(x, y));
                self.settings.x = Some(x);
                self.settings.y = Some(y);
                self.settings.monitor = Some(snapshot_monitor(monitor));
            }
        }
    }

    fn handle_ipc_command(&mut self, event_loop: &ActiveEventLoop, command: IpcCommand) {
        match command {
            IpcCommand::Quit => {
                self.telemetry_menu_action(MenuAction::Quit, None);
                self.save_settings();
                self.finish_session(SessionEndReason::QuitMenu);
                event_loop.exit();
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
            IpcCommand::CloseUpdateDialog => self.close_update_dialog_window(),
            IpcCommand::DownloadUpdate => {
                self.close_update_dialog_window();
                self.launch_update_download("dialog");
            }
            IpcCommand::ShowContextMenu { x, y } => {
                self.telemetry_menu_action(MenuAction::ContextMenu, None);
                #[cfg(target_os = "macos")]
                self.show_native_context_menu(x, y);
                #[cfg(not(target_os = "macos"))]
                let _ = (x, y);
            }
            IpcCommand::Resize { delta, fine } => {
                let next = apply_resize_step(self.settings.size, delta, fine);
                self.apply_size(next);
                self.save_settings();
            }
            IpcCommand::SetSize { size } => {
                let size_target = self
                    .window
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
                    .and_then(size_target_label);
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
        let Some(webview) = self.webview.as_ref() else {
            return;
        };
        let presets = size_presets_for_monitor(monitor);
        let js = format!(
            "window.breathBallApplyState({{ size_presets: {} }});",
            serde_json::json!(presets)
        );
        let _ = webview.evaluate_script(&js);
    }

    #[cfg(target_os = "macos")]
    fn current_size_presets(&self) -> [f64; 4] {
        self.window
            .as_ref()
            .and_then(|window| window.current_monitor())
            .map(|monitor| size_presets_for_monitor(&monitor))
            .unwrap_or([64.0, 96.0, 128.0, 160.0])
    }

    #[cfg(target_os = "macos")]
    fn show_native_context_menu(&mut self, x: i32, y: i32) {
        self.native_context_menu = NativeContextMenu::new(&self.settings);
        let Some(menu) = self.native_context_menu.as_ref() else {
            return;
        };
        let Some(window) = self.window.as_ref() else {
            return;
        };
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
        menu.sync_from_settings(
            &self.settings,
            self.current_size_presets(),
            &self.updates.menu_label(),
        );
        menu.sync_consent(
            self.settings.usage_data_sharing,
            self.settings.crash_reports_sharing,
        );
        let position = MenuPhysicalPosition::new(x as f64, y as f64).into();
        unsafe {
            let _ = menu
                .root
                .show_context_menu_for_nsview(view.cast_const(), Some(position));
        }
    }

    #[cfg(target_os = "macos")]
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
                self.telemetry_menu_action(MenuAction::Quit, None);
                self.save_settings();
                self.finish_session(SessionEndReason::QuitMenu);
                event_loop.exit();
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
            MENU_ID_ANALYTICS_INFO => self.show_analytics_modal(event_loop),
            MENU_ID_UPDATE_PRIMARY => self.handle_update_primary_action(event_loop),
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
        if let Some(error) = self.settings_load_error.as_ref() {
            log_stderr!("warning: {error}");
        }
        self.updates.dismissed_badge_version = self.settings.dismissed_update_version.clone();
        self.updates.latest_version = self.settings.cached_latest_update_version.clone();
        self.telemetry
            .set_usage_enabled(self.settings.usage_data_sharing);
        self.telemetry
            .set_crash_enabled(self.settings.crash_reports_sharing);
        #[cfg(target_os = "macos")]
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

        if let Some(position) = self.choose_initial_position(event_loop, self.settings.size) {
            let x = position.x.round() as i32;
            let y = position.y.round() as i32;
            window_attributes = window_attributes.with_position(LogicalPosition::new(x, y));
            self.settings.x = Some(x);
            self.settings.y = Some(y);
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
        self.settings.monitor = window.current_monitor().map(snapshot_monitor);

        let startup_monitor = window
            .current_monitor()
            .or_else(|| event_loop.primary_monitor())
            .or_else(|| event_loop.available_monitors().next());
        let size_presets = startup_monitor
            .as_ref()
            .map(size_presets_for_monitor)
            .unwrap_or([64.0, 96.0, 128.0, 160.0]);
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
        let webview_result = WebViewBuilder::new()
            .with_html(BREATH_HTML)
            .with_transparent(true)
            .with_initialization_script(&init_script)
            .with_ipc_handler(move |request: wry::http::Request<String>| {
                let payload = request.into_body();
                let _ = ipc_proxy.send_event(AppEvent::Ipc(payload));
            })
            .build_as_child(&window);

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
        #[cfg(target_os = "macos")]
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
            #[cfg(target_os = "macos")]
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

fn default_corner_position(monitor: &MonitorHandle, size: f64) -> LogicalPosition<f64> {
    let scale = monitor.scale_factor();
    let monitor_pos = monitor.position().to_logical::<f64>(scale);
    let monitor_size = monitor.size().to_logical::<f64>(scale);
    let margin = monitor_size.width.min(monitor_size.height) * DEFAULT_EDGE_MARGIN_RATIO;
    LogicalPosition::new(
        monitor_pos.x + monitor_size.width - size - margin,
        monitor_pos.y + margin,
    )
}

fn default_size_for_monitor(monitor: &MonitorHandle) -> f64 {
    let scale = monitor.scale_factor();
    let size = monitor.size().to_logical::<f64>(scale);
    let shorter_side = size.width.min(size.height);
    clamp_size(shorter_side * DEFAULT_SIZE_SHORT_SIDE_RATIO)
}

fn heartbeat_interval() -> Duration {
    let value = telemetry_heartbeat_interval_seconds().unwrap_or(DEFAULT_HEARTBEAT_INTERVAL_SEC);
    Duration::from_secs(value)
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

#[cfg(unix)]
fn instance_socket_path() -> Option<PathBuf> {
    let mut path = dirs::config_dir()?;
    path.push("downshift");
    path.push("instance.sock");
    Some(path)
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
    if let Err(error) = validate_build_metadata_config() {
        log_stderr!("error: {error}");
        return std::process::ExitCode::from(1);
    }

    let panic_telemetry = RuntimeTelemetryClient::from_env();
    let default_panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = panic_info;
        panic_telemetry.track_error(
            EventName::AppCrash,
            serde_json::json!({
                "category": "panic",
                "fatal": true,
            }),
        );
        panic_telemetry.end_session(SessionEndReason::Panic);
        panic_telemetry.flush(std::time::Duration::from_secs(2));
        panic_telemetry.shutdown(std::time::Duration::from_secs(2));
        default_panic_hook(panic_info);
    }));

    let mut event_loop_builder = EventLoop::<AppEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
    let event_loop = match event_loop_builder.build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            log_stderr!("error: failed to create event loop: {error}");
            return std::process::ExitCode::from(1);
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

    let ctrlc_proxy = event_loop_proxy.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        let _ = ctrlc_proxy.send_event(AppEvent::ExitRequested);
    }) {
        log_stderr!("warning: failed to install ctrl-c handler: {error}");
    }
    #[cfg(target_os = "macos")]
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

    let mut app = App::default();
    app.event_loop_proxy = Some(event_loop_proxy);

    if let Err(error) = event_loop.run_app(&mut app) {
        app.finish_session(SessionEndReason::EventLoopFailure);
        app.telemetry.track_error(
            EventName::AppError,
            serde_json::json!({
                "category": "event_loop",
                "severity": "error",
                "recoverable": false,
            }),
        );
        log_stderr!("error: app event loop failed: {error}");
        return std::process::ExitCode::from(1);
    }
    if let Some(error) = app.startup_error {
        log_stderr!("error: {error}");
        return std::process::ExitCode::from(1);
    }
    app.finish_session(SessionEndReason::Unknown);
    std::process::ExitCode::SUCCESS
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
        std::env::remove_var("DOWNSHIFT_GITHUB_ISSUES_URL");
        std::env::remove_var("DOWNSHIFT_SUPPORT_EMAIL");
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
            UPDATE_DOWNLOAD_FALLBACK_URL
        );
        assert_eq!(
            github_issues_url().expect("github issues url"),
            DEFAULT_GITHUB_ISSUES_URL
        );
        assert_eq!(
            support_email_address().expect("support email"),
            DEFAULT_SUPPORT_EMAIL
        );
    }

    #[test]
    #[serial]
    fn external_contact_values_are_required_in_prod() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "prod");

        let download_error = download_release_url().expect_err("download release url should fail");
        let build_channel_error = build_channel().expect_err("build channel should fail");
        let github_error = github_issues_url().expect_err("github issues url should fail");
        let email_error = support_email_address().expect_err("support email should fail");
        let telemetry_enabled_error =
            telemetry_enabled().expect_err("telemetry enabled should fail when prod");

        assert_eq!(
            download_error,
            "DOWNSHIFT_DOWNLOAD_RELEASE_URL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            build_channel_error,
            "DOWNSHIFT_BUILD_CHANNEL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            github_error,
            "DOWNSHIFT_GITHUB_ISSUES_URL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            email_error,
            "DOWNSHIFT_SUPPORT_EMAIL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            telemetry_enabled_error,
            "DOWNSHIFT_TELEMETRY_ENABLED is required when DOWNSHIFT_ENV=prod"
        );
    }

    #[test]
    #[serial]
    fn external_contact_values_use_runtime_env_when_set() {
        clear_external_contact_env();
        std::env::set_var("DOWNSHIFT_ENV", "prod");
        std::env::set_var("DOWNSHIFT_GITHUB_ISSUES_URL", "https://example.com/issues");
        std::env::set_var("DOWNSHIFT_SUPPORT_EMAIL", "support@example.com");

        let download_error =
            download_release_url().expect_err("download release url should stay compile-time only");
        let build_channel_error =
            build_channel().expect_err("build channel should stay compile-time only");
        let telemetry_enabled_error =
            telemetry_enabled().expect_err("telemetry enabled should stay compile-time only");
        assert_eq!(
            download_error,
            "DOWNSHIFT_DOWNLOAD_RELEASE_URL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            build_channel_error,
            "DOWNSHIFT_BUILD_CHANNEL is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            telemetry_enabled_error,
            "DOWNSHIFT_TELEMETRY_ENABLED is required when DOWNSHIFT_ENV=prod"
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
    fn telemetry_enabled_parses_false_variants() {
        assert!(!telemetry_enabled_for_env(Some("false"), false).expect("false"));
        assert!(!telemetry_enabled_for_env(Some("0"), false).expect("zero"));
        assert!(!telemetry_enabled_for_env(Some("off"), false).expect("off"));
        assert!(telemetry_enabled_for_env(Some("true"), false).expect("true"));
    }

    #[test]
    fn telemetry_dependencies_are_required_in_prod_when_enabled() {
        let token_error =
            resolve_compiled_setting_for_env("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN", None, "", true)
                .expect_err("logs token should fail");
        let host_error =
            resolve_compiled_setting_for_env("DOWNSHIFT_BETTERSTACK_LOGS_HOST", None, "", true)
                .expect_err("logs host should fail");
        let dsn_error =
            resolve_compiled_setting_for_env("DOWNSHIFT_BETTERSTACK_ERRORS_DSN", None, "", true)
                .expect_err("errors dsn should fail");
        let heartbeat_error = resolve_compiled_setting_for_env(
            "DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC",
            None,
            "60",
            true,
        )
        .expect_err("heartbeat interval should fail");

        assert_eq!(
            token_error,
            "DOWNSHIFT_BETTERSTACK_LOGS_TOKEN is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            host_error,
            "DOWNSHIFT_BETTERSTACK_LOGS_HOST is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            dsn_error,
            "DOWNSHIFT_BETTERSTACK_ERRORS_DSN is required when DOWNSHIFT_ENV=prod"
        );
        assert_eq!(
            heartbeat_error,
            "DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC is required when DOWNSHIFT_ENV=prod"
        );
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

        let mut app = App::default();
        app.telemetry = telemetry;
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

    #[test]
    fn resume_from_snooze_restores_active_state_without_pausing() {
        let mut app = App::default();
        app.activity_mode = ActivityMode::Snoozed;
        app.settings.paused = true;
        app.snooze_deadline = Some(SystemTime::now() + Duration::from_secs(60));

        assert!(app.resume_from_snooze());
        assert_eq!(app.activity_mode, ActivityMode::Active);
        assert!(!app.settings.paused);
        assert!(app.snooze_deadline.is_none());
    }

    #[test]
    fn resume_from_snooze_is_noop_when_not_snoozed() {
        let mut app = App::default();
        app.activity_mode = ActivityMode::Paused;
        app.settings.paused = true;

        assert!(!app.resume_from_snooze());
        assert_eq!(app.activity_mode, ActivityMode::Paused);
        assert!(app.settings.paused);
    }

    #[test]
    fn reconcile_snooze_after_resume_expires_elapsed_snooze() {
        let mut app = App::default();
        app.activity_mode = ActivityMode::Snoozed;
        app.snooze_deadline = Some(SystemTime::now() - Duration::from_secs(1));

        app.reconcile_snooze_after_resume();

        assert_eq!(app.activity_mode, ActivityMode::Active);
        assert!(app.snooze_deadline.is_none());
    }

    #[test]
    fn reconcile_snooze_after_resume_keeps_pending_snooze() {
        let mut app = App::default();
        app.activity_mode = ActivityMode::Snoozed;
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

        let mut app = App::default();
        app.config_path = Some(settings_path.clone());
        app.settings = Settings::default();
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
    #[cfg(target_os = "macos")]
    fn snooze_menu_id_maps_to_expected_minutes() {
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_5), Some(5));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_10), Some(10));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_15), Some(15));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_30), Some(30));
        assert_eq!(snooze_minutes_for_menu_id(MENU_ID_SNOOZE_60), Some(60));
        assert_eq!(snooze_minutes_for_menu_id("nope"), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
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
