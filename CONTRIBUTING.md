# contributing

downshift is a native Rust desktop app with an embedded webview. the main app is in `src/`, the landing page is in `docs/`, and Windows packaging is in `windows/`.

## Local development

### macos bootstrap

the development environment uses a Linux host to provision a remote macOS checkout:

```bash
source .env
./dev/linux/bootstrap-01.bash
```

then, on the remote macOS checkout:

```bash
./dev/mac/bootstrap-02.bash
```

see [`dev/README.md`](dev/README.md) for Codex sync options and helper scripts. the bootstrap scripts may read `.env`; app code must only read environment variables supplied by the process.

### Run the app

use the no-telemetry target for ordinary local work:

```bash
make run-no-telemetry
```

add `RESET=1` to clear the saved position and settings before launch:

```bash
make run-no-telemetry RESET=1
```

use `make run` only when testing telemetry-enabled builds. that target requires the telemetry variables in `env.tmpl`; never commit their values.

### Preview the landing page

```bash
npm ci
make pages-preview
```

open <http://127.0.0.1:4173/>. set `PAGES_PREVIEW_PORT=4174` when port 4173 is already in use. run `make pages-check` to validate the release fixture and generated page.

## Checks

install the web tooling, then run the complete local gate:

```bash
npm ci
make verify-release
```

`make verify-release` runs Rust formatting, Rust tests, Rust Clippy, and the web, shell, Markdown, and Pages checks. `npm run check` covers formatting and linting only; it does not run Rust tests.

## Platform builds

### macOS

the packaged macOS app supports macOS 13 or later on Apple Silicon. build an unsigned local app bundle with:

```bash
make app-macos-no-telemetry
open dist/Downshift.app
```

build unsigned release archives with:

```bash
make release-no-telemetry
```

the command writes an unsigned zip, an unsigned DMG, and `dist/SHA256SUMS.txt`. pass `MACOS_TARGET=aarch64-apple-darwin` when the target must be selected explicitly.

the signed and notarized flow is reserved for releases. it requires the signing and notarization variables listed in `env.tmpl`; `make release-notarized` submits the DMG to Apple.

### Windows

the release target is Windows x64. run host-native checks from a Windows checkout:

```powershell
cargo build --release
cargo test
powershell -ExecutionPolicy Bypass -File .\windows\fast-check.ps1
```

build the unsigned installer with:

```powershell
powershell -ExecutionPolicy Bypass -File .\windows\build-installer.ps1
```

the installer uses a per-user install and installs the Microsoft Edge WebView2 Evergreen Runtime when it is missing. use `-SkipBuild` while iterating on an existing release binary. run `windows\smoke-installer.ps1` to exercise install, uninstall, shortcuts, and cleanup.

## UI checks

the app widget uses a native context menu on macOS and Windows. GUI smoke scripts must leave logs and screenshots under `logs/` when they fail. the platform workflows upload this evidence with an unconditional artifact step.

capture a short macOS demo clip from an interactive desktop session with:

```bash
./dev/mac/capture_demo_webm.bash 8
```

the capture script needs screen-recording and Accessibility permission for the terminal, plus `ffmpeg`.

keep the breathing animation aligned between the embedded app widget and the landing-page preview. the app implementation is in `src/main.rs`; the preview implementation is in `docs/styles.css` and `docs/polygon-animation.js`.

## Telemetry and build metadata

telemetry is emitted by the Rust runtime. usage data and crash reports have separate in-app controls. `telemetry.md` is the source-of-truth inventory for event names, payloads, sinks, and privacy behavior.

production builds require the metadata variables checked by `build.rs`, including the build channel, support URL, issues URL, download URL, and telemetry setting. values are compiled into the binary; changing the process environment after compilation does not change them. use `env.tmpl` as the variable list and keep all credentials out of Git.

## Releases

the app version lives in `Cargo.toml`. verify the repository before a release:

```bash
make verify-release
```

when a tag is supplied, it must match the Cargo version exactly:

```bash
make release TAG=v0.3.2
```

the unified workflow in `.github/workflows/release.yml` builds macOS and Windows artifacts, runs the platform smoke gates, publishes the GitHub release, and deploys the stable release to GitHub Pages. release publishing must not bypass the macOS GUI smoke or Windows installer smoke checks.
