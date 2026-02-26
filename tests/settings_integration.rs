use downshift::{
    load_settings, Settings, DEFAULT_HALF_CYCLE_SECONDS, DEFAULT_SIZE, MAX_SIZE, MIN_SIZE,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_file_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("breath-ball-{name}-{nanos}.toml"))
}

#[test]
fn load_settings_returns_defaults_when_path_missing() {
    let path = temp_file_path("missing");
    let settings = load_settings(Some(&path));

    assert_eq!(settings, Settings::default());
}

#[test]
fn load_settings_parses_valid_toml_and_sanitizes_values() {
    let path = temp_file_path("valid");
    let raw = r#"
size = 400.0
half_cycle_seconds = 4.52
paused = true
x = 120
y = 240
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.size, MAX_SIZE);
    assert_eq!(settings.half_cycle_seconds, 4.5);
    assert!(settings.paused);
    assert!(settings.usage_data_sharing);
    assert!(settings.crash_reports_sharing);
    assert_eq!(settings.x, Some(120));
    assert_eq!(settings.y, Some(240));
    assert_eq!(settings.monitor, None);
}

#[test]
fn load_settings_falls_back_to_default_when_toml_is_invalid() {
    let path = temp_file_path("invalid");
    std::fs::write(&path, "this is not toml").expect("should write invalid settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.size, DEFAULT_SIZE);
    assert_eq!(settings.half_cycle_seconds, DEFAULT_HALF_CYCLE_SECONDS);
    assert!(!settings.paused);
    assert_eq!(settings.x, None);
    assert_eq!(settings.y, None);
}

#[test]
fn load_settings_sanitizes_minimum_bounds() {
    let path = temp_file_path("min-bounds");
    let raw = r#"
size = 0.0
half_cycle_seconds = 999.0
paused = false
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.size, MIN_SIZE);
    assert_eq!(settings.half_cycle_seconds, DEFAULT_HALF_CYCLE_SECONDS);
}
