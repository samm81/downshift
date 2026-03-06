use downshift::telemetry::{
    menu_action_size_target, ActivityState, ActivityTrigger, EventName, MenuAction,
    RuntimeTelemetryClient, SessionEndReason, SizeTarget, TelemetryClient,
};
use downshift::{
    apply_resize_step, clamp_size, load_settings, normalize_half_cycle, IpcCommand,
    PersistedMonitor, Settings, DEFAULT_HALF_CYCLE_SECONDS, DEFAULT_SIZE,
};
#[cfg(target_os = "macos")]
use muda::dpi::PhysicalPosition as MenuPhysicalPosition;
#[cfg(target_os = "macos")]
use muda::{CheckMenuItem, ContextMenu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
#[cfg(target_os = "macos")]
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId, WindowLevel};
use wry::{Rect, WebView, WebViewBuilder};

const DEFAULT_SIZE_SHORT_SIDE_RATIO: f64 = 0.10;
const DEFAULT_EDGE_MARGIN_RATIO: f64 = 0.05;
const SIZE_PRESET_RATIOS: [f64; 4] = [0.08, 0.10, 0.13, 0.16];
#[cfg(target_os = "macos")]
const MENU_ID_PAUSE: &str = "pause";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_S: &str = "size_s";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_M: &str = "size_m";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_L: &str = "size_l";
#[cfg(target_os = "macos")]
const MENU_ID_SIZE_XL: &str = "size_xl";
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
        width: 100%;
        aspect-ratio: 1 / 1;
        border-radius: 9999px;
        background: rgba(124, 182, 255, 0.52);
        box-shadow: inset 0 0 0 1px rgba(124, 182, 255, 0.35);
        transform: scale(0.8);
        transform-origin: center;
        /* keep in sync with docs/styles.css .demo-ball animation */
        animation: breathe 5.5s cubic-bezier(0.42, 0, 0.58, 1) infinite alternate;
      }
      .ball.paused {
        animation-play-state: paused;
        opacity: 0.55;
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
      @keyframes breathe {
        from {
          transform: scale(0.8);
        }
        to {
          transform: scale(1);
        }
      }
    </style>
  </head>
  <body>
    <div class="ball" id="ball"></div>
    <div class="menu" id="menu" hidden>
      <button id="menu-pause">pause</button>
      <div class="divider"></div>
      <div class="group">
        <div class="label">size</div>
        <button data-size-slot="0">S</button>
        <button data-size-slot="1">M</button>
        <button data-size-slot="2">L</button>
        <button data-size-slot="3">XL</button>
      </div>
      <div class="divider"></div>
      <button id="menu-reset">reset</button>
      <button id="menu-quit">quit</button>
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
        const analyticsToggleButton = document.getElementById("menu-analytics-toggle");
        const analyticsSubmenu = document.getElementById("analytics-submenu");
        const usageOnButton = document.getElementById("menu-usage-on");
        const usageOffButton = document.getElementById("menu-usage-off");
        const crashOnButton = document.getElementById("menu-crash-on");
        const crashOffButton = document.getElementById("menu-crash-off");
        const whatWeCollectButton = document.getElementById("menu-what-we-collect");
        const sizeButtons = Array.from(document.querySelectorAll("[data-size-slot]"));
        const init = window.__BB_INIT__ || { paused: false, half_cycle_seconds: 5.5, use_native_menu: false };
        const useNativeMenu = Boolean(init.use_native_menu);
        const state = {
          paused: Boolean(init.paused),
          halfCycleSeconds: Number(init.half_cycle_seconds) || 5.5,
          usageDataSharing: Object.prototype.hasOwnProperty.call(init, "usage_data_sharing") ? Boolean(init.usage_data_sharing) : true,
          crashReportsSharing: Object.prototype.hasOwnProperty.call(init, "crash_reports_sharing") ? Boolean(init.crash_reports_sharing) : true,
          analyticsOpen: false,
          sizePresets: Array.isArray(init.size_presets) && init.size_presets.length === 4
            ? init.size_presets.map((value) => Number(value)).filter((value) => Number.isFinite(value) && value > 0)
            : [64, 96, 128, 160],
        };
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
          state.analyticsOpen = false;
        }

        function showMenu(x, y) {
          menu.style.left = `${x}px`;
          menu.style.top = `${y}px`;
          menu.hidden = false;
        }

        function applyBallState() {
          ball.classList.toggle("paused", state.paused);
          // halfCycleSeconds is inhale OR exhale duration; with CSS `alternate`,
          // one animation iteration maps to one half-breath.
          const seconds = Math.max(2.0, state.halfCycleSeconds);
          ball.style.animationDuration = `${seconds}s`;
          pauseButton.textContent = state.paused ? "resume" : "pause";
        }

        function applyAnalyticsButtons() {
          usageOnButton.textContent = `share anonymous usage data ${state.usageDataSharing ? "✓" : ""}`.trim();
          usageOffButton.textContent = `don’t share usage data ${!state.usageDataSharing ? "✓" : ""}`.trim();
          crashOnButton.textContent = `share anonymous crash reports ${state.crashReportsSharing ? "✓" : ""}`.trim();
          crashOffButton.textContent = `don't share crash reports ${!state.crashReportsSharing ? "✓" : ""}`.trim();
          analyticsToggleButton.textContent = "help improve downshift";
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

        window.breathBallApplyState = function(next) {
          if (Object.prototype.hasOwnProperty.call(next, "paused")) {
            state.paused = Boolean(next.paused);
          }
          if (Object.prototype.hasOwnProperty.call(next, "half_cycle_seconds")) {
            const value = Number(next.half_cycle_seconds);
            if (Number.isFinite(value) && value > 0) {
              state.halfCycleSeconds = value;
            }
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
          applyBallState();
          applyAnalyticsButtons();
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

        resetButton.addEventListener("click", () => {
          state.paused = false;
          state.halfCycleSeconds = 5.5;
          applyBallState();
          post({ cmd: "reset" });
          hideMenu();
        });

        quitButton.addEventListener("click", () => {
          post({ cmd: "quit" });
          hideMenu();
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
        window.addEventListener("resize", hideMenu);

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

        applyBallState();
        applyAnalyticsButtons();
        applySizePresetButtons();
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

#[derive(Debug, Clone)]
enum AppEvent {
    ExitRequested,
    Ipc(String),
    #[cfg(target_os = "macos")]
    MenuActivated(String),
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct NativeContextMenu {
    root: Submenu,
    pause: CheckMenuItem,
    size_menu: Submenu,
    size_s: MenuItem,
    size_m: MenuItem,
    size_l: MenuItem,
    size_xl: MenuItem,
    reset: MenuItem,
    quit: MenuItem,
    analytics_menu: Submenu,
    usage_on: CheckMenuItem,
    usage_off: CheckMenuItem,
    crash_on: CheckMenuItem,
    crash_off: CheckMenuItem,
    analytics_info: MenuItem,
}

#[cfg(target_os = "macos")]
impl NativeContextMenu {
    fn new() -> Option<Self> {
        let pause = CheckMenuItem::with_id(MENU_ID_PAUSE, "paused", true, false, None);
        let size_s = MenuItem::with_id(MENU_ID_SIZE_S, "S (64px)", true, None);
        let size_m = MenuItem::with_id(MENU_ID_SIZE_M, "M (96px)", true, None);
        let size_l = MenuItem::with_id(MENU_ID_SIZE_L, "L (128px)", true, None);
        let size_xl = MenuItem::with_id(MENU_ID_SIZE_XL, "XL (160px)", true, None);
        let reset = MenuItem::with_id(MENU_ID_RESET, "reset", true, None);
        let quit = MenuItem::with_id(MENU_ID_QUIT, "quit", true, None);
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
                eprintln!("warning: failed to build analytics submenu: {error}");
                return None;
            }
        };
        let size_submenu =
            match Submenu::with_items("size", true, &[&size_s, &size_m, &size_l, &size_xl]) {
                Ok(menu) => menu,
                Err(error) => {
                    eprintln!("warning: failed to build size submenu: {error}");
                    return None;
                }
            };
        let separator_one = PredefinedMenuItem::separator();
        let separator_two = PredefinedMenuItem::separator();
        let separator_three = PredefinedMenuItem::separator();
        let root = match Submenu::with_items(
            "menu",
            true,
            &[
                &pause,
                &separator_one,
                &size_submenu,
                &separator_two,
                &reset,
                &quit,
                &separator_three,
                &analytics_menu,
            ],
        ) {
            Ok(menu) => menu,
            Err(error) => {
                eprintln!("warning: failed to build native context menu: {error}");
                return None;
            }
        };
        Some(Self {
            root,
            pause,
            size_menu: size_submenu,
            size_s,
            size_m,
            size_l,
            size_xl,
            reset,
            quit,
            analytics_menu,
            usage_on,
            usage_off,
            crash_on,
            crash_off,
            analytics_info,
        })
    }

    fn sync_from_settings(&self, settings: &Settings, size_presets: [f64; 4]) {
        self.pause.set_checked(settings.paused);
        self.pause
            .set_text(if settings.paused { "paused" } else { "pause" });
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
        self.reset.set_enabled(true);
        self.quit.set_enabled(true);
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
    telemetry_info_window: Option<Window>,
    telemetry_info_window_id: Option<WindowId>,
    telemetry_info_webview: Option<WebView>,
    #[cfg(target_os = "macos")]
    native_context_menu: Option<NativeContextMenu>,
    startup_error: Option<String>,
    settings: Settings,
    config_path: Option<std::path::PathBuf>,
    event_loop_proxy: Option<EventLoopProxy<AppEvent>>,
    drag_anchor_window_pos: Option<LogicalPosition<f64>>,
    drag_anchor_pointer_pos: Option<LogicalPosition<f64>>,
    telemetry: RuntimeTelemetryClient,
    telemetry_install_first_run: bool,
    session_ended: bool,
}

impl Default for App {
    fn default() -> Self {
        let telemetry_state = RuntimeTelemetryClient::telemetry_state();
        let telemetry_install_first_run = telemetry_state.install_first_run;
        let telemetry = RuntimeTelemetryClient::from_state(telemetry_state);
        Self {
            window: None,
            window_id: None,
            webview: None,
            telemetry_info_window: None,
            telemetry_info_window_id: None,
            telemetry_info_webview: None,
            #[cfg(target_os = "macos")]
            native_context_menu: None,
            startup_error: None,
            settings: Settings::default(),
            config_path: None,
            event_loop_proxy: None,
            drag_anchor_window_pos: None,
            drag_anchor_pointer_pos: None,
            telemetry,
            telemetry_install_first_run,
            session_ended: false,
        }
    }
}

impl App {
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
        let (Some(window), Some(webview)) = (self.window.as_ref(), self.webview.as_ref()) else {
            return;
        };
        let size = window.inner_size().to_logical::<u32>(window.scale_factor());
        let bounds = Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(size.width, size.height).into(),
        };
        if let Err(error) = webview.set_bounds(bounds) {
            eprintln!("warning: failed to sync webview bounds: {error}");
        }
    }

    fn sync_telemetry_info_webview_bounds(&self) {
        let (Some(window), Some(webview)) = (
            self.telemetry_info_window.as_ref(),
            self.telemetry_info_webview.as_ref(),
        ) else {
            return;
        };
        let size = window.inner_size().to_logical::<u32>(window.scale_factor());
        let bounds = Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(size.width, size.height).into(),
        };
        if let Err(error) = webview.set_bounds(bounds) {
            eprintln!("warning: failed to sync telemetry info webview bounds: {error}");
        }
    }

    fn config_path() -> Option<std::path::PathBuf> {
        let mut path = dirs::config_dir()?;
        path.push("downshift");
        path.push("settings.toml");
        Some(path)
    }

    fn save_settings(&self) {
        let Some(path) = &self.config_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!("warning: failed to create config directory: {error}");
                return;
            }
        }
        let content = match toml::to_string_pretty(&self.settings) {
            Ok(content) => content,
            Err(error) => {
                eprintln!("warning: failed to serialize settings: {error}");
                return;
            }
        };
        if let Err(error) = std::fs::write(path, content) {
            eprintln!("warning: failed to write settings: {error}");
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
                eprintln!("warning: failed to create telemetry info window: {error}");
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
            eprintln!("warning: missing event loop proxy for telemetry info window");
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
                eprintln!("warning: failed to create telemetry info webview: {error}");
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
          "paused": self.settings.paused,
          "half_cycle_seconds": self.settings.half_cycle_seconds,
          "usage_data_sharing": self.settings.usage_data_sharing,
          "crash_reports_sharing": self.settings.crash_reports_sharing,
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

    fn apply_half_cycle(&mut self, half_cycle_seconds: f64) {
        self.settings.half_cycle_seconds = normalize_half_cycle(half_cycle_seconds);
        if let Some(webview) = self.webview.as_ref() {
            let js = format!(
                "window.breathBallApplyState({{ half_cycle_seconds: {} }});",
                self.settings.half_cycle_seconds
            );
            let _ = webview.evaluate_script(&js);
        }
    }

    fn apply_paused(&mut self, paused: bool) {
        self.settings.paused = paused;
        if let Some(webview) = self.webview.as_ref() {
            let js = format!("window.breathBallApplyState({{ paused: {} }});", paused);
            let _ = webview.evaluate_script(&js);
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
        self.apply_half_cycle(DEFAULT_HALF_CYCLE_SECONDS);
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
                let action = if paused {
                    MenuAction::Pause
                } else {
                    MenuAction::Resume
                };
                self.telemetry_menu_action(action, None);
                self.apply_paused(paused);
                if paused {
                    self.telemetry_activity_state(
                        ActivityState::Disabled,
                        ActivityTrigger::DisabledForever,
                        None,
                    );
                } else {
                    self.telemetry_activity_state(ActivityState::Active, ActivityTrigger::Manual, None);
                }
                self.save_settings();
            }
            IpcCommand::SetSpeed { half_cycle_seconds } => {
                self.apply_half_cycle(half_cycle_seconds);
                self.save_settings();
            }
            IpcCommand::SetUsageDataSharing { enabled } => {
                self.settings.usage_data_sharing = enabled;
                self.telemetry.set_usage_enabled(enabled);
                self.telemetry_privacy_change("usage_data", enabled);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            IpcCommand::SetCrashReportsSharing { enabled } => {
                self.settings.crash_reports_sharing = enabled;
                self.telemetry.set_crash_enabled(enabled);
                self.telemetry_privacy_change("crash_reports", enabled);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            IpcCommand::AnalyticsMenuOpened => {
                self.telemetry_menu_action(MenuAction::AnalyticsMenu, None);
            }
            IpcCommand::ShowTelemetryInfo => {
                self.open_telemetry_info_window(event_loop);
            }
            IpcCommand::CloseTelemetryInfo => self.close_telemetry_info_window(),
            IpcCommand::ShowContextMenu { x, y } => {
                self.telemetry_menu_action(MenuAction::AnalyticsMenu, None);
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
                    .and_then(menu_action_size_target)
                    .map(|target| match target {
                        SizeTarget::S => "S",
                        SizeTarget::M => "M",
                        SizeTarget::L => "L",
                        SizeTarget::Xl => "XL",
                    });
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
                eprintln!("warning: failed to access window handle for native menu: {error}");
                return;
            }
        };
        menu.sync_from_settings(&self.settings, self.current_size_presets());
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
                let action = if !self.settings.paused {
                    MenuAction::Pause
                } else {
                    MenuAction::Resume
                };
                self.telemetry_menu_action(action, None);
                let next_paused = !self.settings.paused;
                self.apply_paused(next_paused);
                if next_paused {
                    self.telemetry_activity_state(
                        ActivityState::Disabled,
                        ActivityTrigger::DisabledForever,
                        None,
                    );
                } else {
                    self.telemetry_activity_state(ActivityState::Active, ActivityTrigger::Manual, None);
                }
                self.save_settings();
            }
            MENU_ID_SIZE_S | MENU_ID_SIZE_M | MENU_ID_SIZE_L | MENU_ID_SIZE_XL => {
                let presets = self.current_size_presets();
                let size = match id {
                    MENU_ID_SIZE_S => presets[0],
                    MENU_ID_SIZE_M => presets[1],
                    MENU_ID_SIZE_L => presets[2],
                    _ => presets[3],
                };
                let size_target = match id {
                    MENU_ID_SIZE_S => "S",
                    MENU_ID_SIZE_M => "M",
                    MENU_ID_SIZE_L => "L",
                    _ => "XL",
                };
                self.telemetry_menu_action(MenuAction::SizeChange, Some(size_target));
                self.apply_size(size);
                self.save_settings();
            }
            MENU_ID_RESET => {
                self.telemetry_menu_action(MenuAction::Reset, None);
                self.reset_widget(event_loop);
                self.save_settings();
            }
            MENU_ID_QUIT => {
                self.telemetry_menu_action(MenuAction::Quit, None);
                self.save_settings();
                self.finish_session(SessionEndReason::QuitMenu);
                event_loop.exit();
            }
            MENU_ID_USAGE_ON => {
                self.settings.usage_data_sharing = true;
                self.telemetry.set_usage_enabled(true);
                self.telemetry_privacy_change("usage_data", true);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            MENU_ID_USAGE_OFF => {
                self.settings.usage_data_sharing = false;
                self.telemetry.set_usage_enabled(false);
                self.telemetry_privacy_change("usage_data", false);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            MENU_ID_CRASH_ON => {
                self.settings.crash_reports_sharing = true;
                self.telemetry.set_crash_enabled(true);
                self.telemetry_privacy_change("crash_reports", true);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            MENU_ID_CRASH_OFF => {
                self.settings.crash_reports_sharing = false;
                self.telemetry.set_crash_enabled(false);
                self.telemetry_privacy_change("crash_reports", false);
                self.sync_privacy_state_to_webview();
                self.sync_analytics_menu_state();
                self.save_settings();
            }
            MENU_ID_ANALYTICS_INFO => self.show_analytics_modal(event_loop),
            _ => {}
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.config_path = Self::config_path();
        let settings_exist = self.config_path.as_ref().is_some_and(|path| path.exists());
        self.settings = load_settings(self.config_path.as_deref());
        self.telemetry
            .set_usage_enabled(self.settings.usage_data_sharing);
        self.telemetry
            .set_crash_enabled(self.settings.crash_reports_sharing);
        if !settings_exist {
            if let Some(primary) = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())
            {
                self.settings.size = default_size_for_monitor(&primary);
            }
        }

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
            self.native_context_menu = NativeContextMenu::new();
        }
        self.sync_analytics_menu_state();
        self.telemetry.start_session(if self.settings.paused {
            ActivityState::Disabled
        } else {
            ActivityState::Active
        });
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
        self.save_settings();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) == self.telemetry_info_window_id {
            self.handle_telemetry_info_window_event(event);
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
                        eprintln!("warning: ignored malformed ipc command: {error}");
                        return;
                    }
                };
                self.handle_ipc_command(event_loop, command);
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

fn main() -> std::process::ExitCode {
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

    let event_loop = match EventLoop::<AppEvent>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("error: failed to create event loop: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let event_loop_proxy = event_loop.create_proxy();

    let ctrlc_proxy = event_loop_proxy.clone();
    if let Err(error) = ctrlc::set_handler(move || {
        let _ = ctrlc_proxy.send_event(AppEvent::ExitRequested);
    }) {
        eprintln!("warning: failed to install ctrl-c handler: {error}");
    }
    #[cfg(target_os = "macos")]
    {
        let menu_proxy = event_loop_proxy.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = menu_proxy.send_event(AppEvent::MenuActivated(event.id().as_ref().to_string()));
        }));
    }

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
        eprintln!("error: app event loop failed: {error}");
        return std::process::ExitCode::from(1);
    }
    if let Some(error) = app.startup_error {
        eprintln!("error: {error}");
        return std::process::ExitCode::from(1);
    }
    app.finish_session(SessionEndReason::Unknown);
    std::process::ExitCode::SUCCESS
}
