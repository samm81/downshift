use downshift::telemetry::{
    ActivityState, BetterStackLogsSink, EventName, RuntimeTelemetryClient, TelemetryClient, TelemetrySink,
    TelemetryState,
};
use serial_test::serial;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Response, Server, StatusCode};
use uuid::Uuid;

fn temp_dir(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("downshift-telemetry-int-{name}-{nanos}"))
}

fn test_state() -> TelemetryState {
    TelemetryState {
        anon_user_id: Uuid::new_v4().to_string(),
        usage_enabled: true,
        crash_enabled: true,
        install_first_run: false,
    }
}

#[test]
#[serial]
fn logs_sink_sends_auth_and_expected_payload_shape() {
    let server = Server::http("127.0.0.1:0").expect("start tiny_http server");
    let addr = format!("http://{}", server.server_addr());
    std::env::set_var("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN", "token-123");
    std::env::set_var("DOWNSHIFT_BETTERSTACK_LOGS_HOST", &addr);

    let received = Arc::new(AtomicUsize::new(0));
    let received_clone = received.clone();
    let handle = std::thread::spawn(move || {
        let mut request = server.recv().expect("receive request");
        let auth = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"))
            .map(|header| header.value.as_str().to_string())
            .unwrap_or_default();
        assert_eq!(auth, "Bearer token-123");

        let mut body = String::new();
        request
            .as_reader()
            .read_to_string(&mut body)
            .expect("read request body");
        let events: serde_json::Value =
            serde_json::from_str(&body).expect("logs sink should send json array");
        let array = events.as_array().expect("payload is an array");
        assert_eq!(array.len(), 1);
        assert_eq!(array[0]["event_name"], "menu_action");
        assert_eq!(array[0]["schema_version"], 1);
        received_clone.store(1, Ordering::SeqCst);

        let _ = request.respond(Response::empty(200));
    });

    let mut sink = BetterStackLogsSink::from_env().expect("sink should initialize from env");
    let client = RuntimeTelemetryClient::new_with_sinks(
        test_state(),
        Box::new(downshift::telemetry::NoopSink),
        Box::new(downshift::telemetry::NoopSink),
    );
    client.start_session(ActivityState::Active);
    let sample = serde_json::json!({
        "category": "menu",
        "severity": "warn",
        "recoverable": true,
    });
    let event = {
        client.track(
            EventName::MenuAction,
            serde_json::json!({"action": "pause"}),
        );
        client.flush(Duration::from_millis(200));
        downshift::telemetry::Envelope {
            schema_version: 1,
            event_name: EventName::MenuAction,
            event_id: Uuid::new_v4().to_string(),
            occurred_at_utc: "2026-01-01T00:00:00Z".to_string(),
            local_date: "2026-01-01".to_string(),
            local_tz_offset_min: 0,
            anon_user_id: Uuid::new_v4().to_string(),
            session_id: Some(Uuid::new_v4().to_string()),
            app_version: "0.1.0".to_string(),
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            build_channel: "alpha".to_string(),
            properties: sample,
        }
    };
    sink.send_batch(&[event]).expect("sink send should succeed");
    handle.join().expect("server thread should join");
    assert_eq!(received.load(Ordering::SeqCst), 1);
    client.shutdown(Duration::from_millis(200));
}

#[test]
#[serial]
fn retries_after_transient_failure_and_drains_when_recovered() {
    let server = Server::http("127.0.0.1:0").expect("start tiny_http server");
    let addr = format!("http://{}", server.server_addr());
    std::env::set_var("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN", "token-456");
    std::env::set_var("DOWNSHIFT_BETTERSTACK_LOGS_HOST", &addr);

    let hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = hits.clone();
    let handle = std::thread::spawn(move || {
        for index in 0..2 {
            let request = server.recv().expect("receive request");
            hits_clone.fetch_add(1, Ordering::SeqCst);
            let status = if index == 0 { 500 } else { 200 };
            let _ = request.respond(Response::empty(StatusCode(status)));
        }
    });

    let sink = BetterStackLogsSink::from_env().expect("sink from env");
    let client = RuntimeTelemetryClient::new_with_sinks(
        test_state(),
        Box::new(sink),
        Box::new(downshift::telemetry::NoopSink),
    );
    client.start_session(ActivityState::Active);
    client.track(
        EventName::MenuAction,
        serde_json::json!({"action": "pause"}),
    );

    client.flush(Duration::from_millis(250));
    std::thread::sleep(Duration::from_secs(2));
    client.flush(Duration::from_millis(500));

    assert_eq!(hits.load(Ordering::SeqCst), 2);
    client.shutdown(Duration::from_millis(400));
    handle.join().expect("server thread should join");
}

#[test]
#[serial]
fn missing_config_gracefully_degrades_to_noop() {
    let root = temp_dir("noop");
    std::env::set_var("DOWNSHIFT_TELEMETRY_DIR", &root);
    std::env::remove_var("DOWNSHIFT_BETTERSTACK_LOGS_TOKEN");
    std::env::remove_var("DOWNSHIFT_BETTERSTACK_LOGS_HOST");
    std::env::remove_var("DOWNSHIFT_BETTERSTACK_ERRORS_DSN");

    let client = RuntimeTelemetryClient::from_env();
    client.start_session(ActivityState::Active);
    client.track(
        EventName::MenuAction,
        serde_json::json!({"action": "pause"}),
    );
    client.flush(Duration::from_millis(200));
    client.shutdown(Duration::from_millis(200));

    std::fs::remove_dir_all(root).ok();
}
