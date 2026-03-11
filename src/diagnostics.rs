#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub app_version: String,
    pub os_version: String,
    pub arch: String,
    pub runtime_state: String,
    pub settings_toml: String,
}

pub fn build_summary(snapshot: &DiagnosticsSnapshot) -> String {
    [
        "downshift diagnostics".to_string(),
        format!("app_version = {:?}", snapshot.app_version),
        format!("os_version = {:?}", snapshot.os_version),
        format!("arch = {:?}", snapshot.arch),
        format!("runtime_state = {:?}", snapshot.runtime_state),
        String::new(),
        "[settings]".to_string(),
        snapshot.settings_toml.trim().to_string(),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_summary_includes_runtime_and_settings_dump() {
        let summary = build_summary(&DiagnosticsSnapshot {
            app_version: "0.1.12".to_string(),
            os_version: "macOS 15.4".to_string(),
            arch: "aarch64".to_string(),
            runtime_state: "active".to_string(),
            settings_toml: [
                "size = 96.0",
                "[breathing_pattern]",
                "expanding_seconds = 5.5",
                "paused = false",
            ]
            .join("\n"),
        });

        assert!(summary.contains(r#"app_version = "0.1.12""#));
        assert!(summary.contains(r#"runtime_state = "active""#));
        assert!(summary.contains("[settings]"));
        assert!(summary.contains("expanding_seconds = 5.5"));
    }
}
