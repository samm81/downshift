use wry::application::dpi::LogicalSize;
use wry::application::event::{Event, WindowEvent};
use wry::application::event_loop::{ControlFlow, EventLoop};
use wry::application::window::WindowBuilder;
use wry::webview::WebViewBuilder;

const WINDOW_SIZE: f64 = 220.0;
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

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();

  let window = WindowBuilder::new()
    .with_title("breath-ball")
    .with_decorations(false)
    .with_transparent(true)
    .with_resizable(false)
    .with_inner_size(LogicalSize::new(WINDOW_SIZE, WINDOW_SIZE))
    .build(&event_loop)?;

  let _webview = WebViewBuilder::new(window)?
    .with_html(BREATH_HTML)?
    .with_transparent(true)
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit;
    }
  });
}
