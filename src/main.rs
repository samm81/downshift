use breath_ball::{
    apply_resize_step, clamp_size, load_settings, normalize_half_cycle, IpcCommand,
    PersistedMonitor, Settings, DEFAULT_HALF_CYCLE_SECONDS, DEFAULT_MARGIN, DEFAULT_SIZE,
};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId, WindowLevel};
use wry::{Rect, WebView, WebViewBuilder};

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
        width: 74%;
        aspect-ratio: 1 / 1;
        border-radius: 9999px;
        background: rgba(124, 182, 255, 0.52);
        box-shadow: inset 0 0 0 1px rgba(124, 182, 255, 0.35);
        transform: scale(0.65);
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
          transform: scale(0.65);
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
        <div class="label">speed</div>
        <button data-speed="4.5">fast (4.5 / 4.5)</button>
        <button data-speed="5.5">default (5.5 / 5.5)</button>
        <button data-speed="6.5">slow (6.5 / 6.5)</button>
      </div>
      <div class="divider"></div>
      <div class="group">
        <div class="label">size</div>
        <button data-size="24">S (24px)</button>
        <button data-size="32">M (32px)</button>
        <button data-size="48">L (48px)</button>
        <button data-size="64">XL (64px)</button>
      </div>
      <div class="divider"></div>
      <button id="menu-reset">reset</button>
      <button id="menu-quit">quit</button>
    </div>
    <script>
      (() => {
        const ball = document.getElementById("ball");
        const menu = document.getElementById("menu");
        const pauseButton = document.getElementById("menu-pause");
        const resetButton = document.getElementById("menu-reset");
        const quitButton = document.getElementById("menu-quit");
        const speedButtons = Array.from(document.querySelectorAll("[data-speed]"));
        const sizeButtons = Array.from(document.querySelectorAll("[data-size]"));
        const init = window.__BB_INIT__ || { paused: false, half_cycle_seconds: 5.5 };
        const state = {
          paused: Boolean(init.paused),
          halfCycleSeconds: Number(init.half_cycle_seconds) || 5.5,
        };

        function post(payload) {
          if (window.ipc && typeof window.ipc.postMessage === "function") {
            window.ipc.postMessage(JSON.stringify(payload));
          }
        }

        function hideMenu() {
          menu.hidden = true;
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
          applyBallState();
        };

        ball.addEventListener("wheel", (event) => {
          event.preventDefault();
          const direction = event.deltaY < 0 ? 1 : -1;
          post({ cmd: "resize", delta: direction, fine: event.shiftKey });
        }, { passive: false });

        ball.addEventListener("contextmenu", (event) => {
          event.preventDefault();
          applyBallState();
          showMenu(event.clientX, event.clientY);
        });

        pauseButton.addEventListener("click", () => {
          state.paused = !state.paused;
          applyBallState();
          post({ cmd: "set_paused", paused: state.paused });
          hideMenu();
        });

        speedButtons.forEach((button) => {
          button.addEventListener("click", () => {
            const half = Number(button.dataset.speed);
            if (!Number.isFinite(half) || half <= 0) return;
            state.halfCycleSeconds = half;
            applyBallState();
            post({ cmd: "set_speed", half_cycle_seconds: half });
            hideMenu();
          });
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

        document.addEventListener("mousedown", (event) => {
          if (!menu.hidden && !menu.contains(event.target)) {
            hideMenu();
          }
        });

        document.addEventListener("blur", hideMenu);
        window.addEventListener("resize", hideMenu);

        ball.addEventListener("pointerdown", (event) => {
          if (event.button !== 0) return;
          post({ cmd: "start_drag" });
        });

        applyBallState();
      })();
    </script>
  </body>
</html>"#;

#[derive(Debug, Clone)]
enum AppEvent {
    ExitRequested,
    Ipc(String),
}

#[derive(Default)]
struct App {
    window: Option<Window>,
    window_id: Option<WindowId>,
    webview: Option<WebView>,
    startup_error: Option<String>,
    settings: Settings,
    config_path: Option<std::path::PathBuf>,
    event_loop_proxy: Option<EventLoopProxy<AppEvent>>,
}

impl App {
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

    fn choose_initial_position(
        &self,
        event_loop: &ActiveEventLoop,
        size: f64,
    ) -> Option<LogicalPosition<f64>> {
        let monitors: Vec<_> = event_loop.available_monitors().collect();
        if monitors.is_empty() {
            return None;
        }
        if let (Some(saved_x), Some(saved_y)) = (self.settings.x, self.settings.y) {
            let saved = LogicalPosition::new(saved_x as f64, saved_y as f64);
            if monitors
                .iter()
                .any(|monitor| position_fits_monitor(saved, size, monitor))
            {
                return Some(saved);
            }
        }
        let primary = event_loop
            .primary_monitor()
            .or_else(|| monitors.first().cloned())?;
        Some(default_corner_position(&primary, size))
    }

    fn build_init_script(&self) -> String {
        let payload = serde_json::json!({
          "paused": self.settings.paused,
          "half_cycle_seconds": self.settings.half_cycle_seconds,
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
            self.settings.monitor = window.current_monitor().map(snapshot_monitor);
        }
    }

    fn reset_widget(&mut self, event_loop: &ActiveEventLoop) {
        self.apply_size(DEFAULT_SIZE);
        self.apply_half_cycle(DEFAULT_HALF_CYCLE_SECONDS);
        self.apply_paused(false);
        if let Some(window) = self.window.as_ref() {
            let monitor = window
                .current_monitor()
                .or_else(|| event_loop.primary_monitor())
                .or_else(|| event_loop.available_monitors().next());
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
                self.save_settings();
                event_loop.exit();
            }
            IpcCommand::SetPaused { paused } => {
                self.apply_paused(paused);
                self.save_settings();
            }
            IpcCommand::SetSpeed { half_cycle_seconds } => {
                self.apply_half_cycle(half_cycle_seconds);
                self.save_settings();
            }
            IpcCommand::Resize { delta, fine } => {
                let next = apply_resize_step(self.settings.size, delta, fine);
                self.apply_size(next);
                self.save_settings();
            }
            IpcCommand::SetSize { size } => {
                self.apply_size(size);
                self.save_settings();
            }
            IpcCommand::StartDrag => {
                if let Some(window) = self.window.as_ref() {
                    if let Err(error) = window.drag_window() {
                        eprintln!("warning: failed to start window drag: {error}");
                    }
                }
            }
            IpcCommand::Reset => {
                self.reset_widget(event_loop);
                self.save_settings();
            }
        }
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.config_path = Self::config_path();
        self.settings = load_settings(self.config_path.as_deref());

        let mut window_attributes = Window::default_attributes()
            .with_title("downshift")
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
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
                self.startup_error = Some(format!("failed to create app window: {error}"));
                event_loop.exit();
                return;
            }
        };
        let window_id = window.id();
        self.settings.monitor = window.current_monitor().map(snapshot_monitor);

        let init_script = self.build_init_script();
        let Some(ipc_proxy) = self.event_loop_proxy.as_ref().cloned() else {
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
                self.startup_error = Some(format!("failed to create webview: {error}"));
                event_loop.exit();
                return;
            }
        };

        self.window = Some(window);
        self.window_id = Some(window_id);
        self.webview = Some(webview);
        self.sync_webview_bounds();
        self.save_settings();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if Some(window_id) != self.window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                self.save_settings();
                event_loop.exit();
            }
            WindowEvent::Moved(position) => {
                self.update_position_from_physical(position);
                self.save_settings();
            }
            WindowEvent::Resized(_) => {
                self.sync_webview_bounds();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ExitRequested => {
                self.save_settings();
                event_loop.exit();
            }
            AppEvent::Ipc(payload) => {
                let command = match serde_json::from_str::<IpcCommand>(&payload) {
                    Ok(command) => command,
                    Err(error) => {
                        eprintln!("warning: ignored malformed ipc command: {error}");
                        return;
                    }
                };
                self.handle_ipc_command(event_loop, command);
            }
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
    LogicalPosition::new(
        monitor_pos.x + monitor_size.width - size - DEFAULT_MARGIN,
        monitor_pos.y + DEFAULT_MARGIN,
    )
}

fn main() -> std::process::ExitCode {
    let event_loop = match EventLoop::<AppEvent>::with_user_event().build() {
        Ok(event_loop) => event_loop,
        Err(error) => {
            eprintln!("error: failed to create event loop: {error}");
            return std::process::ExitCode::from(1);
        }
    };
    let event_loop_proxy = event_loop.create_proxy();

    if let Err(error) = ctrlc::set_handler(move || {
        let _ = event_loop_proxy.send_event(AppEvent::ExitRequested);
    }) {
        eprintln!("warning: failed to install ctrl-c handler: {error}");
    }

    let mut app = App::default();
    app.event_loop_proxy = Some(event_loop.create_proxy());

    if let Err(error) = event_loop.run_app(&mut app) {
        eprintln!("error: app event loop failed: {error}");
        return std::process::ExitCode::from(1);
    }
    if let Some(error) = app.startup_error {
        eprintln!("error: {error}");
        return std::process::ExitCode::from(1);
    }
    std::process::ExitCode::SUCCESS
}
