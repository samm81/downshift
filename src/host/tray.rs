use winit::event_loop::EventLoopProxy;

use super::menu::NativeContextMenu;
use crate::app_core::AppEvent;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::io::Cursor;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::{Icon, TrayIconBuilder};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ICON_ID: &str = "downshift";

#[cfg(any(target_os = "macos", target_os = "windows"))]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../docs/assets/icon.png");

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) type TrayIconHandle = tray_icon::TrayIcon;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) struct TrayIconHandle;

pub(crate) fn install_event_handler(proxy: EventLoopProxy<AppEvent>) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if matches!(
                event,
                tray_icon::TrayIconEvent::Click {
                    button_state: tray_icon::MouseButtonState::Down,
                    ..
                }
            ) {
                let _ = proxy.send_event(AppEvent::TrayIconClicked);
            }
        }));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = proxy;
}

pub(crate) fn create_tray_icon(
    menu: Option<&NativeContextMenu>,
) -> Result<Option<TrayIconHandle>, String> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let result = {
        let Some(menu) = menu else {
            return Ok(None);
        };
        let icon = load_tray_icon()?;
        let tray_icon = TrayIconBuilder::new()
            .with_id(TRAY_ICON_ID)
            .with_menu(menu.clone_for_tray())
            .with_tooltip("downshift")
            .with_icon(icon)
            .with_menu_on_left_click(true)
            .with_menu_on_right_click(true)
            .build()
            .map_err(|error| format!("failed to create tray icon: {error}"))?;
        Ok(Some(tray_icon))
    };

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let result = {
        let _ = menu;
        Ok(None)
    };

    result
}

pub(crate) fn update_menu(tray_icon: Option<&TrayIconHandle>, menu: Option<&NativeContextMenu>) {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if let (Some(tray_icon), Some(menu)) = (tray_icon, menu) {
        tray_icon.set_menu(Some(menu.clone_for_tray()));
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = (tray_icon, menu);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn load_tray_icon() -> Result<Icon, String> {
    let mut decoder = png::Decoder::new(Cursor::new(TRAY_ICON_PNG));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::ALPHA | png::Transformations::STRIP_16,
    );
    let mut reader = decoder
        .read_info()
        .map_err(|error| format!("failed to decode tray icon: {error}"))?;
    let mut rgba = vec![0; reader.output_buffer_size()];
    let output = reader
        .next_frame(&mut rgba)
        .map_err(|error| format!("failed to read tray icon: {error}"))?;
    if output.color_type != png::ColorType::Rgba || output.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "tray icon must decode to 8-bit RGBA, got {:?} {:?}",
            output.color_type, output.bit_depth
        ));
    }
    rgba.truncate(output.buffer_size());
    Icon::from_rgba(rgba, output.width, output.height)
        .map_err(|error| format!("failed to create tray icon image: {error}"))
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use super::load_tray_icon;

    #[test]
    fn embedded_tray_icon_decodes() {
        load_tray_icon().expect("embedded tray icon should decode");
    }
}
