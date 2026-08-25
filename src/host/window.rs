use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::window::Window;

pub(crate) fn logical_outer_position(window: Option<&Window>) -> Option<LogicalPosition<f64>> {
    let window = window?;
    let physical = window.outer_position().ok()?;
    Some(physical.to_logical(window.scale_factor()))
}

pub(crate) fn physical_outer_position(window: &Window) -> Option<PhysicalPosition<i32>> {
    window.outer_position().ok()
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

pub(crate) fn resize_preserving_center(
    window: &Window,
    target_dimensions: LogicalSize<f64>,
) -> Option<PhysicalPosition<i32>> {
    let old_dimensions = window.inner_size().to_logical::<f64>(window.scale_factor());
    window.set_min_inner_size(Some(target_dimensions));
    window.set_max_inner_size(Some(target_dimensions));
    let _ = window.request_inner_size(target_dimensions);

    let current_pos = logical_outer_position(Some(window))?;
    let center_x = current_pos.x + old_dimensions.width / 2.0;
    let center_y = current_pos.y + old_dimensions.height / 2.0;
    let next_x = (center_x - target_dimensions.width / 2.0).round() as i32;
    let next_y = (center_y - target_dimensions.height / 2.0).round() as i32;
    set_outer_position(window, LogicalPosition::new(next_x, next_y));
    physical_outer_position(window)
}
