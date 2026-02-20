use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};
use wry::{WebView, WebViewBuilder};

const WINDOW_SIZE: f64 = 220.0;

#[derive(Debug, Clone, Copy)]
enum AppEvent {
  ExitRequested,
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
      }
      body {
        display: grid;
        place-items: center;
        overflow: hidden;
      }
      .ball {
        width: 62%;
        aspect-ratio: 1 / 1;
        border-radius: 9999px;
        background: rgba(124, 182, 255, 0.55);
        box-shadow: inset 0 0 0 1px rgba(124, 182, 255, 0.35);
        transform: scale(0.65);
        animation: breathe 11s ease-in-out infinite alternate;
        transform-origin: center;
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
    <div class="ball"></div>
  </body>
</html>"#;

#[derive(Default)]
struct App {
  window: Option<Window>,
  window_id: Option<WindowId>,
  webview: Option<WebView>,
  startup_error: Option<String>,
}

impl ApplicationHandler<AppEvent> for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.window.is_some() {
      return;
    }

    let window_attributes = Window::default_attributes()
      .with_title("breath-ball")
      .with_decorations(false)
      .with_transparent(true)
      .with_resizable(false)
      .with_inner_size(LogicalSize::new(WINDOW_SIZE, WINDOW_SIZE));

    let window = match event_loop.create_window(window_attributes) {
      Ok(window) => window,
      Err(error) => {
        self.startup_error = Some(format!("failed to create app window: {error}"));
        event_loop.exit();
        return;
      }
    };

    let window_id = window.id();

    let webview_result = WebViewBuilder::new()
      .with_html(BREATH_HTML)
      .with_transparent(true)
      .build(&window);

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
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    window_id: WindowId,
    event: WindowEvent,
  ) {
    if Some(window_id) == self.window_id && matches!(event, WindowEvent::CloseRequested) {
      event_loop.exit();
    }
  }

  fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
    if matches!(event, AppEvent::ExitRequested) {
      event_loop.exit();
    }
  }
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
