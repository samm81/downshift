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

    let window = event_loop
      .create_window(window_attributes)
      .expect("failed to create app window");

    let window_id = window.id();

    let webview = WebViewBuilder::new()
      .with_html(BREATH_HTML)
      .with_transparent(true)
      .build(&window)
      .expect("failed to create webview");

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

fn main() {
  let event_loop = EventLoop::<AppEvent>::with_user_event()
    .build()
    .expect("failed to create event loop");
  let event_loop_proxy = event_loop.create_proxy();

  ctrlc::set_handler(move || {
    let _ = event_loop_proxy.send_event(AppEvent::ExitRequested);
  })
  .expect("failed to install ctrl-c handler");

  let mut app = App::default();
  event_loop.run_app(&mut app).expect("failed to run app");
}
