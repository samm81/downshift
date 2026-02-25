# plan: remote-friendly verification for downshift without live visual inspection

## summary
build a verification pipeline that runs fully on the mac host, produces machine pass/fail results, and emits compact artifacts (video + keyframes + diffs) for async review from linux. keep the current app binary behavior unchanged (no in-app test hooks). use a phased hybrid approach: strengthen existing scripts first, then add a framework-backed path only if reliability remains poor.

## current-state grounding
1. existing scripts already do gui capture and automation:
[dev/mac/smoke_gui.bash](/home/maynard/studio/dwsk/breath-ball/dev/mac/smoke_gui.bash), [dev/mac/e2e_interactions.bash](/home/maynard/studio/dwsk/breath-ball/dev/mac/e2e_interactions.bash), [dev/mac/capture_demo_webm.bash](/home/maynard/studio/dwsk/breath-ball/dev/mac/capture_demo_webm.bash).
2. known flake is explicitly documented around `System Events`/assistive access context mismatch:
[docs-dev/dev-environment.md](/home/maynard/studio/dwsk/breath-ball/docs-dev/dev-environment.md).
3. linux `cargo test` currently cannot be the full baseline due missing `gobject-2.0` on linux host; gui verification must stay mac-executed for now.

## implementation plan

### phase 1: make current smoke/e2e artifacts deterministic and reviewable
1. add `dev/mac/verify_visual_cycle.bash`:
- launch app (`cargo run --quiet`).
- locate app window bounds.
- record 18s raw screen at 30fps with `ffmpeg` avfoundation.
- crop to window bounds.
- extract frames at 12fps into `frames/`.
- compute per-frame mean luma for a fixed center crop and derive:
  - `motion_present` (variance > threshold),
  - `half_cycle_sec_estimate` (peak/trough spacing),
  - `period_ok` (`5.5s ± 0.7s`),
  - `paused_state_check` by toggling via existing right-click menu automation and confirming near-zero variance while paused.
- write `result.json` plus `downshift-demo.webm`, `keyframes/`, and `luma_timeseries.csv`.

2. upgrade `dev/mac/e2e_interactions.bash` assertions:
- keep drag/resize/pause checks.
- add menu speed checks by selecting `fast/default/slow` and validating cycle estimate changes (`4.5`, `5.5`, `6.5` within tolerances) using the same video-analysis helper.
- keep writing human-readable `result.txt`, and also write machine-parseable `result.json`.

3. standardize artifact layout for all gui checks:
- `logs/gui-verify-<timestamp>/`
- required files: `result.json`, `summary.txt`, `run.log`, `capture.webm`, `keyframes/*.png`.
- add a stable symlink or copy target: `logs/latest-gui-verify/`.

4. add one entrypoint script `dev/mac/run_verification_suite.bash`:
- run sequence: interaction e2e -> visual cycle verification.
- exit non-zero on first failure.
- always emit artifact index `logs/latest-gui-verify/index.txt`.

### phase 2: enable async linux-side review workflow
1. add `dev/linux/fetch_latest_gui_artifacts.bash`:
- ssh/scp from remote mac using `.env` connection vars.
- pull `logs/latest-gui-verify/` into local `artifacts/mac-gui/<timestamp>/`.
- print local paths to video and summary.

2. add optional html report generator `dev/linux/render_gui_report.bash`:
- read `result.json` files.
- render a single `report.html` with:
  - pass/fail table,
  - embedded video,
  - keyframes,
  - links to logs.

3. document “single command from linux”:
- run remote suite over ssh, fetch artifacts, open local report/video.
- this avoids watching remote desktop at 5fps.

### phase 3: framework-backed fallback (only if phase 1/2 still flaky)
1. add an optional `appium-mac2` lane in `dev/mac/e2e_appium/`:
- use webdriver + appium mac2 driver for native interactions and built-in screenshot/recording commands.
- scope to high-value flows only: launch, drag, resize, context menu pause/resume.
2. keep it opt-in behind `npm run test:gui:e2e:appium` and do not replace phase 1 scripts unless reliability data proves better over 20 consecutive runs.

## public interfaces / api changes
1. no app runtime api changes (`src/main.rs` / ipc unchanged by default).
2. new script interfaces only:
- `./dev/mac/verify_visual_cycle.bash [duration_sec]`
- `./dev/mac/run_verification_suite.bash`
- `./dev/linux/fetch_latest_gui_artifacts.bash`
- optional `npm` aliases for these scripts.

## test cases and scenarios
1. animation present:
- startup unpaused, motion variance above threshold, distinct phase extrema detected.
2. tempo correctness:
- default tempo estimates near 5.5s half-cycle.
- menu `fast` and `slow` produce expected shorter/longer half-cycle.
3. pause/resume:
- pause yields near-flat luma series; resume restores oscillation.
4. interaction behavior:
- drag moves window beyond minimum delta.
- coarse wheel resize delta >= 2 px.
- shift+wheel fine delta in 1-3 px.
5. persistence behavior:
- `settings.toml` reflects pause/speed/size changes and survives restart.
6. artifact integrity:
- every run emits mandatory files; suite fails if artifact set incomplete.

## acceptance criteria
1. from linux, one command produces a local artifact bundle with video + summary for the latest mac run.
2. suite can run 10 times on the remote mac with at least 9/10 pass rate (excluding explicit environment outages).
3. failures are diagnosable from artifacts alone (no live desktop required).

## assumptions and defaults
1. chosen defaults:
- strategy: hybrid phased.
- primary artifact: video + keyframes.
- app instrumentation: disallowed (no new in-app debug/test channels).
2. mac host has or will have required tools: `cargo`, `ffmpeg`, `cliclick`, `swift`, `osascript`.
3. gui session is available when executing mac gui tests (required by current automation model).
4. tolerance defaults:
- half-cycle tolerance `±0.7s`,
- pause variance threshold tuned empirically in first implementation pass and then frozen in docs.

## sources
1. appium mac2 driver docs (native mac automation commands and recording support): https://github.com/appium/appium-mac2-driver/blob/master/README.md
2. apple technical note on command-line test execution / session constraints: https://developer.apple.com/library/archive/technotes/tn2339/_index.html
3. apple wwdc “record, replay, and review your ui tests” (artifact-centric ui test review direction): https://developer.apple.com/videos/play/wwdc2025/316/
4. playwright visual comparison docs (`toHaveScreenshot`, animation controls): https://playwright.dev/docs/test-snapshots
5. ffmpeg filter documentation (`ssim`, `psnr`, `signature`) for machine visual checks: https://ffmpeg.org/ffmpeg-filters.html
