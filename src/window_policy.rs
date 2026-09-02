use downshift::{
    clamp_size, LinuxOutputPlacement, LinuxWindowAnchor, LinuxWindowMode, PersistedMonitor,
    Settings, DEFAULT_SIZE,
};
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::monitor::MonitorHandle;

use crate::app_core::{
    DEFAULT_EDGE_MARGIN_RATIO, DEFAULT_SIZE_SHORT_SIDE_RATIO, SIZE_PRESET_RATIOS,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PhysicalPoint {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl PhysicalPoint {
    pub(crate) const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LogicalPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

impl LogicalPoint {
    pub(crate) const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// These values are CSS logical units. The embedded webview uses a 16-unit
// root font size for one rem, and winit applies the monitor scale factor when
// converting the resulting logical geometry to physical pixels.
pub(crate) const CSS_ROOT_FONT_SIZE_LOGICAL: f64 = 16.0;
pub(crate) const FOLLOW_CURSOR_OFFSET_REM: f64 = 0.25;
pub(crate) const FOLLOW_CURSOR_OFFSET_LOGICAL: f64 =
    FOLLOW_CURSOR_OFFSET_REM * CSS_ROOT_FONT_SIZE_LOGICAL;
pub(crate) const FOLLOW_CURSOR_ARTWORK_SIZE_REM: f64 = 3.0;
pub(crate) const FOLLOW_CURSOR_ARTWORK_SIZE_LOGICAL: f64 =
    FOLLOW_CURSOR_ARTWORK_SIZE_REM * CSS_ROOT_FONT_SIZE_LOGICAL;
pub(crate) const FOLLOW_CURSOR_HALO_SIZE_REM: f64 = 3.5;
pub(crate) const FOLLOW_CURSOR_HALO_SIZE_LOGICAL: f64 =
    FOLLOW_CURSOR_HALO_SIZE_REM * CSS_ROOT_FONT_SIZE_LOGICAL;
pub(crate) const FOLLOW_CURSOR_WINDOW_SIZE_REM: f64 = 4.0;
pub(crate) const FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL: f64 =
    FOLLOW_CURSOR_WINDOW_SIZE_REM * CSS_ROOT_FONT_SIZE_LOGICAL;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScreenRect {
    pub(crate) left: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
}

impl ScreenRect {
    pub(crate) fn from_monitor(monitor: &MonitorHandle) -> Self {
        let position = monitor.position();
        let size = monitor.size();
        Self {
            left: position.x,
            top: position.y,
            right: position
                .x
                .saturating_add(size.width.min(i32::MAX as u32) as i32),
            bottom: position
                .y
                .saturating_add(size.height.min(i32::MAX as u32) as i32),
        }
    }

    pub(crate) fn contains(self, point: PhysicalPosition<i32>) -> bool {
        point.x >= self.left && point.x < self.right && point.y >= self.top && point.y < self.bottom
    }

    pub(crate) fn clamp_window_origin(
        self,
        desired_x: i32,
        desired_y: i32,
        width: i32,
        height: i32,
    ) -> PhysicalPosition<i32> {
        let max_x = self.right.saturating_sub(width).max(self.left);
        let max_y = self.bottom.saturating_sub(height).max(self.top);
        PhysicalPosition::new(
            desired_x.clamp(self.left, max_x),
            desired_y.clamp(self.top, max_y),
        )
    }
}

pub(crate) fn follow_window_origin(
    cursor: PhysicalPosition<i32>,
    work_area: ScreenRect,
    scale_factor: f64,
    window_dimensions: LogicalSize<f64>,
    anchor_center: LogicalPosition<f64>,
) -> PhysicalPosition<i32> {
    let scale_factor = if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
    };
    let width = (window_dimensions.width.max(1.0) * scale_factor)
        .round()
        .max(1.0) as i32;
    let height = (window_dimensions.height.max(1.0) * scale_factor)
        .round()
        .max(1.0) as i32;
    let anchor_center_x = (anchor_center.x.max(0.0) * scale_factor).round() as i32;
    let anchor_center_y = (anchor_center.y.max(0.0) * scale_factor).round() as i32;
    let offset = (FOLLOW_CURSOR_OFFSET_LOGICAL * scale_factor)
        .round()
        .max(0.0) as i32;
    let desired_x = cursor
        .x
        .saturating_add(offset)
        .saturating_sub(anchor_center_x);
    let desired_y = cursor
        .y
        .saturating_add(offset)
        .saturating_sub(anchor_center_y);
    work_area.clamp_window_origin(desired_x, desired_y, width, height)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MonitorSnapshot {
    pub(crate) position: PhysicalPoint,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) scale_factor: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LinuxOutputSnapshot {
    pub(crate) name: Option<String>,
    pub(crate) monitor: MonitorSnapshot,
    pub(crate) primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxSessionBackend {
    X11,
    Wayland,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinuxWindowBackend {
    X11,
    WaylandNormal,
    WaylandLayerShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LinuxWindowDecision {
    pub(crate) backend: LinuxWindowBackend,
    pub(crate) overlay_supported: bool,
    pub(crate) fallback_reason: Option<&'static str>,
}

pub(crate) fn choose_linux_window_backend(
    session: LinuxSessionBackend,
    requested: LinuxWindowMode,
    layer_shell_supported: bool,
) -> LinuxWindowDecision {
    match session {
        LinuxSessionBackend::X11 => LinuxWindowDecision {
            backend: LinuxWindowBackend::X11,
            overlay_supported: false,
            fallback_reason: (requested == LinuxWindowMode::Overlay).then_some(
                "layer-shell overlay is only available on Wayland; using X11 utility window",
            ),
        },
        LinuxSessionBackend::Wayland => {
            let can_use_overlay =
                requested != LinuxWindowMode::NormalWindow && layer_shell_supported;
            LinuxWindowDecision {
                backend: if can_use_overlay {
                    LinuxWindowBackend::WaylandLayerShell
                } else {
                    LinuxWindowBackend::WaylandNormal
                },
                overlay_supported: layer_shell_supported,
                fallback_reason: if requested == LinuxWindowMode::NormalWindow {
                    None
                } else if !layer_shell_supported {
                    Some("gtk-layer-shell is unavailable; using a regular Wayland window")
                } else {
                    None
                },
            }
        }
        LinuxSessionBackend::Unknown => LinuxWindowDecision {
            backend: LinuxWindowBackend::WaylandNormal,
            overlay_supported: false,
            fallback_reason: Some("Linux display backend is unknown; using a regular window"),
        },
    }
}

pub(crate) fn linux_output_index_for_placement(
    placement: &LinuxOutputPlacement,
    outputs: &[LinuxOutputSnapshot],
) -> Option<usize> {
    if let Some(name) = placement.output_name.as_deref() {
        if let Some(index) = outputs
            .iter()
            .position(|output| output.name.as_deref() == Some(name))
        {
            return Some(index);
        }
    }
    if let Some(index) = outputs
        .iter()
        .position(|output| monitor_matches_persisted(&output.monitor, &placement.output))
    {
        return Some(index);
    }
    outputs.iter().position(|output| output.primary)
}

pub(crate) fn linux_output_origin_for_placement(
    placement: &LinuxOutputPlacement,
    output: &LinuxOutputSnapshot,
    window_width: u32,
    window_height: u32,
) -> PhysicalPoint {
    let monitor = output.monitor;
    let width = window_width.min(i32::MAX as u32) as i32;
    let height = window_height.min(i32::MAX as u32) as i32;
    let margin_x = placement.margin_x;
    let margin_y = placement.margin_y;
    let desired_x = match placement.anchor {
        LinuxWindowAnchor::TopLeft | LinuxWindowAnchor::BottomLeft => {
            monitor.position.x.saturating_add(margin_x)
        }
        LinuxWindowAnchor::TopRight | LinuxWindowAnchor::BottomRight => monitor
            .position
            .x
            .saturating_add(monitor.width.min(i32::MAX as u32) as i32)
            .saturating_sub(width)
            .saturating_sub(margin_x),
    };
    let desired_y = match placement.anchor {
        LinuxWindowAnchor::TopLeft | LinuxWindowAnchor::TopRight => {
            monitor.position.y.saturating_add(margin_y)
        }
        LinuxWindowAnchor::BottomLeft | LinuxWindowAnchor::BottomRight => monitor
            .position
            .y
            .saturating_add(monitor.height.min(i32::MAX as u32) as i32)
            .saturating_sub(height)
            .saturating_sub(margin_y),
    };
    PhysicalPoint::new(desired_x, desired_y)
}

pub(crate) fn linux_output_placement_for_origin(
    output: &LinuxOutputSnapshot,
    origin: PhysicalPoint,
    window_width: u32,
    window_height: u32,
) -> LinuxOutputPlacement {
    let monitor = output.monitor;
    let width = window_width.min(i32::MAX as u32) as i32;
    let height = window_height.min(i32::MAX as u32) as i32;
    let clamped = ScreenRect {
        left: monitor.position.x,
        top: monitor.position.y,
        right: monitor
            .position
            .x
            .saturating_add(monitor.width.min(i32::MAX as u32) as i32),
        bottom: monitor
            .position
            .y
            .saturating_add(monitor.height.min(i32::MAX as u32) as i32),
    }
    .clamp_window_origin(origin.x, origin.y, width, height);
    let right =
        clamped.x + width / 2 >= monitor.position.x + monitor.width.min(i32::MAX as u32) as i32 / 2;
    let bottom = clamped.y + height / 2
        >= monitor.position.y + monitor.height.min(i32::MAX as u32) as i32 / 2;
    linux_output_placement_for_position(
        output,
        PhysicalPoint::new(clamped.x, clamped.y),
        width,
        height,
        match (right, bottom) {
            (false, false) => LinuxWindowAnchor::TopLeft,
            (true, false) => LinuxWindowAnchor::TopRight,
            (false, true) => LinuxWindowAnchor::BottomLeft,
            (true, true) => LinuxWindowAnchor::BottomRight,
        },
    )
}

pub(crate) fn linux_output_placement_for_origin_with_anchor(
    output: &LinuxOutputSnapshot,
    origin: PhysicalPoint,
    window_width: u32,
    window_height: u32,
    anchor: LinuxWindowAnchor,
) -> LinuxOutputPlacement {
    let width = window_width.min(i32::MAX as u32) as i32;
    let height = window_height.min(i32::MAX as u32) as i32;
    linux_output_placement_for_position(output, origin, width, height, anchor)
}

fn linux_output_placement_for_position(
    output: &LinuxOutputSnapshot,
    origin: PhysicalPoint,
    width: i32,
    height: i32,
    anchor: LinuxWindowAnchor,
) -> LinuxOutputPlacement {
    let monitor = output.monitor;
    let right = matches!(
        anchor,
        LinuxWindowAnchor::TopRight | LinuxWindowAnchor::BottomRight
    );
    let bottom = matches!(
        anchor,
        LinuxWindowAnchor::BottomLeft | LinuxWindowAnchor::BottomRight
    );
    LinuxOutputPlacement {
        output_name: output.name.clone(),
        output: monitor.persisted(),
        anchor,
        margin_x: if right {
            monitor
                .position
                .x
                .saturating_add(monitor.width.min(i32::MAX as u32) as i32)
                .saturating_sub(origin.x.saturating_add(width))
        } else {
            origin.x.saturating_sub(monitor.position.x)
        },
        margin_y: if bottom {
            monitor
                .position
                .y
                .saturating_add(monitor.height.min(i32::MAX as u32) as i32)
                .saturating_sub(origin.y.saturating_add(height))
        } else {
            origin.y.saturating_sub(monitor.position.y)
        },
    }
}

pub(crate) fn linux_drag_delta(delta: LogicalPoint, scale_factor: f64) -> Option<PhysicalPoint> {
    if !delta.x.is_finite()
        || !delta.y.is_finite()
        || !scale_factor.is_finite()
        || scale_factor <= 0.0
    {
        return None;
    }
    let x = (delta.x * scale_factor).round();
    let y = (delta.y * scale_factor).round();
    if !x.is_finite() || !y.is_finite() {
        return None;
    }
    Some(PhysicalPoint::new(
        x.clamp(i32::MIN as f64, i32::MAX as f64) as i32,
        y.clamp(i32::MIN as f64, i32::MAX as f64) as i32,
    ))
}

impl MonitorSnapshot {
    pub(crate) fn persisted(self) -> PersistedMonitor {
        PersistedMonitor {
            width: self.width,
            height: self.height,
            scale_factor: self.scale_factor,
        }
    }

    pub(crate) fn contains_logical(self, point: LogicalPoint) -> bool {
        if !point.x.is_finite()
            || !point.y.is_finite()
            || !self.scale_factor.is_finite()
            || self.scale_factor <= 0.0
        {
            return false;
        }
        let origin_x = f64::from(self.position.x) / self.scale_factor;
        let origin_y = f64::from(self.position.y) / self.scale_factor;
        let width = f64::from(self.width) / self.scale_factor;
        let height = f64::from(self.height) / self.scale_factor;
        point.x >= origin_x
            && point.x < origin_x + width
            && point.y >= origin_y
            && point.y < origin_y + height
    }
}

pub(crate) fn logical_cursor_to_physical(
    point: LogicalPoint,
    monitor: &MonitorSnapshot,
) -> Option<PhysicalPoint> {
    if !point.x.is_finite()
        || !point.y.is_finite()
        || !monitor.scale_factor.is_finite()
        || monitor.scale_factor <= 0.0
    {
        return None;
    }
    let origin_x = f64::from(monitor.position.x) / monitor.scale_factor;
    let origin_y = f64::from(monitor.position.y) / monitor.scale_factor;
    let relative_x = ((point.x - origin_x) * monitor.scale_factor).round();
    let relative_y = ((point.y - origin_y) * monitor.scale_factor).round();
    if !relative_x.is_finite() || !relative_y.is_finite() {
        return None;
    }
    Some(PhysicalPoint::new(
        monitor.position.x.saturating_add(relative_x as i32),
        monitor.position.y.saturating_add(relative_y as i32),
    ))
}

pub(crate) fn monitor_matches_persisted(
    monitor: &MonitorSnapshot,
    persisted: &PersistedMonitor,
) -> bool {
    monitor.width == persisted.width
        && monitor.height == persisted.height
        && (monitor.scale_factor - persisted.scale_factor).abs() < 0.01
}

pub(crate) fn physical_size_for_monitor(size: f64, monitor: &MonitorSnapshot) -> i32 {
    (size * monitor.scale_factor).round() as i32
}

pub(crate) fn position_fits_monitor(
    position: PhysicalPoint,
    size: f64,
    monitor: &MonitorSnapshot,
) -> bool {
    let window_size = physical_size_for_monitor(size, monitor);
    let max_x = i64::from(monitor.position.x) + i64::from(monitor.width) - i64::from(window_size);
    let max_y = i64::from(monitor.position.y) + i64::from(monitor.height) - i64::from(window_size);

    i64::from(position.x) >= i64::from(monitor.position.x)
        && i64::from(position.y) >= i64::from(monitor.position.y)
        && i64::from(position.x) <= max_x
        && i64::from(position.y) <= max_y
}

pub(crate) fn position_fits_monitor_legacy(
    position: LogicalPoint,
    size: f64,
    monitor: &MonitorSnapshot,
) -> bool {
    let monitor_pos = LogicalPoint::new(
        f64::from(monitor.position.x) / monitor.scale_factor,
        f64::from(monitor.position.y) / monitor.scale_factor,
    );
    let monitor_size = LogicalPoint::new(
        f64::from(monitor.width) / monitor.scale_factor,
        f64::from(monitor.height) / monitor.scale_factor,
    );
    let max_x = monitor_pos.x + monitor_size.x - size;
    let max_y = monitor_pos.y + monitor_size.y - size;
    position.x >= monitor_pos.x
        && position.y >= monitor_pos.y
        && position.x <= max_x
        && position.y <= max_y
}

pub(crate) fn default_corner_position(monitor: &MonitorSnapshot, size: f64) -> PhysicalPoint {
    let margin =
        (f64::from(monitor.width.min(monitor.height)) * DEFAULT_EDGE_MARGIN_RATIO).round() as i32;
    let window_size = physical_size_for_monitor(size, monitor);
    PhysicalPoint::new(
        monitor.position.x + monitor.width as i32 - window_size - margin,
        monitor.position.y + margin,
    )
}

pub(crate) fn default_size_for_monitor(monitor: &MonitorSnapshot) -> f64 {
    clamp_size(
        (f64::from(monitor.width.min(monitor.height)) / monitor.scale_factor)
            * DEFAULT_SIZE_SHORT_SIDE_RATIO,
    )
}

pub(crate) fn logical_to_physical_position(
    position: LogicalPoint,
    scale_factor: f64,
) -> PhysicalPoint {
    PhysicalPoint::new(
        (position.x * scale_factor).round() as i32,
        (position.y * scale_factor).round() as i32,
    )
}

pub(crate) fn size_presets_for_monitor(monitor: &MonitorSnapshot) -> [f64; 4] {
    let short_side_logical = f64::from(monitor.width.min(monitor.height)) / monitor.scale_factor;
    std::array::from_fn(|index| {
        clamp_size((short_side_logical * SIZE_PRESET_RATIOS[index]).round())
    })
}

pub(crate) fn choose_initial_position(
    settings: &Settings,
    monitors: &[MonitorSnapshot],
    primary: &MonitorSnapshot,
    size: f64,
) -> PhysicalPoint {
    if let (Some(saved_x), Some(saved_y)) = (settings.physical_x, settings.physical_y) {
        let saved = PhysicalPoint::new(saved_x, saved_y);
        if let Some(saved_monitor) = settings.monitor.as_ref() {
            if let Some(current) = monitors
                .iter()
                .find(|monitor| monitor_matches_persisted(monitor, saved_monitor))
            {
                if position_fits_monitor(saved, size, current) {
                    return saved;
                }
            } else {
                // Display config changed (for example resolution), so reuse corner-relative spawn.
                return default_corner_position(primary, size);
            }
        } else if monitors
            .iter()
            .any(|monitor| position_fits_monitor(saved, size, monitor))
        {
            return saved;
        }
    }

    if let (Some(saved_x), Some(saved_y)) = (settings.x, settings.y) {
        let saved = LogicalPoint::new(saved_x as f64, saved_y as f64);
        if let Some(saved_monitor) = settings.monitor.as_ref() {
            if let Some(current) = monitors
                .iter()
                .find(|monitor| monitor_matches_persisted(monitor, saved_monitor))
            {
                if position_fits_monitor_legacy(saved, size, current) {
                    return logical_to_physical_position(saved, current.scale_factor);
                }
            } else {
                return default_corner_position(primary, size);
            }
        } else if let Some(current) = monitors
            .iter()
            .find(|monitor| position_fits_monitor_legacy(saved, size, monitor))
        {
            return logical_to_physical_position(saved, current.scale_factor);
        }
    }

    default_corner_position(primary, size)
}

pub(crate) fn reset_size_for_monitor(monitor: Option<&MonitorSnapshot>) -> f64 {
    monitor
        .map(default_size_for_monitor)
        .unwrap_or(DEFAULT_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> MonitorSnapshot {
        MonitorSnapshot {
            position: PhysicalPoint::new(0, 0),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        }
    }

    #[test]
    fn size_presets_keep_legacy_minimums_on_small_outputs() {
        assert_eq!(
            size_presets_for_monitor(&monitor()),
            [86.0, 108.0, 140.0, 173.0]
        );
    }

    #[test]
    fn default_position_is_in_the_monitor_corner() {
        let position = default_corner_position(&monitor(), 96.0);
        assert_eq!(position, PhysicalPoint::new(1770, 54));
        assert!(position_fits_monitor(position, 96.0, &monitor()));
    }

    #[test]
    fn initial_position_prefers_a_valid_saved_physical_position() {
        let mut settings = Settings::default();
        settings.physical_x = Some(100);
        settings.physical_y = Some(200);
        settings.monitor = Some(monitor().persisted());

        assert_eq!(
            choose_initial_position(&settings, &[monitor()], &monitor(), 96.0),
            PhysicalPoint::new(100, 200)
        );
    }

    #[test]
    fn drag_and_legacy_position_math_remain_numeric_only() {
        assert!(position_fits_monitor_legacy(
            LogicalPoint::new(10.0, 20.0),
            96.0,
            &monitor()
        ));
        assert_eq!(
            crate::app_core::drag_position((100.0, 200.0), (10.0, 20.0), (25.0, 5.0)),
            (115, 185)
        );
    }

    #[test]
    fn logical_cursor_conversion_tracks_scaled_multi_monitor_layouts() {
        let left = MonitorSnapshot {
            position: PhysicalPoint::new(-1920, 0),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        };
        let right = MonitorSnapshot {
            position: PhysicalPoint::new(0, 0),
            width: 3840,
            height: 2160,
            scale_factor: 2.0,
        };

        let left_cursor = LogicalPoint::new(-960.0, 540.0);
        assert!(left.contains_logical(left_cursor));
        assert_eq!(
            logical_cursor_to_physical(left_cursor, &left),
            Some(PhysicalPoint::new(-960, 540))
        );

        let right_cursor = LogicalPoint::new(960.0, 540.0);
        assert!(!left.contains_logical(right_cursor));
        assert!(right.contains_logical(right_cursor));
        assert_eq!(
            logical_cursor_to_physical(right_cursor, &right),
            Some(PhysicalPoint::new(1920, 1080))
        );
    }

    #[test]
    fn follow_window_origin_centers_halo_on_cursor() {
        let origin = follow_window_origin(
            PhysicalPosition::new(500, 200),
            ScreenRect {
                left: 0,
                top: 0,
                right: 1000,
                bottom: 800,
            },
            1.0,
            LogicalSize::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
            ),
            LogicalPosition::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
            ),
        );

        assert_eq!(origin, PhysicalPosition::new(472, 172));
    }

    #[test]
    fn follow_window_origin_clamps_halo_to_monitor_edges() {
        let origin = follow_window_origin(
            PhysicalPosition::new(990, 790),
            ScreenRect {
                left: 0,
                top: 0,
                right: 1000,
                bottom: 800,
            },
            1.0,
            LogicalSize::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
            ),
            LogicalPosition::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
            ),
        );

        assert_eq!(origin, PhysicalPosition::new(936, 736));
    }

    #[test]
    fn follow_window_origin_clamps_to_negative_monitor_edges() {
        let origin = follow_window_origin(
            PhysicalPosition::new(-1910, -500),
            ScreenRect {
                left: -1920,
                top: -1080,
                right: 0,
                bottom: 0,
            },
            1.0,
            LogicalSize::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
            ),
            LogicalPosition::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
            ),
        );

        assert_eq!(origin, PhysicalPosition::new(-1920, -528));
    }

    #[test]
    fn follow_window_origin_scales_rem_halo_geometry() {
        let origin = follow_window_origin(
            PhysicalPosition::new(1000, 400),
            ScreenRect {
                left: 0,
                top: 0,
                right: 2000,
                bottom: 1600,
            },
            2.0,
            LogicalSize::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL,
            ),
            LogicalPosition::new(
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
                FOLLOW_CURSOR_WINDOW_SIZE_LOGICAL / 2.0,
            ),
        );

        assert_eq!(origin, PhysicalPosition::new(944, 344));
    }

    fn output(name: &str, position: PhysicalPoint, primary: bool) -> LinuxOutputSnapshot {
        LinuxOutputSnapshot {
            name: Some(name.to_string()),
            monitor: MonitorSnapshot {
                position,
                width: 1920,
                height: 1080,
                scale_factor: 1.0,
            },
            primary,
        }
    }

    #[test]
    fn linux_backend_selection_uses_layer_shell_when_supported() {
        let decision =
            choose_linux_window_backend(LinuxSessionBackend::Wayland, LinuxWindowMode::Auto, true);
        assert_eq!(decision.backend, LinuxWindowBackend::WaylandLayerShell);
        assert!(decision.fallback_reason.is_none());

        let decision =
            choose_linux_window_backend(LinuxSessionBackend::Wayland, LinuxWindowMode::Auto, false);
        assert_eq!(decision.backend, LinuxWindowBackend::WaylandNormal);
        assert_eq!(
            decision.fallback_reason,
            Some("gtk-layer-shell is unavailable; using a regular Wayland window")
        );

        let decision = choose_linux_window_backend(
            LinuxSessionBackend::Wayland,
            LinuxWindowMode::Overlay,
            true,
        );
        assert_eq!(decision.backend, LinuxWindowBackend::WaylandLayerShell);
        assert!(decision.fallback_reason.is_none());

        let decision = choose_linux_window_backend(
            LinuxSessionBackend::Wayland,
            LinuxWindowMode::Overlay,
            true,
        );
        assert_eq!(decision.backend, LinuxWindowBackend::WaylandLayerShell);
        assert!(decision.fallback_reason.is_none());

        let decision = choose_linux_window_backend(
            LinuxSessionBackend::Wayland,
            LinuxWindowMode::NormalWindow,
            true,
        );
        assert_eq!(decision.backend, LinuxWindowBackend::WaylandNormal);
        assert!(decision.fallback_reason.is_none());
    }

    #[test]
    fn linux_x11_and_unknown_backends_keep_a_safe_normal_window() {
        let x11 =
            choose_linux_window_backend(LinuxSessionBackend::X11, LinuxWindowMode::Auto, true);
        assert_eq!(x11.backend, LinuxWindowBackend::X11);
        assert!(x11.fallback_reason.is_none());

        let unknown = choose_linux_window_backend(
            LinuxSessionBackend::Unknown,
            LinuxWindowMode::Overlay,
            true,
        );
        assert_eq!(unknown.backend, LinuxWindowBackend::WaylandNormal);
        assert!(unknown.fallback_reason.is_some());
    }

    #[test]
    fn linux_output_matching_prefers_name_then_geometry_then_primary() {
        let mut outputs = vec![
            output("DP-1", PhysicalPoint::new(0, 0), true),
            output("HDMI-1", PhysicalPoint::new(1920, 0), false),
        ];
        outputs[1].monitor.width = 2560;
        let by_name = LinuxOutputPlacement {
            output_name: Some("HDMI-1".to_string()),
            output: outputs[0].monitor.persisted(),
            anchor: LinuxWindowAnchor::TopRight,
            margin_x: 24,
            margin_y: 24,
        };
        assert_eq!(
            linux_output_index_for_placement(&by_name, &outputs),
            Some(1)
        );

        let by_geometry = LinuxOutputPlacement {
            output_name: Some("missing".to_string()),
            output: outputs[1].monitor.persisted(),
            anchor: LinuxWindowAnchor::TopRight,
            margin_x: 24,
            margin_y: 24,
        };
        assert_eq!(
            linux_output_index_for_placement(&by_geometry, &outputs),
            Some(1)
        );

        let fallback = LinuxOutputPlacement {
            output_name: Some("missing".to_string()),
            output: PersistedMonitor {
                width: 1280,
                height: 720,
                scale_factor: 1.0,
            },
            anchor: LinuxWindowAnchor::TopRight,
            margin_x: 24,
            margin_y: 24,
        };
        assert_eq!(
            linux_output_index_for_placement(&fallback, &outputs),
            Some(0)
        );
    }

    #[test]
    fn linux_output_placement_round_trips_anchor_and_margins() {
        let output = output("HDMI-1", PhysicalPoint::new(-1920, 20), false);
        let origin = PhysicalPoint::new(-1920 + 1920 - 96 - 32, 20 + 1080 - 96 - 18);
        let placement = linux_output_placement_for_origin(&output, origin, 96, 96);
        assert_eq!(placement.anchor, LinuxWindowAnchor::BottomRight);
        assert_eq!(placement.margin_x, 32);
        assert_eq!(placement.margin_y, 18);
        assert_eq!(
            linux_output_origin_for_placement(&placement, &output, 96, 96),
            origin
        );
    }

    #[test]
    fn fixed_drag_anchor_stays_continuous_across_output_center() {
        let output = output("HDMI-1", PhysicalPoint::new(0, 0), false);
        let right_of_center = PhysicalPoint::new(912, 100);
        let left_of_center = PhysicalPoint::new(911, 100);
        let right_placement = linux_output_placement_for_origin_with_anchor(
            &output,
            right_of_center,
            96,
            96,
            LinuxWindowAnchor::TopRight,
        );
        let left_placement = linux_output_placement_for_origin_with_anchor(
            &output,
            left_of_center,
            96,
            96,
            LinuxWindowAnchor::TopRight,
        );

        assert_eq!(right_placement.anchor, LinuxWindowAnchor::TopRight);
        assert_eq!(left_placement.anchor, LinuxWindowAnchor::TopRight);
        assert_eq!(left_placement.margin_x, right_placement.margin_x + 1);
        assert_eq!(
            linux_output_origin_for_placement(&left_placement, &output, 96, 96),
            left_of_center
        );
    }

    #[test]
    fn fixed_drag_anchor_preserves_offscreen_origins() {
        let output = output("HDMI-1", PhysicalPoint::new(0, 0), false);
        let above_left = PhysicalPoint::new(-40, -30);
        let above_left_placement = linux_output_placement_for_origin_with_anchor(
            &output,
            above_left,
            96,
            96,
            LinuxWindowAnchor::TopRight,
        );
        assert_eq!(above_left_placement.margin_x, 1864);
        assert_eq!(above_left_placement.margin_y, -30);
        assert_eq!(
            linux_output_origin_for_placement(&above_left_placement, &output, 96, 96),
            above_left
        );

        let below_right = PhysicalPoint::new(1870, 1040);
        let below_right_placement = linux_output_placement_for_origin_with_anchor(
            &output,
            below_right,
            96,
            96,
            LinuxWindowAnchor::BottomLeft,
        );
        assert_eq!(below_right_placement.margin_x, 1870);
        assert_eq!(below_right_placement.margin_y, -56);
        assert_eq!(
            linux_output_origin_for_placement(&below_right_placement, &output, 96, 96),
            below_right
        );
    }

    #[test]
    fn linux_drag_delta_converts_local_logical_units_to_physical_pixels() {
        assert_eq!(
            linux_drag_delta(LogicalPoint::new(12.25, -4.5), 2.0),
            Some(PhysicalPoint::new(25, -9))
        );
        assert_eq!(
            linux_drag_delta(LogicalPoint::new(f64::NAN, 1.0), 1.0),
            None
        );
    }
}
