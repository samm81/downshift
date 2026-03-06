use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, Once};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 1;
const MAX_QUEUE_EVENTS: usize = 1000;
const MAX_QUEUE_BYTES: usize = 2 * 1024 * 1024;
const MAX_EVENTS_PER_SEC: usize = 5;
const MAX_BATCH_SIZE: usize = 25;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const COMPILED_TELEMETRY_ENABLED: Option<&str> = option_env!("DOWNSHIFT_TELEMETRY_ENABLED");
const COMPILED_BETTERSTACK_LOGS_TOKEN: Option<&str> = option_env!("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN");
const COMPILED_BETTERSTACK_LOGS_HOST: Option<&str> = option_env!("DOWNSHIFT_BETTERSTACK_LOGS_HOST");
const COMPILED_BETTERSTACK_ERRORS_DSN: Option<&str> = option_env!("DOWNSHIFT_BETTERSTACK_ERRORS_DSN");
const COMPILED_BUILD_CHANNEL: Option<&str> = option_env!("DOWNSHIFT_BUILD_CHANNEL");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventName {
    InstallFirstRun,
    SessionStart,
    SessionEnd,
    ActivityStateChanged,
    MenuAction,
    PrivacyPreferenceChanged,
    AppError,
    AppCrash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEndReason {
    QuitMenu,
    WindowClose,
    CtrlC,
    StartupFailure,
    EventLoopFailure,
    Panic,
    Unknown,
}

impl SessionEndReason {
    pub fn clean_exit(self) -> bool {
        matches!(self, Self::QuitMenu | Self::WindowClose | Self::CtrlC)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuAction {
    Pause,
    Resume,
    SizeChange,
    Reset,
    Quit,
    AnalyticsMenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityState {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityTrigger {
    Manual,
    DisabledForever,
    DisabledTimed,
    ExpiryTimed,
    AppStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeTarget {
    #[serde(rename = "S")]
    S,
    #[serde(rename = "M")]
    M,
    #[serde(rename = "L")]
    L,
    #[serde(rename = "XL")]
    Xl,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub schema_version: u32,
    pub event_name: EventName,
    pub event_id: String,
    pub occurred_at_utc: String,
    pub local_date: String,
    pub local_tz_offset_min: i32,
    pub anon_user_id: String,
    pub session_id: Option<String>,
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub build_channel: String,
    #[serde(rename = ".env", default = "default_telemetry_env")]
    pub telemetry_env: String,
    pub properties: serde_json::Value,
}

#[derive(Debug)]
pub enum TelemetryError {
    Network(String),
    Serialization(String),
}

impl fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(msg) | Self::Serialization(msg) => f.write_str(msg),
        }
    }
}

impl std::error::Error for TelemetryError {}

pub trait TelemetrySink: Send {
    fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError>;
}

#[derive(Default)]
pub struct NoopSink;

impl TelemetrySink for NoopSink {
    fn send_batch(&mut self, _events: &[Envelope]) -> Result<(), TelemetryError> {
        Ok(())
    }
}

pub struct BetterStackLogsSink {
    url: String,
    token: String,
}

impl BetterStackLogsSink {
    pub fn from_env() -> Option<Self> {
        static MISSING_LOGS_CONFIG_WARNING: Once = Once::new();
        let token = env_or_compiled("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN", COMPILED_BETTERSTACK_LOGS_TOKEN);
        let host = env_or_compiled("DOWNSHIFT_BETTERSTACK_LOGS_HOST", COMPILED_BETTERSTACK_LOGS_HOST);
        let (Some(token), Some(host)) = (token, host) else {
            MISSING_LOGS_CONFIG_WARNING.call_once(|| {
                eprintln!(
                    "warning: telemetry usage sink disabled; set DOWNSHIFT_BETTERSTACK_LOGS_TOKEN and DOWNSHIFT_BETTERSTACK_LOGS_HOST at runtime or build time"
                );
            });
            return None;
        };
        Some(Self {
            url: normalize_logs_url(&host),
            token,
        })
    }
}

impl TelemetrySink for BetterStackLogsSink {
    fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError> {
        let payload = serde_json::to_vec(events)
            .map_err(|error| TelemetryError::Serialization(error.to_string()))?;
        let response = ureq::post(&self.url)
            .set("Authorization", &format!("Bearer {}", self.token))
            .set("Content-Type", "application/json")
            .send_bytes(&payload);
        match response {
            Ok(response) if (200..300).contains(&response.status()) => Ok(()),
            Ok(response) => Err(TelemetryError::Network(format!(
                "logs sink returned status {}",
                response.status()
            ))),
            Err(error) => Err(TelemetryError::Network(error.to_string())),
        }
    }
}

pub struct SentryErrorSink {
    _guard: sentry::ClientInitGuard,
}

impl SentryErrorSink {
    pub fn from_env() -> Option<Self> {
        static MISSING_ERRORS_DSN_WARNING: Once = Once::new();
        let dsn =
            match env_or_compiled("DOWNSHIFT_BETTERSTACK_ERRORS_DSN", COMPILED_BETTERSTACK_ERRORS_DSN)
            {
                Some(dsn) => dsn,
                None => {
                MISSING_ERRORS_DSN_WARNING.call_once(|| {
                    eprintln!(
                        "warning: telemetry crash sink disabled; set DOWNSHIFT_BETTERSTACK_ERRORS_DSN at runtime or build time"
                    );
                });
                return None;
                }
            };
        let release = format!("downshift@{}", env!("CARGO_PKG_VERSION"));
        let guard = sentry::init((
            dsn,
            sentry::ClientOptions {
                release: Some(release.into()),
                ..Default::default()
            },
        ));
        Some(Self { _guard: guard })
    }
}

impl TelemetrySink for SentryErrorSink {
    fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError> {
        for event in events {
            let level = match event.event_name {
                EventName::AppCrash => sentry::Level::Fatal,
                _ => sentry::Level::Error,
            };
            let message = format!("{:?}", event.event_name);
            let payload = serde_json::to_string(&event.properties)
                .map_err(|error| TelemetryError::Serialization(error.to_string()))?;
            sentry::configure_scope(|scope| {
                scope.set_extra("properties", payload.clone().into());
                scope.set_extra("event_id", event.event_id.clone().into());
                scope.set_extra("build_channel", event.build_channel.clone().into());
            });
            sentry::capture_event(sentry::protocol::Event {
                level,
                message: Some(message),
                ..Default::default()
            });
        }
        Ok(())
    }
}

pub trait TelemetryClient {
    fn track(&self, event_name: EventName, properties: serde_json::Value);
    fn track_error(&self, event_name: EventName, properties: serde_json::Value);
    fn start_session(&self, initial_state: ActivityState);
    fn track_activity_state(
        &self,
        state: ActivityState,
        trigger: ActivityTrigger,
        requested_duration_sec: Option<u64>,
    );
    fn end_session(&self, reason: SessionEndReason);
    fn flush(&self, timeout: Duration);
    fn shutdown(&self, timeout: Duration);
    fn set_usage_enabled(&self, enabled: bool);
    fn set_crash_enabled(&self, enabled: bool);
}

#[derive(Debug, Clone)]
pub struct TelemetryState {
    pub anon_user_id: String,
    pub usage_enabled: bool,
    pub crash_enabled: bool,
    pub install_first_run: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct PersistedTelemetryState {
    anon_user_id: String,
    #[serde(default = "bool_true")]
    usage_enabled: bool,
    #[serde(default = "bool_true")]
    crash_enabled: bool,
}

fn bool_true() -> bool {
    true
}

fn default_telemetry_env() -> String {
    "unset".to_string()
}

fn telemetry_env_from_process() -> String {
    env::var("DOWNSHIFT_ENV").unwrap_or_else(|_| default_telemetry_env())
}

#[derive(Clone)]
pub struct RuntimeTelemetryClient {
    sender: mpsc::Sender<WorkerCommand>,
    shared: Arc<SharedContext>,
}

struct SharedContext {
    anon_user_id: String,
    session: Mutex<Option<SessionContext>>,
    usage_enabled: Mutex<bool>,
    crash_enabled: Mutex<bool>,
    build_channel: String,
    telemetry_env: String,
}

#[derive(Debug, Clone)]
struct SessionContext {
    session_id: String,
    started_at: Instant,
    activity_state: ActivityState,
    state_started_at: Instant,
    active_duration_sec: u64,
    disabled_duration_sec: u64,
}

enum WorkerCommand {
    TrackUsage(Envelope),
    TrackCrash(Envelope),
    SetUsageEnabled(bool),
    SetCrashEnabled(bool),
    Flush(mpsc::Sender<()>),
    Shutdown(mpsc::Sender<()>),
}

struct Worker {
    usage_sink: Box<dyn TelemetrySink>,
    crash_sink: Box<dyn TelemetrySink>,
    usage_queue: VecDeque<Envelope>,
    usage_queue_bytes: usize,
    usage_enabled: bool,
    crash_enabled: bool,
    spool_path: PathBuf,
    recent_events: VecDeque<Instant>,
    retry_backoff: Duration,
    next_retry_at: Instant,
}

impl RuntimeTelemetryClient {
    pub fn from_env() -> Self {
        let state = telemetry_state();
        Self::from_state(state)
    }

    pub fn from_state(state: TelemetryState) -> Self {
        let usage_sink: Box<dyn TelemetrySink> =
            if global_telemetry_enabled() && state.usage_enabled {
                BetterStackLogsSink::from_env()
                    .map(|sink| Box::new(sink) as Box<dyn TelemetrySink>)
                    .unwrap_or_else(|| Box::new(NoopSink))
            } else {
                Box::new(NoopSink)
            };
        let crash_sink: Box<dyn TelemetrySink> =
            if global_telemetry_enabled() && state.crash_enabled {
                SentryErrorSink::from_env()
                    .map(|sink| Box::new(sink) as Box<dyn TelemetrySink>)
                    .unwrap_or_else(|| Box::new(NoopSink))
            } else {
                Box::new(NoopSink)
            };
        Self::new_with_sinks(state, usage_sink, crash_sink)
    }

    pub fn new_with_sinks(
        state: TelemetryState,
        usage_sink: Box<dyn TelemetrySink>,
        crash_sink: Box<dyn TelemetrySink>,
    ) -> Self {
        let spool_path = telemetry_queue_path();
        let (sender, receiver) = mpsc::channel();
        let shared = Arc::new(SharedContext {
            anon_user_id: state.anon_user_id,
            session: Mutex::new(None),
            usage_enabled: Mutex::new(state.usage_enabled),
            crash_enabled: Mutex::new(state.crash_enabled),
            build_channel: env_or_compiled("DOWNSHIFT_BUILD_CHANNEL", COMPILED_BUILD_CHANNEL)
                .unwrap_or_else(|| "dev".to_string()),
            telemetry_env: telemetry_env_from_process(),
        });

        let mut worker = Worker {
            usage_sink,
            crash_sink,
            usage_queue: load_spool(&spool_path),
            usage_queue_bytes: 0,
            usage_enabled: state.usage_enabled,
            crash_enabled: state.crash_enabled,
            spool_path,
            recent_events: VecDeque::new(),
            retry_backoff: Duration::from_secs(1),
            next_retry_at: Instant::now(),
        };
        worker.usage_queue_bytes = worker
            .usage_queue
            .iter()
            .map(serialized_event_size)
            .sum::<usize>();
        let _ = worker.persist_spool();

        thread::spawn(move || worker.run(receiver));

        Self { sender, shared }
    }

    pub fn telemetry_state() -> TelemetryState {
        telemetry_state()
    }

    fn build_envelope(&self, event_name: EventName, properties: serde_json::Value) -> Envelope {
        let utc_now = chrono::Utc::now();
        let local_now = chrono::Local::now();
        let session_id = self
            .shared
            .session
            .lock()
            .ok()
            .and_then(|state| state.as_ref().map(|session| session.session_id.clone()));
        Envelope {
            schema_version: SCHEMA_VERSION,
            event_name,
            event_id: Uuid::new_v4().to_string(),
            occurred_at_utc: utc_now.to_rfc3339(),
            local_date: local_now.format("%Y-%m-%d").to_string(),
            local_tz_offset_min: local_now.offset().local_minus_utc() / 60,
            anon_user_id: self.shared.anon_user_id.clone(),
            session_id,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: env::consts::OS.to_string(),
            arch: env::consts::ARCH.to_string(),
            build_channel: self.shared.build_channel.clone(),
            telemetry_env: self.shared.telemetry_env.clone(),
            properties,
        }
    }
}

impl TelemetryClient for RuntimeTelemetryClient {
    fn track(&self, event_name: EventName, properties: serde_json::Value) {
        let enabled = self
            .shared
            .usage_enabled
            .lock()
            .ok()
            .map(|value| *value)
            .unwrap_or(false);
        if !enabled {
            return;
        }
        let envelope = self.build_envelope(event_name, properties);
        let _ = self.sender.send(WorkerCommand::TrackUsage(envelope));
    }

    fn track_error(&self, event_name: EventName, properties: serde_json::Value) {
        let enabled = self
            .shared
            .crash_enabled
            .lock()
            .ok()
            .map(|value| *value)
            .unwrap_or(false);
        if !enabled {
            return;
        }
        let envelope = self.build_envelope(event_name, properties);
        let _ = self.sender.send(WorkerCommand::TrackCrash(envelope));
    }

    fn start_session(&self, initial_state: ActivityState) {
        let now = Instant::now();
        {
            if let Ok(mut session) = self.shared.session.lock() {
                if session.is_some() {
                    return;
                }
                *session = Some(SessionContext {
                    session_id: Uuid::new_v4().to_string(),
                    started_at: now,
                    activity_state: initial_state,
                    state_started_at: now,
                    active_duration_sec: 0,
                    disabled_duration_sec: 0,
                });
            }
        }
        self.track(
            EventName::SessionStart,
            serde_json::json!({
                "launch_reason": "manual",
            }),
        );
        self.track(
            EventName::ActivityStateChanged,
            serde_json::json!({
                "state": serde_json::to_value(initial_state).unwrap_or_else(|_| serde_json::json!("active")),
                "trigger": serde_json::to_value(ActivityTrigger::AppStart)
                    .unwrap_or_else(|_| serde_json::json!("app_start")),
            }),
        );
    }

    fn track_activity_state(
        &self,
        state: ActivityState,
        trigger: ActivityTrigger,
        requested_duration_sec: Option<u64>,
    ) {
        let should_emit = self
            .shared
            .session
            .lock()
            .ok()
            .and_then(|mut session| {
                let session = session.as_mut()?;
                let now = Instant::now();
                let elapsed = now.duration_since(session.state_started_at).as_secs();
                match session.activity_state {
                    ActivityState::Active => {
                        session.active_duration_sec = session.active_duration_sec.saturating_add(elapsed)
                    }
                    ActivityState::Disabled => {
                        session.disabled_duration_sec =
                            session.disabled_duration_sec.saturating_add(elapsed)
                    }
                }
                session.state_started_at = now;
                if session.activity_state == state && trigger != ActivityTrigger::AppStart {
                    return Some(false);
                }
                session.activity_state = state;
                Some(true)
            })
            .unwrap_or(false);

        if !should_emit {
            return;
        }
        let mut payload = serde_json::json!({
            "state": serde_json::to_value(state).unwrap_or_else(|_| serde_json::json!("active")),
            "trigger": serde_json::to_value(trigger).unwrap_or_else(|_| serde_json::json!("manual")),
        });
        if let Some(seconds) = requested_duration_sec {
            payload["requested_duration_sec"] = serde_json::json!(seconds);
        }
        self.track(EventName::ActivityStateChanged, payload);
    }

    fn end_session(&self, reason: SessionEndReason) {
        let (duration, active_duration_sec, disabled_duration_sec) = self
            .shared
            .session
            .lock()
            .ok()
            .and_then(|session| {
                session.as_ref().map(|ctx| {
                    let now = Instant::now();
                    let mut active_duration_sec = ctx.active_duration_sec;
                    let mut disabled_duration_sec = ctx.disabled_duration_sec;
                    let trailing = now.duration_since(ctx.state_started_at).as_secs();
                    match ctx.activity_state {
                        ActivityState::Active => {
                            active_duration_sec = active_duration_sec.saturating_add(trailing)
                        }
                        ActivityState::Disabled => {
                            disabled_duration_sec = disabled_duration_sec.saturating_add(trailing)
                        }
                    }
                    (
                        ctx.started_at.elapsed().as_secs(),
                        active_duration_sec,
                        disabled_duration_sec,
                    )
                })
            })
            .unwrap_or((0, 0, 0));
        self.track(
            EventName::SessionEnd,
            serde_json::json!({
                "reason": serde_json::to_value(reason).unwrap_or_else(|_| serde_json::json!("unknown")),
                "session_duration_sec": duration,
                "active_duration_sec": active_duration_sec,
                "disabled_duration_sec": disabled_duration_sec,
                "clean_exit": reason.clean_exit(),
            }),
        );
    }

    fn flush(&self, timeout: Duration) {
        let (tx, rx) = mpsc::channel();
        let _ = self.sender.send(WorkerCommand::Flush(tx));
        let _ = rx.recv_timeout(timeout);
    }

    fn shutdown(&self, timeout: Duration) {
        let (tx, rx) = mpsc::channel();
        let _ = self.sender.send(WorkerCommand::Shutdown(tx));
        let _ = rx.recv_timeout(timeout);
    }

    fn set_usage_enabled(&self, enabled: bool) {
        if let Ok(mut value) = self.shared.usage_enabled.lock() {
            *value = enabled;
        }
        persist_telemetry_toggles(Some(enabled), None);
        let _ = self.sender.send(WorkerCommand::SetUsageEnabled(enabled));
    }

    fn set_crash_enabled(&self, enabled: bool) {
        if let Ok(mut value) = self.shared.crash_enabled.lock() {
            *value = enabled;
        }
        persist_telemetry_toggles(None, Some(enabled));
        let _ = self.sender.send(WorkerCommand::SetCrashEnabled(enabled));
    }
}

impl Worker {
    fn run(&mut self, receiver: mpsc::Receiver<WorkerCommand>) {
        loop {
            match receiver.recv_timeout(FLUSH_INTERVAL) {
                Ok(command) => {
                    if self.handle_command(command) {
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.flush_usage();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    self.flush_usage();
                    break;
                }
            }
        }
    }

    fn handle_command(&mut self, command: WorkerCommand) -> bool {
        match command {
            WorkerCommand::TrackUsage(envelope) => {
                if self.usage_enabled && self.allow_event() {
                    self.push_usage(envelope);
                    if self.usage_queue.len() >= MAX_BATCH_SIZE {
                        self.flush_usage();
                    }
                }
            }
            WorkerCommand::TrackCrash(envelope) => {
                if self.crash_enabled {
                    let _ = self.crash_sink.send_batch(&[envelope]);
                }
            }
            WorkerCommand::SetUsageEnabled(enabled) => {
                self.usage_enabled = enabled;
            }
            WorkerCommand::SetCrashEnabled(enabled) => {
                self.crash_enabled = enabled;
            }
            WorkerCommand::Flush(done) => {
                self.flush_usage();
                let _ = done.send(());
            }
            WorkerCommand::Shutdown(done) => {
                self.flush_usage();
                let _ = done.send(());
                return true;
            }
        }
        false
    }

    fn allow_event(&mut self) -> bool {
        let now = Instant::now();
        while self
            .recent_events
            .front()
            .is_some_and(|timestamp| now.duration_since(*timestamp) > Duration::from_secs(1))
        {
            let _ = self.recent_events.pop_front();
        }
        if self.recent_events.len() >= MAX_EVENTS_PER_SEC {
            return false;
        }
        self.recent_events.push_back(now);
        true
    }

    fn push_usage(&mut self, envelope: Envelope) {
        let size = serialized_event_size(&envelope);
        self.usage_queue.push_back(envelope);
        self.usage_queue_bytes += size;
        while self.usage_queue.len() > MAX_QUEUE_EVENTS || self.usage_queue_bytes > MAX_QUEUE_BYTES
        {
            if let Some(oldest) = self.usage_queue.pop_front() {
                self.usage_queue_bytes = self
                    .usage_queue_bytes
                    .saturating_sub(serialized_event_size(&oldest));
            } else {
                break;
            }
        }
        let _ = self.persist_spool();
    }

    fn flush_usage(&mut self) {
        if !self.usage_enabled || self.usage_queue.is_empty() || Instant::now() < self.next_retry_at
        {
            return;
        }

        let batch: Vec<_> = self
            .usage_queue
            .iter()
            .take(MAX_BATCH_SIZE)
            .cloned()
            .collect();
        match self.usage_sink.send_batch(&batch) {
            Ok(()) => {
                for item in &batch {
                    self.usage_queue_bytes = self
                        .usage_queue_bytes
                        .saturating_sub(serialized_event_size(item));
                    let _ = self.usage_queue.pop_front();
                }
                self.retry_backoff = Duration::from_secs(1);
                self.next_retry_at = Instant::now();
                let _ = self.persist_spool();
            }
            Err(_) => {
                let jitter_ms = fastrand::u64(0..500);
                self.next_retry_at =
                    Instant::now() + self.retry_backoff + Duration::from_millis(jitter_ms);
                self.retry_backoff = (self.retry_backoff * 2).min(Duration::from_secs(60));
            }
        }
    }

    fn persist_spool(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.spool_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut content = String::new();
        for event in &self.usage_queue {
            if let Ok(line) = serde_json::to_string(event) {
                content.push_str(&line);
                content.push('\n');
            }
        }
        fs::write(&self.spool_path, content)
    }
}

fn normalize_logs_url(host: &str) -> String {
    let trimmed = host.trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        format!("{trimmed}/")
    } else {
        format!("https://{trimmed}/")
    }
}

fn serialized_event_size(event: &Envelope) -> usize {
    serde_json::to_vec(event)
        .map(|bytes| bytes.len())
        .unwrap_or(256)
}

fn global_telemetry_enabled() -> bool {
    env_or_compiled("DOWNSHIFT_TELEMETRY_ENABLED", COMPILED_TELEMETRY_ENABLED)
        .map(|raw| !matches!(raw.to_ascii_lowercase().as_str(), "0" | "false" | "off"))
        .unwrap_or(true)
}

fn env_or_compiled(key: &str, compiled_value: Option<&str>) -> Option<String> {
    env::var(key)
        .ok()
        .or_else(|| compiled_value.map(ToString::to_string))
        .and_then(|value| {
            if value.trim().is_empty() {
                None
            } else {
                Some(value)
            }
        })
}

pub fn telemetry_state() -> TelemetryState {
    let path = telemetry_state_path();
    match fs::read_to_string(&path)
        .ok()
        .and_then(|raw| toml::from_str::<PersistedTelemetryState>(&raw).ok())
    {
        Some(state) if Uuid::parse_str(&state.anon_user_id).is_ok() => TelemetryState {
            anon_user_id: state.anon_user_id,
            usage_enabled: state.usage_enabled,
            crash_enabled: state.crash_enabled,
            install_first_run: false,
        },
        _ => {
            let anon_user_id = Uuid::new_v4().to_string();
            let state = PersistedTelemetryState {
                anon_user_id: anon_user_id.clone(),
                usage_enabled: true,
                crash_enabled: true,
            };
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(content) = toml::to_string_pretty(&state) {
                let _ = fs::write(path, content);
            }
            TelemetryState {
                anon_user_id,
                usage_enabled: true,
                crash_enabled: true,
                install_first_run: true,
            }
        }
    }
}

fn persist_telemetry_toggles(usage_enabled: Option<bool>, crash_enabled: Option<bool>) {
    let path = telemetry_state_path();
    let mut current = telemetry_state();
    if let Some(value) = usage_enabled {
        current.usage_enabled = value;
    }
    if let Some(value) = crash_enabled {
        current.crash_enabled = value;
    }
    let persisted = PersistedTelemetryState {
        anon_user_id: current.anon_user_id,
        usage_enabled: current.usage_enabled,
        crash_enabled: current.crash_enabled,
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(content) = toml::to_string_pretty(&persisted) {
        let _ = fs::write(path, content);
    }
}

fn telemetry_state_path() -> PathBuf {
    let base = telemetry_dir();
    base.join("telemetry.toml")
}

fn telemetry_queue_path() -> PathBuf {
    let base = telemetry_dir();
    base.join("telemetry-queue.ndjson")
}

fn telemetry_dir() -> PathBuf {
    if let Ok(path) = env::var("DOWNSHIFT_TELEMETRY_DIR") {
        return PathBuf::from(path);
    }
    if let Some(mut dir) = dirs::config_dir() {
        dir.push("downshift");
        return dir;
    }
    PathBuf::from(".")
}

fn load_spool(path: &Path) -> VecDeque<Envelope> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return VecDeque::new(),
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str::<Envelope>(&line).ok())
        .collect()
}

pub fn menu_action_size_target(size_slot: usize) -> Option<SizeTarget> {
    match size_slot {
        0 => Some(SizeTarget::S),
        1 => Some(SizeTarget::M),
        2 => Some(SizeTarget::L),
        3 => Some(SizeTarget::Xl),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;

    fn temp_dir(name: &str) -> PathBuf {
        let pid = std::process::id();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("downshift-telemetry-{name}-{pid}-{now}"))
    }

    #[derive(Default)]
    struct CountingSink {
        pub calls: Arc<AtomicUsize>,
        pub events: Arc<AtomicUsize>,
        pub fail_calls: usize,
    }

    impl TelemetrySink for CountingSink {
        fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.events.fetch_add(events.len(), Ordering::SeqCst);
            if self.fail_calls > 0 {
                self.fail_calls -= 1;
                return Err(TelemetryError::Network("transient".to_string()));
            }
            Ok(())
        }
    }

    struct CollectingSink {
        pub events: Arc<Mutex<Vec<Envelope>>>,
    }

    impl TelemetrySink for CollectingSink {
        fn send_batch(&mut self, events: &[Envelope]) -> Result<(), TelemetryError> {
            if let Ok(mut out) = self.events.lock() {
                out.extend_from_slice(events);
            }
            Ok(())
        }
    }

    #[test]
    #[serial]
    fn anon_user_id_persists_across_reads() {
        let root = temp_dir("anon");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
        let first = telemetry_state();
        let second = telemetry_state();
        assert_eq!(first.anon_user_id, second.anon_user_id);
        assert!(Uuid::parse_str(&first.anon_user_id).is_ok());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[serial]
    fn corrupt_anon_user_id_regenerates_and_marks_first_run() {
        let root = temp_dir("corrupt");
        let path = root.join("telemetry.toml");
        std::fs::create_dir_all(&root).expect("create temp telemetry dir");
        std::fs::write(
            &path,
            "anon_user_id = \"not-a-uuid\"\nusage_enabled = true\ncrash_enabled = true\n",
        )
        .expect("write corrupt telemetry state");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
        let state = telemetry_state();
        assert!(state.install_first_run);
        assert!(Uuid::parse_str(&state.anon_user_id).is_ok());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn session_end_reason_clean_exit_mapping() {
        assert!(SessionEndReason::QuitMenu.clean_exit());
        assert!(SessionEndReason::WindowClose.clean_exit());
        assert!(SessionEndReason::CtrlC.clean_exit());
        assert!(!SessionEndReason::StartupFailure.clean_exit());
        assert!(!SessionEndReason::Panic.clean_exit());
    }

    #[test]
    fn activity_trigger_serializes_with_adjective_last() {
        let disabled_timed =
            serde_json::to_string(&ActivityTrigger::DisabledTimed).expect("serialize trigger");
        let expiry_timed =
            serde_json::to_string(&ActivityTrigger::ExpiryTimed).expect("serialize trigger");
        let disabled_forever =
            serde_json::to_string(&ActivityTrigger::DisabledForever).expect("serialize trigger");
        assert_eq!(disabled_timed, "\"disabled_timed\"");
        assert_eq!(expiry_timed, "\"expiry_timed\"");
        assert_eq!(disabled_forever, "\"disabled_forever\"");
    }

    #[test]
    #[serial]
    fn usage_and_crash_toggles_gate_pipelines() {
        let root = temp_dir("toggles");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
        let usage_calls = Arc::new(AtomicUsize::new(0));
        let usage_events = Arc::new(AtomicUsize::new(0));
        let crash_calls = Arc::new(AtomicUsize::new(0));
        let crash_events = Arc::new(AtomicUsize::new(0));

        let state = TelemetryState {
            anon_user_id: Uuid::new_v4().to_string(),
            usage_enabled: false,
            crash_enabled: false,
            install_first_run: false,
        };
        let client = RuntimeTelemetryClient::new_with_sinks(
            state,
            Box::new(CountingSink {
                calls: usage_calls.clone(),
                events: usage_events.clone(),
                fail_calls: 0,
            }),
            Box::new(CountingSink {
                calls: crash_calls.clone(),
                events: crash_events.clone(),
                fail_calls: 0,
            }),
        );

        client.track(
            EventName::MenuAction,
            serde_json::json!({"action": "pause"}),
        );
        client.track_error(EventName::AppError, serde_json::json!({"category": "ipc"}));
        client.flush(Duration::from_millis(200));
        assert_eq!(usage_events.load(Ordering::SeqCst), 0);
        assert_eq!(crash_events.load(Ordering::SeqCst), 0);

        client.set_usage_enabled(true);
        client.set_crash_enabled(true);
        client.track(
            EventName::MenuAction,
            serde_json::json!({"action": "pause"}),
        );
        client.track_error(EventName::AppError, serde_json::json!({"category": "ipc"}));
        client.flush(Duration::from_millis(400));
        assert!(usage_events.load(Ordering::SeqCst) >= 1);
        assert!(crash_events.load(Ordering::SeqCst) >= 1);

        client.shutdown(Duration::from_millis(400));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn menu_action_mapping_emits_expected_values() {
        assert_eq!(menu_action_size_target(0), Some(SizeTarget::S));
        assert_eq!(menu_action_size_target(1), Some(SizeTarget::M));
        assert_eq!(menu_action_size_target(2), Some(SizeTarget::L));
        assert_eq!(menu_action_size_target(3), Some(SizeTarget::Xl));
        assert_eq!(menu_action_size_target(4), None);
    }

    #[test]
    fn queue_overflow_drops_oldest() {
        let mut worker = Worker {
            usage_sink: Box::new(NoopSink),
            crash_sink: Box::new(NoopSink),
            usage_queue: VecDeque::new(),
            usage_queue_bytes: 0,
            usage_enabled: true,
            crash_enabled: true,
            spool_path: temp_dir("queue").join("telemetry-queue.ndjson"),
            recent_events: VecDeque::new(),
            retry_backoff: Duration::from_secs(1),
            next_retry_at: Instant::now(),
        };

        let base = Envelope {
            schema_version: SCHEMA_VERSION,
            event_name: EventName::MenuAction,
            event_id: String::new(),
            occurred_at_utc: "2026-01-01T00:00:00Z".to_string(),
            local_date: "2026-01-01".to_string(),
            local_tz_offset_min: 0,
            anon_user_id: Uuid::new_v4().to_string(),
            session_id: Some(Uuid::new_v4().to_string()),
            app_version: "0.1.0".to_string(),
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            build_channel: "alpha".to_string(),
            telemetry_env: "unset".to_string(),
            properties: serde_json::json!({"action": "pause"}),
        };

        for _ in 0..(MAX_QUEUE_EVENTS + 25) {
            let mut event = base.clone();
            event.event_id = Uuid::new_v4().to_string();
            worker.push_usage(event);
        }
        assert!(worker.usage_queue.len() <= MAX_QUEUE_EVENTS);
    }

    #[test]
    #[serial]
    fn backoff_retries_and_drains_after_recovery() {
        let root = temp_dir("retry");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);

        let usage_calls = Arc::new(AtomicUsize::new(0));
        let usage_events = Arc::new(AtomicUsize::new(0));
        let state = TelemetryState {
            anon_user_id: Uuid::new_v4().to_string(),
            usage_enabled: true,
            crash_enabled: true,
            install_first_run: false,
        };
        let client = RuntimeTelemetryClient::new_with_sinks(
            state,
            Box::new(CountingSink {
                calls: usage_calls.clone(),
                events: usage_events.clone(),
                fail_calls: 1,
            }),
            Box::new(NoopSink),
        );

        client.track(
            EventName::MenuAction,
            serde_json::json!({"action": "pause"}),
        );
        client.flush(Duration::from_millis(250));
        thread::sleep(Duration::from_secs(2));
        client.flush(Duration::from_millis(500));
        assert!(usage_calls.load(Ordering::SeqCst) >= 2);
        assert!(usage_events.load(Ordering::SeqCst) >= 1);

        client.shutdown(Duration::from_millis(500));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[serial]
    fn session_end_reports_active_and_disabled_durations() {
        let root = temp_dir("durations");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
        let captured_events = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let state = TelemetryState {
            anon_user_id: Uuid::new_v4().to_string(),
            usage_enabled: true,
            crash_enabled: true,
            install_first_run: false,
        };
        let client = RuntimeTelemetryClient::new_with_sinks(
            state,
            Box::new(CollectingSink {
                events: captured_events.clone(),
            }),
            Box::new(NoopSink),
        );
        client.start_session(ActivityState::Active);
        thread::sleep(Duration::from_secs(1));
        client.track_activity_state(
            ActivityState::Disabled,
            ActivityTrigger::DisabledForever,
            None,
        );
        thread::sleep(Duration::from_secs(1));
        client.end_session(SessionEndReason::Unknown);
        client.flush(Duration::from_millis(400));
        client.shutdown(Duration::from_millis(400));

        let events = captured_events.lock().expect("captured events lock");
        let activity_app_start = events.iter().find(|event| {
            event.event_name == EventName::ActivityStateChanged
                && event.properties["trigger"] == "app_start"
        });
        assert!(activity_app_start.is_some());
        let activity_disabled = events.iter().find(|event| {
            event.event_name == EventName::ActivityStateChanged
                && event.properties["trigger"] == "disabled_forever"
        });
        assert!(activity_disabled.is_some());
        let session_end = events
            .iter()
            .find(|event| event.event_name == EventName::SessionEnd)
            .expect("session end event should exist");
        let active_duration_sec = session_end.properties["active_duration_sec"]
            .as_u64()
            .expect("active duration should be u64 seconds");
        let disabled_duration_sec = session_end.properties["disabled_duration_sec"]
            .as_u64()
            .expect("disabled duration should be u64 seconds");
        assert!(active_duration_sec >= 1);
        assert!(disabled_duration_sec >= 1);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    #[serial]
    fn telemetry_env_defaults_to_unset_when_var_missing() {
        let root = temp_dir("env-default");
        std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
        std::env::remove_var("DOWNSHIFT_ENV");
        let captured_events = Arc::new(Mutex::new(Vec::<Envelope>::new()));
        let state = TelemetryState {
            anon_user_id: Uuid::new_v4().to_string(),
            usage_enabled: true,
            crash_enabled: true,
            install_first_run: false,
        };
        let client = RuntimeTelemetryClient::new_with_sinks(
            state,
            Box::new(CollectingSink {
                events: captured_events.clone(),
            }),
            Box::new(NoopSink),
        );
        client.track(
            EventName::MenuAction,
            serde_json::json!({"action": "pause"}),
        );
        client.flush(Duration::from_millis(250));
        client.shutdown(Duration::from_millis(250));

        let events = captured_events.lock().expect("captured events lock");
        let menu_event = events
            .iter()
            .find(|event| event.event_name == EventName::MenuAction)
            .expect("menu action should be captured");
        assert_eq!(menu_event.telemetry_env, "unset");
        std::fs::remove_dir_all(root).ok();
    }
}
