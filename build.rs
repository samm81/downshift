fn main() {
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_ENV");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_BUILD_CHANNEL");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_DOWNLOAD_RELEASE_URL");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_GITHUB_ISSUES_URL");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_SUPPORT_EMAIL");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_TELEMETRY_ENABLED");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_BETTERSTACK_LOGS_TOKEN");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_BETTERSTACK_LOGS_HOST");
    println!("cargo:rerun-if-env-changed=DOWNSHIFT_BETTERSTACK_ERRORS_DSN");

    if env_var("DOWNSHIFT_ENV").as_deref() != Some("prod") {
        return;
    }

    require_nonempty("DOWNSHIFT_BUILD_CHANNEL");
    require_nonempty("DOWNSHIFT_DOWNLOAD_RELEASE_URL");
    require_nonempty("DOWNSHIFT_GITHUB_ISSUES_URL");
    require_nonempty("DOWNSHIFT_SUPPORT_EMAIL");

    let telemetry_enabled = require_nonempty("DOWNSHIFT_TELEMETRY_ENABLED");
    if !matches!(
        telemetry_enabled.to_ascii_lowercase().as_str(),
        "0" | "false" | "off"
    ) {
        require_nonempty("DOWNSHIFT_TELEMETRY_HEARTBEAT_INTERVAL_SEC");
        require_nonempty("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN");
        require_nonempty("DOWNSHIFT_BETTERSTACK_LOGS_HOST");
        require_nonempty("DOWNSHIFT_BETTERSTACK_ERRORS_DSN");
    }
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn require_nonempty(name: &str) -> String {
    env_var(name).unwrap_or_else(|| panic!("{name} is required when DOWNSHIFT_ENV=prod"))
}
