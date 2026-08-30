use downshift::{clamp_size, PersistedMonitor, Settings, DEFAULT_SIZE};
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
}
