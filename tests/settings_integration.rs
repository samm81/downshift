use downshift::{load_settings, BreathingPattern, Settings, DEFAULT_SIZE, MAX_SIZE, MIN_SIZE};
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
launch_at_login = true
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.size, MAX_SIZE);
    assert_eq!(
        settings.breathing_pattern,
        BreathingPattern {
            expanding_seconds: 4.5,
            expanded_hold_seconds: 0.0,
            compressing_seconds: 4.5,
            compressed_hold_seconds: 0.0,
        }
    );
    assert_eq!(settings.active_breathing_preset_id, "custom");
    assert!(settings.paused);
    assert!(settings.launch_at_login);
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
    assert_eq!(settings.breathing_pattern, BreathingPattern::coherent());
    assert!(!settings.paused);
    assert!(settings.launch_at_login);
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
    assert_eq!(settings.breathing_pattern, BreathingPattern::coherent());
}

#[test]
fn load_settings_supports_new_pattern_and_saved_presets() {
    let path = temp_file_path("pattern");
    let raw = r#"
size = 96.0
paused = false
active_breathing_preset_id = "focus"

[breathing_pattern]
expanding_seconds = 4.0
expanded_hold_seconds = 1.0
compressing_seconds = 6.0
compressed_hold_seconds = 2.0

[[saved_breathing_presets]]
id = "focus"
name = "focus"

[saved_breathing_presets.pattern]
expanding_seconds = 4.0
expanded_hold_seconds = 2.0
compressing_seconds = 6.0
compressed_hold_seconds = 2.0
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(
        settings.breathing_pattern,
        BreathingPattern {
            expanding_seconds: 4.0,
            expanded_hold_seconds: 2.0,
            compressing_seconds: 6.0,
            compressed_hold_seconds: 2.0,
        }
    );
    assert_eq!(settings.active_breathing_preset_id, "focus");
    assert_eq!(settings.saved_breathing_presets.len(), 1);
    assert_eq!(settings.saved_breathing_presets[0].name, "focus");
}

#[test]
fn load_settings_migrates_legacy_half_cycle_to_pattern() {
    let path = temp_file_path("legacy-half-cycle");
    let raw = r#"
size = 96.0
half_cycle_seconds = 6.49
paused = false
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(
        settings.breathing_pattern,
        BreathingPattern {
            expanding_seconds: 6.5,
            expanded_hold_seconds: 0.0,
            compressing_seconds: 6.5,
            compressed_hold_seconds: 0.0,
        }
    );
    assert_eq!(settings.active_breathing_preset_id, "custom");
}
