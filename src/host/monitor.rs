use winit::monitor::MonitorHandle;

use crate::window_policy::{MonitorSnapshot, PhysicalPoint};
use downshift::PersistedMonitor;

pub(crate) fn snapshot_monitor(monitor: &MonitorHandle) -> MonitorSnapshot {
    let size = monitor.size();
    MonitorSnapshot {
        position: PhysicalPoint::new(monitor.position().x, monitor.position().y),
        width: size.width,
        height: size.height,
        scale_factor: monitor.scale_factor(),
    }
}

pub(crate) fn persisted_monitor(monitor: &MonitorHandle) -> PersistedMonitor {
    snapshot_monitor(monitor).persisted()
}
