# alpha analytics + diagnostics implementation plan

## summary

implement a small telemetry layer in the rust app with backend adapters so product code stays backend-agnostic. for alpha, send usage events to better stack logs and send crash/error diagnostics to better stack errors (sentry-compatible).

this implementation spec is limited to app-side behavior, schema, delivery, privacy controls, and tests.

## 1) implementation scope

- install/first run detection
- session start/end lifecycle tracking
- menu interaction tracking for: `pause`, `resume`, `size_change`, `reset`, `quit`, `analytics_menu`
- basic error/crash diagnostics
- split telemetry consent toggles (usage vs crash reports)

## 2) event/data model (pii-safe)

1. common fields on every event

- `schema_version` (e.g. `1`)
- `event_name`
- `event_id` (uuid v4)
- `occurred_at_utc` (rfc3339)
- `local_date` (`YYYY-MM-DD`)
- `local_tz_offset_min`
- `anon_user_id` (stable random uuid persisted locally)
- `session_id` (uuid per process launch; null only for pre-session startup failures)
- `app_version` (from `CARGO_PKG_VERSION`)
- `os`
- `arch`
- `build_channel` (`alpha`/`dev` via env/build flag)

1. event set

- `install_first_run`
  - properties: `usage_sharing_enabled_default`, `crash_sharing_enabled_default`
- `session_start`
  - properties: `launch_reason` (`manual`)
- `session_end`
  - properties: `reason` (`quit_menu|window_close|ctrl_c|startup_failure|event_loop_failure|panic|unknown`), `session_duration_sec`, `clean_exit`
- `menu_action`
  - properties:
    - `action` (`pause|resume|size_change|reset|quit|analytics_menu`)
    - `size_target` (`S|M|L|XL`, only for `size_change`)
- `privacy_preference_changed`
  - properties:
    - `setting` (`usage_data|crash_reports`)
    - `new_value` (`enabled|disabled`)
- `app_error`
  - properties: `category`, `severity` (`warn|error`), `recoverable`
- `app_crash`
  - properties: `category` (`panic` initially), `fatal=true`

1. pii/sensitive exclusions (hard rules)

- never send: window titles, file paths, typed text, raw ipc payloads, urls, monitor coordinates, hostnames, usernames, or raw error strings.
- send only whitelisted enum categories and bounded scalar values.

1. anonymous stable identifier

- store `anon_user_id` in `~/.config/downshift/telemetry.toml`.
- regenerate if missing/corrupt and emit `install_first_run`.
- no hardware fingerprinting.

## 3) consent UI/UX spec (menu + modal)

- `Help improve Downshift ▶`
- `Share anonymous usage data ✓`
- `Don’t share usage data`
- `---`
- `Share anonymous crash reports ✓`
- `Don't share crash reports`
- `---`
- `What we collect…`

behavior:

- exactly one choice active in each pair (usage data on/off, crash reports on/off).
- checkmark reflects persisted state.
- choices update immediately and persist to settings.
- toggling usage controls better stack logs pipeline.
- toggling crash reports controls sentry/better stack errors pipeline.

`What we collect…` opens a small modal in the webview with:

- title: `Anonymous usage data`
- body copy:
  - `We collect basic app usage (first run, session length, menu interactions) and anonymous error reports to improve Downshift. No camera/mic. No window titles, text, or browsing data.`
- button: `OK`

## 4) delivery design (portable + safe)

1. internal interfaces

- `TelemetryClient` trait:
  - `track(Event)`
  - `track_error(ErrorEvent)`
  - `start_session(SessionContext)`
  - `end_session(reason)`
  - `flush(timeout)`
  - `shutdown(timeout)`
  - `set_usage_enabled(bool)`
  - `set_crash_enabled(bool)`
- `TelemetrySink` trait:
  - `send_batch(&[Envelope]) -> Result<()>`
- sinks:
  - `BetterStackLogsSink` (usage)
  - `SentryErrorSink` (errors/crashes)
  - `NoopSink`

1. runtime architecture

- app emits only typed events.
- bounded in-memory queue.
- worker flushes on `max_batch_size` (e.g. 25) or `flush_interval` (e.g. 5s).
- exponential backoff + jitter on failure (`1s` to `60s`).

1. local durability/offline

- spool unsent usage events to `~/.config/downshift/telemetry-queue.ndjson`.
- cap queue (`1000` events or `2mb`), drop oldest on overflow.
- attempt drain on next launch.

1. shutdown semantics

- on menu quit/window close/ctrl-c: enqueue `session_end`, flush best-effort (`<=1500ms`).
- on panic: emit `app_crash`, flush crash pipeline (`~2s`) best-effort.
- telemetry failures never block or crash the app.

1. failure/rate behavior

- local throttle (`max_events_per_sec`, e.g. 5).
- if queue full or spool write fails: drop and continue.
- no telemetry error propagates to user-facing fatal path.

## 5) better stack integration

1. usage events (better stack logs)

- endpoint: `POST https://$INGESTING_HOST/`
- auth header: `Authorization: Bearer $SOURCE_TOKEN`
- `Content-Type: application/json`
- send batched structured json events.

1. errors/crashes (better stack errors via sentry)

- rust sentry sdk with DSN: `https://$APPLICATION_TOKEN@$INGESTING_HOST/1`
- send uncaught panic as fatal; categorized app errors as non-fatal.

1. config/env

- `DOWNSHIFT_TELEMETRY_ENABLED` (global kill switch)
- `DOWNSHIFT_BETTERSTACK_LOGS_TOKEN`
- `DOWNSHIFT_BETTERSTACK_LOGS_HOST`
- `DOWNSHIFT_BETTERSTACK_ERRORS_DSN`
- `DOWNSHIFT_BUILD_CHANNEL`

## 6) migration guardrails

- keep canonical rust event schema independent of backend wire format.
- enforce enum-based event names/actions; no free-form names.
- keep additive schema evolution via `schema_version`.
- do not leak backend-specific fields into app logic.

## 7) implementation sequence

1. add telemetry module, typed schema, and no-op sink.
1. add persistent identity and queue storage (`telemetry.toml`, ndjson spool).
1. add better stack logs sink and retry/backoff batching.
1. add sentry sink and panic hook wiring.
1. wire app lifecycle hooks in `src/main.rs` for install/session/error/crash events.
1. add menu action events for `pause`, `resume`, `size_change`, `reset`, `quit`, `analytics_menu`.
1. implement split consent submenu + `What we collect…` modal in webview html/js + new ipc commands.
1. persist new consent fields in settings and apply runtime enable/disable.
1. add/update docs for env vars and privacy behavior.
1. add tests.

## 8) tests and acceptance criteria

1. unit tests

- `anon_user_id` persistence/regeneration.
- session duration and end-reason mapping.
- backoff + queue overflow behavior.
- usage/crash toggles gate their respective pipelines.
- menu action mapping emits correct `menu_action.action` values.
- analytics submenu open emits `menu_action.action=analytics_menu`.

1. integration tests (mock http)

- logs sink sends auth + expected payload shape.
- retries after transient failure and drains when recovered.
- missing config gracefully degrades to noop.

1. crash/error tests

- subprocess panic test emits `app_crash` best-effort path.
- error category mapping never emits raw strings/paths.

1. acceptance

- menu copy and modal text match spec exactly.
- app remains stable when telemetry endpoints unavailable.
- no prohibited fields appear in emitted payload samples.

## references

- <https://betterstack.com/docs/logs/ingesting-data/http/logs/>
- <https://betterstack.com/docs/errors/collecting-errors/sentry-sdk/>
