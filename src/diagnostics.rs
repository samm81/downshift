use chrono::Local;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const LOG_DIR_ENV: &str = "DOWNSHIFT_LOG_DIR";
const LOG_FILE_NAME: &str = "downshift.log";
const LOG_FILE_NAME_PREVIOUS: &str = "downshift.previous.log";
const MAX_LOG_BYTES: u64 = 1_000_000;

static LOGGER: OnceLock<Result<FileLogger, String>> = OnceLock::new();

struct FileLogger {
    path: PathBuf,
    file: Mutex<File>,
}

impl FileLogger {
    fn new() -> Result<Self, String> {
        let dir = log_dir();
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;

        let path = dir.join(LOG_FILE_NAME);
        rotate_if_needed(&path).map_err(|error| error.to_string())?;

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| error.to_string())?;

        Ok(Self {
            path,
            file: Mutex::new(file),
        })
    }

    fn write_line(&self, line: &str) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
            let _ = file.flush();
        }
    }
}

fn rotate_if_needed(path: &std::path::Path) -> std::io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_LOG_BYTES {
        return Ok(());
    }

    let previous = path.with_file_name(LOG_FILE_NAME_PREVIOUS);
    let _ = fs::remove_file(&previous);
    fs::rename(path, previous)
}

fn logger() -> &'static Result<FileLogger, String> {
    LOGGER.get_or_init(FileLogger::new)
}

pub fn init_logging() -> Result<PathBuf, String> {
    match logger() {
        Ok(logger) => Ok(logger.path.clone()),
        Err(error) => Err(error.clone()),
    }
}

pub fn log_path() -> Option<PathBuf> {
    match logger() {
        Ok(logger) => Some(logger.path.clone()),
        Err(_) => None,
    }
}

pub fn log_line(level: &str, message: &str) {
    let rendered = format!(
        "{} [{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z"),
        level,
        message
    );
    eprint!("{rendered}");
    if let Ok(logger) = logger() {
        logger.write_line(&rendered);
    }
}

fn log_dir() -> PathBuf {
    if let Ok(path) = std::env::var(LOG_DIR_ENV) {
        return PathBuf::from(path);
    }

    if let Some(mut dir) = dirs::config_dir() {
        dir.push("downshift");
        dir.push("logs");
        return dir;
    }

    PathBuf::from(".")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub app_version: String,
    pub build_channel: String,
    pub env: String,
    pub os_version: String,
    pub arch: String,
    pub runtime_state: String,
    pub startup_provenance: String,
    pub settings_load_status: String,
    pub telemetry_global_enabled: bool,
    pub usage_sharing_enabled: bool,
    pub crash_reports_enabled: bool,
    pub telemetry_install_first_run: bool,
    pub executable_path: Option<String>,
    pub settings_path: Option<String>,
    pub log_path: Option<String>,
    pub window_position: Option<String>,
    pub window_size_px: String,
    pub window_scale_factor: Option<String>,
    pub monitor: Option<String>,
    pub linux_session_backend: Option<String>,
    pub linux_window_backend: Option<String>,
    pub linux_requested_mode: Option<String>,
    pub linux_overlay_supported: Option<bool>,
    pub linux_fallback_reason: Option<String>,
    pub settings_toml: String,
}

pub fn build_summary(snapshot: &DiagnosticsSnapshot) -> String {
    let mut lines = vec![
        "downshift diagnostics".to_string(),
        format!("app_version = {:?}", snapshot.app_version),
        format!("build_channel = {:?}", snapshot.build_channel),
        format!("env = {:?}", snapshot.env),
        format!("os_version = {:?}", snapshot.os_version),
        format!("arch = {:?}", snapshot.arch),
        format!("runtime_state = {:?}", snapshot.runtime_state),
        format!("startup_provenance = {:?}", snapshot.startup_provenance),
        format!("settings_load_status = {:?}", snapshot.settings_load_status),
        format!(
            "telemetry_global_enabled = {:?}",
            snapshot.telemetry_global_enabled
        ),
        format!(
            "usage_sharing_enabled = {:?}",
            snapshot.usage_sharing_enabled
        ),
        format!(
            "crash_reports_enabled = {:?}",
            snapshot.crash_reports_enabled
        ),
        format!(
            "telemetry_install_first_run = {:?}",
            snapshot.telemetry_install_first_run
        ),
    ];
    if let Some(path) = snapshot.executable_path.as_ref() {
        lines.push(format!("executable_path = {:?}", path));
    }
    if let Some(path) = snapshot.settings_path.as_ref() {
        lines.push(format!("settings_path = {:?}", path));
    }
    if let Some(path) = snapshot.log_path.as_ref() {
        lines.push(format!("log_path = {:?}", path));
    }
    lines.extend([String::new(), "[window]".to_string()]);
    if let Some(position) = snapshot.window_position.as_ref() {
        lines.push(format!("position = {:?}", position));
    }
    lines.push(format!("size_px = {:?}", snapshot.window_size_px));
    if let Some(scale_factor) = snapshot.window_scale_factor.as_ref() {
        lines.push(format!("scale_factor = {:?}", scale_factor));
    }
    if let Some(monitor) = snapshot.monitor.as_ref() {
        lines.push(format!("monitor = {:?}", monitor));
    }
    if let Some(session_backend) = snapshot.linux_session_backend.as_ref() {
        lines.extend([
            String::new(),
            "[linux]".to_string(),
            format!("session_backend = {:?}", session_backend),
        ]);
        if let Some(window_backend) = snapshot.linux_window_backend.as_ref() {
            lines.push(format!("window_backend = {:?}", window_backend));
        }
        if let Some(requested_mode) = snapshot.linux_requested_mode.as_ref() {
            lines.push(format!("requested_mode = {:?}", requested_mode));
        }
        if let Some(overlay_supported) = snapshot.linux_overlay_supported {
            lines.push(format!("overlay_supported = {overlay_supported:?}"));
        }
        if let Some(fallback_reason) = snapshot.linux_fallback_reason.as_ref() {
            lines.push(format!("fallback_reason = {:?}", fallback_reason));
        }
    }
    lines.extend([
        String::new(),
        "[settings]".to_string(),
        snapshot.settings_toml.trim().to_string(),
    ]);
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn test_log_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("downshift-diagnostics-{name}-{nanos}"))
    }

    #[test]
    fn build_summary_includes_runtime_and_settings_dump() {
        let summary = build_summary(&DiagnosticsSnapshot {
            app_version: "0.1.12".to_string(),
            build_channel: "alpha".to_string(),
            env: "dev".to_string(),
            os_version: "macOS 15.4".to_string(),
            arch: "aarch64".to_string(),
            runtime_state: "active".to_string(),
            startup_provenance: "restored_settings".to_string(),
            settings_load_status: "ok".to_string(),
            telemetry_global_enabled: true,
            usage_sharing_enabled: true,
            crash_reports_enabled: false,
            telemetry_install_first_run: false,
            executable_path: Some(
                "/Applications/Downshift.app/Contents/MacOS/downshift".to_string(),
            ),
            settings_path: Some(
                "/Users/m1/Library/Application Support/downshift/settings.toml".to_string(),
            ),
            log_path: Some("/tmp/downshift.log".to_string()),
            window_position: Some("x=240, y=32".to_string()),
            window_size_px: "96x96".to_string(),
            window_scale_factor: Some("2.00".to_string()),
            monitor: Some("1728x1117 @ 2.00x".to_string()),
            linux_session_backend: None,
            linux_window_backend: None,
            linux_requested_mode: None,
            linux_overlay_supported: None,
            linux_fallback_reason: None,
            settings_toml: [
                "size = 96.0",
                "[breathing_pattern]",
                "expanding_seconds = 5.5",
                "paused = false",
            ]
            .join("\n"),
        });

        assert!(summary.contains(r#"app_version = "0.1.12""#));
        assert!(summary.contains(r#"build_channel = "alpha""#));
        assert!(summary.contains(r#"env = "dev""#));
        assert!(summary.contains(r#"startup_provenance = "restored_settings""#));
        assert!(summary.contains(r#"settings_load_status = "ok""#));
        assert!(summary.contains(r#"usage_sharing_enabled = true"#));
        assert!(summary.contains(r#"crash_reports_enabled = false"#));
        assert!(summary.contains(
            r#"settings_path = "/Users/m1/Library/Application Support/downshift/settings.toml""#
        ));
        assert!(summary.contains(r#"runtime_state = "active""#));
        assert!(summary.contains(r#"log_path = "/tmp/downshift.log""#));
        assert!(summary.contains("[window]"));
        assert!(summary.contains(r#"position = "x=240, y=32""#));
        assert!(summary.contains(r#"monitor = "1728x1117 @ 2.00x""#));
        assert!(summary.contains("[settings]"));
        assert!(summary.contains("expanding_seconds = 5.5"));
    }

    #[test]
    #[serial]
    fn init_logging_creates_log_file_and_writes_entries() {
        let root = test_log_dir("init");
        std::env::set_var(LOG_DIR_ENV, &root);

        let path = init_logging().expect("log path should initialize");
        log_line("INFO", "hello from diagnostics test");

        let content = std::fs::read_to_string(&path).expect("read log file");
        assert!(content.contains("[INFO] hello from diagnostics test"));

        std::fs::remove_dir_all(root).ok();
    }
}
