use std::hash::{Hash, Hasher};

use winit::event_loop::EventLoopProxy;

use crate::app_core::{AppEvent, InstanceCommand};

pub(crate) enum InstanceStart {
    Primary(InstanceGuard),
    AlreadyRunning,
}

pub(crate) struct InstanceGuard {
    #[cfg(target_os = "windows")]
    mutex: windows_sys::Win32::Foundation::HANDLE,
}

#[cfg(target_os = "windows")]
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(self.mutex);
        }
    }
}

pub(crate) fn start(proxy: EventLoopProxy<AppEvent>) -> Result<InstanceStart, String> {
    #[cfg(unix)]
    {
        if let Some(path) = instance_socket_path() {
            if connect_to_existing_instance(&path, InstanceCommand::Activate) {
                return Ok(InstanceStart::AlreadyRunning);
            }
            if let Err(error) = spawn_instance_server(path, proxy.clone()) {
                if error.kind() == std::io::ErrorKind::AddrInUse {
                    return Ok(InstanceStart::AlreadyRunning);
                }
                return Err(format!("failed to start instance server: {error}"));
            }
        }
        return Ok(InstanceStart::Primary(InstanceGuard {}));
    }

    #[cfg(target_os = "windows")]
    {
        start_windows_instance(proxy)
            .map_err(|error| format!("failed to start Windows instance guard: {error}"))
    }

    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = proxy;
        Ok(InstanceStart::Primary(InstanceGuard {}))
    }
}

#[cfg(unix)]
pub(crate) fn instance_socket_path_for_executable(
    executable: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    executable.hash(&mut hasher);
    let executable_hash = hasher.finish();
    let mut path = dirs::config_dir()?;
    path.push("downshift");
    path.push(format!("instance-{executable_hash:016x}.sock"));
    Some(path)
}

#[cfg(unix)]
fn instance_socket_path() -> Option<std::path::PathBuf> {
    let executable = std::env::current_exe().ok()?;
    instance_socket_path_for_executable(&executable)
}

#[cfg(unix)]
fn connect_to_existing_instance(path: &std::path::Path, command: InstanceCommand) -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    stream.write_all(command.as_bytes()).is_ok()
}

#[cfg(unix)]
fn spawn_instance_server(
    path: std::path::PathBuf,
    proxy: EventLoopProxy<AppEvent>,
) -> std::io::Result<()> {
    use std::io::Read;
    use std::os::unix::net::UnixListener;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        match connect_to_existing_instance(&path, InstanceCommand::Activate) {
            true => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::AddrInUse,
                    "instance already running",
                ))
            }
            false => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let listener = UnixListener::bind(&path)?;
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buffer = String::new();
            if stream.read_to_string(&mut buffer).is_err() {
                continue;
            }
            if matches!(
                InstanceCommand::parse(&buffer),
                Some(InstanceCommand::Activate)
            ) {
                let _ = proxy.send_event(AppEvent::InstanceActivate);
            }
        }
    });
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn windows_instance_pipe_name() -> Option<String> {
    let executable = std::env::current_exe().ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    executable.hash(&mut hasher);
    Some(format!(r"\\.\pipe\downshift-{:#016x}", hasher.finish()))
}

#[cfg(target_os = "windows")]
fn windows_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn connect_to_existing_windows_instance(pipe_name: &str, command: InstanceCommand) -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_PIPE_BUSY, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, WriteFile, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        OPEN_EXISTING,
    };
    use windows_sys::Win32::System::Pipes::WaitNamedPipeW;

    let pipe_name = windows_wide(pipe_name);
    for _ in 0..20 {
        let handle = unsafe {
            CreateFileW(
                pipe_name.as_ptr(),
                FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
            let bytes = command.as_bytes();
            let mut written = 0u32;
            let result = unsafe {
                WriteFile(
                    handle,
                    bytes.as_ptr().cast(),
                    bytes.len() as u32,
                    &mut written,
                    std::ptr::null_mut(),
                )
            };
            unsafe {
                let _ = windows_sys::Win32::Foundation::CloseHandle(handle);
            }
            return result != 0 && written == bytes.len() as u32;
        }

        if unsafe { GetLastError() } == ERROR_PIPE_BUSY {
            let _ = unsafe { WaitNamedPipeW(pipe_name.as_ptr(), 100) };
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

#[cfg(target_os = "windows")]
fn spawn_windows_instance_server(
    pipe_name: String,
    proxy: EventLoopProxy<AppEvent>,
) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{
        GetLastError, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, PIPE_ACCESS_INBOUND};
    use windows_sys::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW};
    use windows_sys::Win32::System::Pipes::{
        DisconnectNamedPipe, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES,
        PIPE_WAIT,
    };

    let pipe_name_wide = windows_wide(&pipe_name);
    std::thread::Builder::new()
        .name("downshift-instance-server".to_string())
        .spawn(move || loop {
            let pipe = unsafe {
                CreateNamedPipeW(
                    pipe_name_wide.as_ptr(),
                    PIPE_ACCESS_INBOUND,
                    PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                    PIPE_UNLIMITED_INSTANCES,
                    128,
                    128,
                    0,
                    std::ptr::null(),
                )
            };
            if pipe.is_null() || pipe == INVALID_HANDLE_VALUE {
                crate::diagnostics::log_line(
                    "ERROR",
                    "warning: failed to create Windows instance pipe",
                );
                return;
            }
            let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
                || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
            if connected {
                let mut buffer = [0u8; 128];
                let mut read = 0u32;
                let ok = unsafe {
                    ReadFile(
                        pipe,
                        buffer.as_mut_ptr().cast(),
                        buffer.len() as u32,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                } != 0;
                if ok {
                    let command = String::from_utf8_lossy(&buffer[..read as usize]);
                    if matches!(
                        InstanceCommand::parse(&command),
                        Some(InstanceCommand::Activate)
                    ) && proxy.send_event(AppEvent::InstanceActivate).is_err()
                    {
                        unsafe {
                            let _ = windows_sys::Win32::Foundation::CloseHandle(pipe);
                        }
                        return;
                    }
                }
            }
            unsafe {
                let _ = DisconnectNamedPipe(pipe);
                let _ = windows_sys::Win32::Foundation::CloseHandle(pipe);
            }
        })
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
fn start_windows_instance(proxy: EventLoopProxy<AppEvent>) -> Result<InstanceStart, String> {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    let pipe_name = windows_instance_pipe_name().ok_or_else(|| {
        "failed to resolve executable path for Windows single-instance guard".to_string()
    })?;
    let mutex_name = format!("Local\\downshift-{:016x}", {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pipe_name.hash(&mut hasher);
        hasher.finish()
    });
    let mutex_name_wide = windows_wide(&mutex_name);
    let mutex = unsafe { CreateMutexW(std::ptr::null(), 0, mutex_name_wide.as_ptr()) };
    if mutex.is_null() {
        return Err("CreateMutexW returned a null handle".to_string());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = windows_sys::Win32::Foundation::CloseHandle(mutex);
        }
        let _ = connect_to_existing_windows_instance(&pipe_name, InstanceCommand::Activate);
        return Ok(InstanceStart::AlreadyRunning);
    }

    spawn_windows_instance_server(pipe_name, proxy)?;
    Ok(InstanceStart::Primary(InstanceGuard { mutex }))
}
