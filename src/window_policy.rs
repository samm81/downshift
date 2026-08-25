use downshift::{clamp_size, PersistedMonitor, Settings, DEFAULT_SIZE};

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
}
