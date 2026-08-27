use std::fmt;

use winit::window::Window;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordinateSpace {
    Physical,
    Logical,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
    pub space: CoordinateSpace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorError {
    Unavailable(&'static str),
    Query(String),
}

impl fmt::Display for CursorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => formatter.write_str(reason),
            Self::Query(error) => formatter.write_str(error),
        }
    }
}

pub trait CursorSource {
    fn sample(&mut self) -> Result<CursorPosition, CursorError>;

    fn is_supported(&self) -> bool;

    fn unavailable_reason(&self) -> &'static str;
}

pub struct CursorProvider {
    backend: Backend,
}

impl CursorProvider {
    pub fn for_window(window: &Window) -> Self {
        Self {
            backend: Backend::for_window(window),
        }
    }
}

impl CursorSource for CursorProvider {
    fn sample(&mut self) -> Result<CursorPosition, CursorError> {
        self.backend.sample()
    }

    fn is_supported(&self) -> bool {
        self.backend.is_supported()
    }

    fn unavailable_reason(&self) -> &'static str {
        self.backend.unavailable_reason()
    }
}

enum Backend {
    #[cfg(target_os = "macos")]
    Macos,
    #[cfg(target_os = "windows")]
    Windows,
    Unsupported(&'static str),
}

impl Backend {
    fn for_window(window: &Window) -> Self {
        #[cfg(target_os = "macos")]
        {
            let _ = window;
            return Self::Macos;
        }

        #[cfg(target_os = "windows")]
        {
            let _ = window;
            return Self::Windows;
        }

        #[cfg(target_os = "linux")]
        {
            use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};

            let display_handle = match window.display_handle() {
                Ok(handle) => handle,
                Err(_) => {
                    return Self::Unsupported(
                        "cursor following is unavailable on this display backend",
                    )
                }
            };
            return Self::Unsupported(match display_handle.as_raw() {
                RawDisplayHandle::Wayland(_) => {
                    "cursor following is unavailable on Wayland (global cursor position is not exposed)"
                }
                _ => "cursor following is unavailable on Linux (Linux is not currently supported)",
            });
        }

        #[allow(unreachable_code)]
        Self::Unsupported("cursor following is unavailable on this platform")
    }

    fn is_supported(&self) -> bool {
        !matches!(self, Self::Unsupported(_))
    }

    fn unavailable_reason(&self) -> &'static str {
        match self {
            Self::Unsupported(reason) => reason,
            #[cfg(target_os = "macos")]
            Self::Macos => "",
            #[cfg(target_os = "windows")]
            Self::Windows => "",
        }
    }

    fn sample(&mut self) -> Result<CursorPosition, CursorError> {
        match self {
            #[cfg(target_os = "macos")]
            Self::Macos => sample_macos_cursor(),
            #[cfg(target_os = "windows")]
            Self::Windows => sample_windows_cursor(),
            Self::Unsupported(reason) => Err(CursorError::Unavailable(reason)),
        }
    }
}

#[cfg(target_os = "macos")]
fn sample_macos_cursor() -> Result<CursorPosition, CursorError> {
    use objc2_app_kit::{NSEvent, NSScreen};
    use objc2_foundation::MainThreadMarker;

    let point = unsafe { NSEvent::mouseLocation() };
    let Some(mtm) = MainThreadMarker::new() else {
        return Err(CursorError::Query(
            "failed to obtain the macOS main-thread marker".to_string(),
        ));
    };
    let Some(main_screen) = NSScreen::mainScreen(mtm) else {
        return Err(CursorError::Query(
            "failed to resolve the macOS main screen".to_string(),
        ));
    };
    let main_screen_height = main_screen.frame().size.height;
    Ok(CursorPosition {
        x: point.x,
        y: main_screen_height - point.y,
        space: CoordinateSpace::Logical,
    })
}

#[cfg(target_os = "windows")]
fn sample_windows_cursor() -> Result<CursorPosition, CursorError> {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetPhysicalCursorPos;

    let mut point = POINT { x: 0, y: 0 };
    if unsafe { GetPhysicalCursorPos(&mut point) } == 0 {
        return Err(CursorError::Query(
            "GetPhysicalCursorPos failed".to_string(),
        ));
    }
    Ok(CursorPosition {
        x: f64::from(point.x),
        y: f64::from(point.y),
        space: CoordinateSpace::Physical,
    })
}
