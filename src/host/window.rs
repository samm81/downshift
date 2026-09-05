use super::platform::HostWindow;
use winit::dpi::{LogicalPosition, LogicalSize};

pub(crate) fn logical_outer_position(window: Option<&HostWindow>) -> Option<LogicalPosition<f64>> {
    let window = window?;
    let physical = window.outer_position().ok()?;
    Some(physical.to_logical(window.scale_factor()))
}

pub(crate) fn enforce_fixed_size(window: &HostWindow, target_dimensions: LogicalSize<f64>) {
    let current = window.inner_size().to_logical::<f64>(window.scale_factor());
    let width_mismatch = (current.width - target_dimensions.width).abs() > 0.5;
    let height_mismatch = (current.height - target_dimensions.height).abs() > 0.5;

    window.set_resizable(false);
    window.set_min_inner_size(Some(target_dimensions));
    window.set_max_inner_size(Some(target_dimensions));
    if width_mismatch || height_mismatch {
        let _ = window.request_inner_size(target_dimensions);
    }
}
