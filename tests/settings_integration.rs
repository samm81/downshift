mod support;

use downshift::{
    load_settings, load_settings_result, BreathingPattern, Settings, DEFAULT_SIZE, MAX_SIZE,
    MIN_SIZE,
};

#[test]
fn load_settings_returns_defaults_when_path_missing() {
    let path = support::temp_file_path("breath-ball-missing", "toml");
    let settings = load_settings(Some(&path));

    assert_eq!(settings, Settings::default());
}

#[test]
fn load_settings_parses_valid_toml_and_sanitizes_values() {
    let path = support::temp_file_path("breath-ball-valid", "toml");
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
    assert_eq!(settings.physical_x, None);
    assert_eq!(settings.physical_y, None);
    assert_eq!(settings.x, Some(120));
    assert_eq!(settings.y, Some(240));
    assert_eq!(settings.monitor, None);
}

#[test]
fn load_settings_falls_back_to_default_when_toml_is_invalid() {
    let path = support::temp_file_path("breath-ball-invalid", "toml");
    std::fs::write(&path, "this is not toml").expect("should write invalid settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.size, DEFAULT_SIZE);
    assert_eq!(settings.breathing_pattern, BreathingPattern::coherent());
    assert!(!settings.paused);
    assert!(settings.launch_at_login);
    assert_eq!(settings.x, None);
    assert_eq!(settings.y, None);
    assert_eq!(settings.physical_x, None);
    assert_eq!(settings.physical_y, None);
}

#[test]
fn load_settings_result_reports_invalid_toml_without_touching_file() {
    let path = support::temp_file_path("breath-ball-invalid-result", "toml");
    let raw = "this is not toml";
    std::fs::write(&path, raw).expect("should write invalid settings file");

    let result = load_settings_result(Some(&path));
    let persisted = std::fs::read_to_string(&path).expect("should keep invalid settings content");
    std::fs::remove_file(&path).ok();

    assert!(result.load_error.is_some());
    assert_eq!(result.settings, Settings::default());
    assert_eq!(persisted, raw);
}

#[test]
fn load_settings_sanitizes_minimum_bounds() {
    let path = support::temp_file_path("breath-ball-min-bounds", "toml");
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
fn load_settings_sanitizes_non_finite_size() {
    let path = support::temp_file_path("breath-ball-nan-size", "toml");
    std::fs::write(&path, "size = nan\npaused = false\n")
        .expect("should write settings with nan size");

    let result = load_settings_result(Some(&path));
    std::fs::remove_file(&path).ok();

    assert!(result.load_error.is_none());
    assert_eq!(result.settings.size, DEFAULT_SIZE);
}

#[test]
fn load_settings_supports_new_pattern_and_saved_presets() {
    let path = support::temp_file_path("breath-ball-pattern", "toml");
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
    let path = support::temp_file_path("breath-ball-legacy-half-cycle", "toml");
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

#[test]
fn load_settings_migrates_legacy_update_dismissal_to_ignored_version() {
    let path = support::temp_file_path("breath-ball-legacy-update-dismissal", "toml");
    let raw = r#"
size = 96.0
paused = false
dismissed_update_version = "0.9.0"
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.ignored_update_version.as_deref(), Some("0.9.0"));
    assert!(settings.update_badge_snoozed_version.is_none());
    assert!(settings.update_badge_snoozed_at_epoch_seconds.is_none());
}

#[test]
fn load_settings_parses_physical_window_position_fields() {
    let path = support::temp_file_path("breath-ball-physical-position", "toml");
    let raw = r#"
size = 96.0
paused = false
physical_x = 1440
physical_y = 900
"#;
    std::fs::write(&path, raw).expect("should write temp settings file");

    let settings = load_settings(Some(&path));
    std::fs::remove_file(&path).ok();

    assert_eq!(settings.physical_x, Some(1440));
    assert_eq!(settings.physical_y, Some(900));
    assert_eq!(settings.x, None);
    assert_eq!(settings.y, None);
}
