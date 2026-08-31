#[cfg(debug_assertions)]
use std::sync::Arc;

const UPDATE_RELEASE_API_URL: &str =
    "https://api.github.com/repos/samm81/downshift/releases/latest";

#[cfg(debug_assertions)]
const SIMULATED_PENDING_UPDATE_VERSION: &str = "99.99.99";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateCheckSource {
    Background,
    Manual,
}

impl UpdateCheckSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UpdateCheckResult {
    pub(crate) latest_version: Option<String>,
    pub(crate) download_url: String,
    #[cfg(debug_assertions)]
    pub(crate) simulated: bool,
}

impl UpdateCheckResult {
    pub(crate) fn should_persist_latest_version(&self) -> bool {
        #[cfg(debug_assertions)]
        {
            !self.simulated
        }
        #[cfg(not(debug_assertions))]
        {
            true
        }
    }
}

#[derive(Clone)]
struct HttpUpdateCheckProvider {
    fallback_download_url: String,
}

impl HttpUpdateCheckProvider {
    fn new(fallback_download_url: String) -> Self {
        Self {
            fallback_download_url,
        }
    }
}

impl HttpUpdateCheckProvider {
    fn check(&self) -> UpdateCheckResult {
        let response = ureq::get(UPDATE_RELEASE_API_URL)
            .set("User-Agent", "downshift")
            .call();
        let Ok(response) = response else {
            return UpdateCheckResult {
                latest_version: None,
                download_url: self.fallback_download_url.clone(),
                #[cfg(debug_assertions)]
                simulated: false,
            };
        };
        let body = response.into_string().unwrap_or_default();
        let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let latest_version = data
            .get("tag_name")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let download_url = data
            .get("html_url")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| self.fallback_download_url.clone());
        UpdateCheckResult {
            latest_version,
            download_url,
            #[cfg(debug_assertions)]
            simulated: false,
        }
    }
}

#[cfg(debug_assertions)]
#[derive(Clone, Default)]
struct DeveloperUpdateControls {
    simulate_pending_update: Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(debug_assertions)]
impl DeveloperUpdateControls {
    fn is_simulate_pending_update_enabled(&self) -> bool {
        self.simulate_pending_update
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    fn set_simulate_pending_update(&self, enabled: bool) {
        self.simulate_pending_update
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(debug_assertions)]
fn apply_developer_simulation(result: &mut UpdateCheckResult, enabled: bool) {
    if enabled {
        result.latest_version = Some(SIMULATED_PENDING_UPDATE_VERSION.to_string());
        result.simulated = true;
    }
}

#[derive(Clone)]
pub(crate) struct UpdateCheckService {
    provider: HttpUpdateCheckProvider,
    #[cfg(debug_assertions)]
    controls: DeveloperUpdateControls,
}

impl UpdateCheckService {
    pub(crate) fn new(fallback_download_url: String) -> Self {
        let provider = HttpUpdateCheckProvider::new(fallback_download_url);
        #[cfg(debug_assertions)]
        {
            let controls = DeveloperUpdateControls::default();
            Self { provider, controls }
        }
        #[cfg(not(debug_assertions))]
        Self { provider }
    }

    pub(crate) fn check(&self) -> UpdateCheckResult {
        #[cfg(debug_assertions)]
        let result = {
            let mut result = self.provider.check();
            apply_developer_simulation(
                &mut result,
                self.controls.is_simulate_pending_update_enabled(),
            );
            result
        };
        #[cfg(not(debug_assertions))]
        let result = self.provider.check();
        result
    }

    #[cfg(debug_assertions)]
    pub(crate) fn simulate_pending_update(&self) -> bool {
        self.controls.is_simulate_pending_update_enabled()
    }

    #[cfg(debug_assertions)]
    pub(crate) fn set_simulate_pending_update(&self, enabled: bool) {
        self.controls.set_simulate_pending_update(enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_result_is_persistable() {
        let result = UpdateCheckResult {
            latest_version: None,
            download_url: "https://example.invalid".to_string(),
            #[cfg(debug_assertions)]
            simulated: false,
        };
        assert!(result.should_persist_latest_version());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn developer_simulation_replaces_only_the_latest_version() {
        let mut result = UpdateCheckResult {
            latest_version: Some("1.2.3".to_string()),
            download_url: "https://example.invalid/release".to_string(),
            simulated: false,
        };

        apply_developer_simulation(&mut result, true);

        assert_eq!(result.latest_version.as_deref(), Some("99.99.99"));
        assert_eq!(result.download_url, "https://example.invalid/release");
        assert!(result.simulated);
        assert!(!result.should_persist_latest_version());
    }
}
