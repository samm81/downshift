# telemetry

this is a code-derived inventory of telemetry emitted by this repo as of 2026-03-24.

scope:

- the desktop app in `src/main.rs` and `src/telemetry.rs`

## desktop app telemetry

all app telemetry is emitted by the rust runtime. the embedded webview javascript sends ipc commands back to the app, but it does not send telemetry directly.

### streams and destinations

| stream | enabled when | transport | destination |
| --- | --- | --- | --- |
| usage | global telemetry is on and `usage_data_sharing` is `true` | `POST` with bearer auth; payload is a json array | better stack logs host from `DOWNSHIFT_BETTERSTACK_LOGS_HOST` |
| crash/error | global telemetry is on and `crash_reports_sharing` is `true` | sentry sdk | dsn from `DOWNSHIFT_BETTERSTACK_ERRORS_DSN` |

defaults and persisted state:

- both user-facing toggles default to `true`
- `telemetry.toml` stores `anon_user_id`, `usage_enabled`, and `crash_enabled`
- `anon_user_id` is a persisted uuid and is reused across sessions
- if sink config is missing, the app degrades to noop sinks and only logs a warning

common envelope fields on app events:

- `schema_version`
- `event_name`
- `event_id`
- `occurred_at_utc`
- `local_date`
- `local_tz_offset_min`
- `anon_user_id`
- `session_id` (missing before a session is started)
- `app_version`
- `os`
- `arch`
- `build_channel`
- `env`
  - from `DOWNSHIFT_ENV`, preferring the runtime process env and otherwise falling back to the value compiled into the binary
  - defaults to `unset` only when neither runtime nor build-time `DOWNSHIFT_ENV` was set
- `properties`

delivery behavior:

- usage events are rate-limited to `5` events per second
- usage events are sent one at a time; there is no queue persistence across restarts
- transient send failures are not retried
- suspended time is excluded from session duration totals
- `session_heartbeat` is emitted once at startup, then on a fixed interval
- the heartbeat interval defaults to `60` seconds and is clamped to `5..=3600` seconds

### usage events

- `install_first_run`
  - emitted once when `telemetry.toml` is first created
  - properties: `usage_sharing_enabled_default`, `crash_sharing_enabled_default`
- `session_start`
  - emitted after the main window and main webview are created successfully
  - properties: `launch_reason`
  - current value: `launch_reason = "manual"`
- `session_heartbeat`
  - emitted immediately after `session_start`, then on the configured heartbeat interval
  - properties:
    - `state`
    - `config.paused`
    - `config.snoozed`
    - `config.active_breathing_preset_id`
    - `config.breathing_pattern.expanding_seconds`
    - `config.breathing_pattern.expanded_hold_seconds`
    - `config.breathing_pattern.compressing_seconds`
    - `config.breathing_pattern.compressed_hold_seconds`
    - `config.breathing_pattern.total_seconds`
    - `config.width_px`
    - `config.height_px`
    - `config.usage_enabled`
    - `config.crash_enabled`
- `activity_state_changed`
  - emitted on app start and when the app transitions between `active`, `paused`, and `snoozed`
  - properties:
    - `state`
    - `trigger`
    - optional `requested_duration_sec` for timed snoozes
  - observed triggers: `app_start`, `manual`, `snooze_timed`, `snooze_expired`, `relaunch`
- `session_end`
  - emitted when the app finishes a session
  - properties:
    - `reason`
    - `session_duration_sec`
    - `active_duration_sec`
    - `paused_duration_sec`
    - `snoozed_duration_sec`
    - `clean_exit`
  - observed reasons: `quit_menu`, `window_close`, `ctrl_c`, `startup_failure`, `event_loop_failure`, `panic`, `unknown`
- `menu_action`
  - emitted for interactive app controls
  - `action` values observed in code:
    - `pause`
    - `resume`
    - `snooze`
    - `size_change`
    - `reset`
    - `quit`
    - `context_menu`
    - `analytics_menu`
    - `follow_cursor`
    - `launch_at_login`
  - extra properties:
    - `size_target` for size changes; values are `S`, `M`, `L`, `XL`
    - `enabled` for `launch_at_login`
    - `enabled` for `follow_cursor`
- `breathing_pattern_changed`
  - emitted when the breathing-pattern editor or preset selection changes pattern state
  - `action` values observed in code:
    - `applied`
    - `saved`
    - `deleted`
    - `add_new_opened`
    - `add_new_canceled`
  - `applied` / `saved` / `deleted` payloads include:
    - `preset_id`
    - `preset_name`
    - `is_custom`
    - `is_saved_preset`
    - `pattern.expanding_seconds`
    - `pattern.expanded_hold_seconds`
    - `pattern.compressing_seconds`
    - `pattern.compressed_hold_seconds`
    - `pattern.total_seconds`
  - `add_new_opened` / `add_new_canceled` include only `action` plus the current `pattern`
- `update_flow`
  - emitted for update-check and update-download flows
  - `action` values observed in code:
    - `manual_check_started`
    - `check_completed`
    - `badge_dismissed`
    - `ignore_current_update_changed`
    - `download_opened`
  - extra properties by action:
    - `manual_check_started`: no extra fields
    - `check_completed`: `source`, `latest_version`, `has_update_available`
    - `badge_dismissed`: `latest_version`
    - `ignore_current_update_changed`: `latest_version`, `ignored`
    - `download_opened`: `source`, `has_update_available`, `latest_version`
  - observed `source` values: `manual`, `background`, `menu`, `dialog`
- `privacy_preference_changed`
  - emitted when the user toggles usage sharing or crash-report sharing
  - properties:
    - `setting`
    - `new_value`
  - observed settings: `usage_data`, `crash_reports`

### crash/error events

these events use the crash/error stream, not the usage stream.

- `app_error`
  - properties always include `category`, `severity`, and `recoverable`
  - observed categories:
    - `telemetry_info_window_create`
    - `window_create`
    - `event_proxy`
    - `webview_create`
    - `ipc_parse`
    - `event_loop`
- `app_crash`
  - emitted by the panic hook
  - current properties: `category = "panic"`, `fatal = true`

for sentry, the event message is just the event name (`app_error` or `app_crash`). the full json properties are attached as scope extras, along with `event_id` and `build_channel`.

### coverage assessment

what this telemetry is good enough to answer:

- session start/end cadence
- activation and retention based on active usage over time
- active vs paused vs snoozed duration within a session
- update-check and download interaction rates
- breathing preset adoption and custom-pattern usage

important caveats in the current implementation:

- if the user turns usage sharing off mid-session, the app sends the opt-out event first and then stops all further usage telemetry. that session will usually not get a later heartbeat or `session_end`.
- crash preference changes are recorded through the usage stream, not the crash stream. if usage sharing is already off, crash-toggle changes are not observable.
- panic handling uses a separate telemetry client created in `main()`. because it does not share the active session context, `app_crash` and `session_end(reason = panic)` are not linked to the real in-flight session and do not carry the true session durations.
- the `event_loop` `app_error` is emitted after `finish_session()` shuts the telemetry worker down, so it is effectively dropped.
- startup-failure errors happen before `session_start`, so those events have no `session_id`.
