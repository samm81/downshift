use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::window::Window;

pub(crate) fn logical_outer_position(window: Option<&Window>) -> Option<LogicalPosition<f64>> {
    let window = window?;
    let physical = window.outer_position().ok()?;
    Some(physical.to_logical(window.scale_factor()))
}

pub(crate) fn set_outer_position(window: &Window, position: LogicalPosition<i32>) {
    window.set_outer_position(position);
}

pub(crate) fn set_outer_position_physical(window: &Window, position: PhysicalPosition<i32>) {
    window.set_outer_position(position);
}

pub(crate) fn set_visible(window: &Window, visible: bool) {
    window.set_visible(visible);
}

pub(crate) fn show_without_focus(window: &Window) {
    window.set_visible(true);
}

pub(crate) fn enforce_fixed_size(window: &Window, target_dimensions: LogicalSize<f64>) {
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
