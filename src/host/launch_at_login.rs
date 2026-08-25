#[cfg(target_os = "macos")]
use std::path::Path;

#[cfg(target_os = "macos")]
use downshift::{launch_agent_path_from_home, launch_agent_plist};

#[cfg(target_os = "windows")]
const WINDOWS_RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const WINDOWS_RUN_VALUE: &str = "Downshift";

pub(crate) fn set_launch_at_login(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let path =
            launch_agent_path().ok_or_else(|| "failed to resolve launch agent path".to_string())?;
        let result = if enabled {
            let executable = std::env::current_exe().map_err(|error| error.to_string())?;
            write_launch_agent(&path, &executable)
        } else {
            remove_launch_agent(&path)
        };
        result
    }

    #[cfg(target_os = "windows")]
    {
        set_windows_launch_at_login(enabled)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = enabled;
        Ok(())
    }
}

pub(crate) fn reconcile_launch_at_login(enabled: bool) -> Result<(), String> {
    set_launch_at_login(enabled)
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|home| launch_agent_path_from_home(&home))
}

#[cfg(target_os = "macos")]
pub(crate) fn write_launch_agent(path: &Path, executable: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid launch agent path: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(path, launch_agent_plist(executable)).map_err(|error| error.to_string())
}

#[cfg(target_os = "macos")]
pub(crate) fn remove_launch_agent(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(target_os = "windows")]
fn set_windows_launch_at_login(enabled: bool) -> Result<(), String> {
    let output = if enabled {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let command = format!("\"{}\"", executable.display());
        std::process::Command::new("reg.exe")
            .args([
                "add",
                WINDOWS_RUN_KEY,
                "/v",
                WINDOWS_RUN_VALUE,
                "/t",
                "REG_SZ",
                "/d",
                &command,
                "/f",
            ])
            .output()
            .map_err(|error| error.to_string())?
    } else {
        std::process::Command::new("reg.exe")
            .args(["delete", WINDOWS_RUN_KEY, "/v", WINDOWS_RUN_VALUE, "/f"])
            .output()
            .map_err(|error| error.to_string())?
    };

    if output.status.success()
        || (!enabled
            && String::from_utf8_lossy(&output.stderr)
                .to_ascii_lowercase()
                .contains("unable to find"))
    {
        return Ok(());
    }

    let details = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if details.is_empty() {
        format!("reg.exe exited with status {}", output.status)
    } else {
        details
    })
}
