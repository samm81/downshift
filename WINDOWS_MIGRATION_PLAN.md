# Downshift Windows Migration Plan

## Goal

Port Downshift so the repository builds and ships both the existing macOS application and a consumer-ready Windows x64 application without regressing the macOS experience.

The Windows port will preserve the current user-facing behavior, including the native context menu, diagnostics clipboard copying, launch at login, single-instance activation, breathing-pattern dialogs, settings persistence, update checks, and telemetry behavior.

## Success criteria

### Runtime

- The generic `cargo build --release` and debug build targets remain host-native and do not encode macOS as their default.
- The macOS release/package targets build with an explicit `--target "$(MACOS_TARGET)"`, preserving the architecture currently supported by the macOS release workflow.
- `cargo build --release --target x86_64-pc-windows-msvc` succeeds for Windows x64.
- The Windows application launches and renders the breathing widget through WebView2.
- The Windows widget remains transparent, always-on-top, draggable, resizable through the existing controls, and correctly positioned across monitors.
- The existing menu actions work on Windows, including pause, snooze, size, breathing patterns, diagnostics, updates, and quit.
- Diagnostics can be copied to the Windows clipboard.
- Launch-at-login works through the per-user Windows startup mechanism.
- A second launch activates the existing instance instead of opening a duplicate.
- Settings and logs use the normal Windows per-user locations and remain compatible with the existing settings schema.
- macOS behavior and release checks remain passing.

### Packaging and release

- An Inno Setup installer is produced for Windows x64.
- The installer is per-user, creates a Start Menu entry, supports clean uninstall, and does not require an administrator account for normal installation.
- The installer detects WebView2 and runs the Evergreen Bootstrapper only when needed.
- The Windows application and installer are signed when the protected signing secrets are configured; unsigned builds remain possible when those secrets are absent.
- Partial signing configuration fails clearly rather than silently producing an unexpected release.
- A tagged release publishes the macOS DMG, Windows installer, and SHA-256 checksums from one coordinated release.

### Verification

- Fast local Windows checks are available for repeated development iterations.
- Pull-request CI runs the fast checks on Windows and macOS.
- Local Windows scripted UI interaction and screenshots cover the running application without requiring a human operator.
- A clean Windows VM runs the same scripted interaction and screenshot checklist after installation, including WebView2 and uninstall scenarios, without requiring a human operator.
- Test logs and screenshots are retained as artifacts when CI or release checks fail.

## Plan

### 1. Baseline and branch hygiene

- Keep all work on `codex/windows-port`; do not push implementation commits directly to `main`.
- Keep this file current as the migration progresses.
- Use the existing `main` commit as the compatibility baseline and preserve unrelated repository changes.
- Keep the generic build entry points host-native; add dedicated macOS build/package entry points that pass the selected `MACOS_TARGET` explicitly to Cargo.
- Install or provision missing local prerequisites as needed: Rust/MSVC, Node.js, Inno Setup, and an interactive Windows VM.

### 2. Platform layer

- Introduce an internal platform abstraction for Windows and macOS-specific services.
- Enable the existing `muda` menu implementation on Windows as well as macOS.
- Implement Windows native clipboard copying.
- Implement launch-at-login through `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run`.
- Implement Windows single-instance ownership with a named mutex and activation forwarding through a named pipe.
- Preserve the existing IPC command and settings formats.
- Add Windows-specific diagnostics and actionable WebView2 startup errors.

### 3. Fast local development checks

Add a PowerShell fast-check/smoke workflow that can be run repeatedly without a VM or installer:

- `cargo fmt --check`
- `cargo build --release` using the native Windows toolchain for a release-build iteration.
- `cargo check --target x86_64-pc-windows-msvc`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `npm run check`
- Launch the debug application with isolated temporary settings/log paths.
- Verify process/window startup and clean shutdown.
- Exercise platform helpers and single-instance behavior with cleanup-safe tests.
- Run a scripted Windows UI smoke test that drives the local app and captures screenshots of the widget, dialogs, and context menu. No human clicks or keyboard input are required.

The fast path must not rebuild or install a VM, run notarization, require signing secrets, or reinstall WebView2 on every iteration.

### 4. Windows packaging

- Add a reusable Windows PowerShell packaging script.
- Add an Inno Setup script for the per-user x64 installer.
- Make macOS packaging consume the explicit `MACOS_TARGET` build output rather than relying on an implicit host build.
- Detect WebView2 and invoke the Evergreen Bootstrapper only when it is missing.
- Validate installer version against the Cargo version and release tag.
- Sign the application and installer conditionally with SHA-256 Authenticode signing and a timestamp service.
- Add silent install/uninstall tests that verify files, shortcuts, registry entries, launch behavior, and cleanup.

### 5. CI and release workflow

- Add Windows pull-request CI on `windows-latest` for compilation, tests, fast process/window smoke, and unsigned installer validation.
- Retain macOS pull-request validation.
- Retain the existing macOS notarization and GUI smoke gates for release candidates.
- Coordinate macOS and Windows release jobs through one draft release so assets and checksums are not published from competing workflows.
- Keep signing secrets restricted to the protected release environment.

### 6. Scripted local and VM GUI verification

Use the same smoke script, scripted interaction checklist, screenshot names, and log format locally and in the VM. The script—not a human user—performs the clicks, keystrokes, window activation, and screenshot capture.

Local Windows coverage:

- Run the debug or release executable directly.
- Drive dragging, resizing, pause/snooze, menu actions, breathing-pattern dialogs, diagnostics copying, and launch-at-login through Windows UI Automation/Win32 helpers.
- Capture screenshots and inspect logs after each meaningful UI milestone.

VM coverage:

- Use a clean Windows 10/11 x64 snapshot.
- Install the Inno package rather than running the binary from the source tree.
- Repeat the local interaction and screenshot checklist.
- Test WebView2-present and WebView2-missing states where practical.
- Test second-instance activation and uninstall cleanup.
- Retain screenshots and logs for comparison and diagnosis; do not require pixel-identical comparison between local and VM environments because DPI, fonts, themes, and display settings may differ.

GitHub-hosted Windows runners provide the routine clean CI environment. A local interactive VM is reserved for visual and installation confidence and is not part of every edit/test cycle.

## Current status

- [x] Confirmed the repository remote is `https://github.com/samm81/downshift.git`.
- [x] Checked out local branch `codex/windows-port` from `origin/main`.
- [x] Agreed to keep generic builds host-native and make the macOS target explicit.
- [x] Defined local and VM GUI verification as scripted UI automation, not human interaction.
- [x] Commit this plan file locally as `1c394a5`.
- [ ] Push `codex/windows-port` to GitHub after local GitHub CLI authentication is available.
- [ ] Install local Rust/MSVC, Node.js, GitHub CLI, and Inno Setup prerequisites.
- [ ] Implement the Windows platform layer.
- [ ] Add fast local smoke checks.
- [ ] Add Windows CI and packaging.
- [ ] Provision and validate the interactive Windows VM.
- [ ] Run macOS regression and release verification.

The current shell has Git and an authenticated GitHub CLI; Rust, Node.js, Inno Setup, and local VM tooling are not yet installed.

## Log

Append one entry for each meaningful migration action. Each entry should include the date, branch, change, validation performed, result, and next step.

### 2026-08-22

- Branch: `codex/windows-port`
- Change: Initialized the local checkout from `origin/main` and created this migration plan.
- Validation: Confirmed the branch is based on commit `0ad2fe6`; confirmed no pre-existing Windows migration branch.
- Result: Migration work can proceed without modifying `main`.
- Next: Commit this plan, install the local development prerequisites, then begin the Windows platform-layer implementation.

### 2026-08-22 — planning checkpoint

- Branch: `codex/windows-port`
- Change: Committed `WINDOWS_MIGRATION_PLAN.md` as `1c394a5`.
- Validation: `git diff --check` passed; the local branch remains separate from `main`.
- Result: The plan is safely recorded locally. The shell push was stopped because GitHub credentials are not configured for the bundled Git process.
- Next: Run `gh auth login` and `gh auth setup-git`, then push this branch without touching `main`.

### 2026-08-22 — build and GUI verification clarification

- Branch: `codex/windows-port`
- Change: Updated the plan to keep generic Cargo/Make builds host-native, require an explicit `MACOS_TARGET`/`--target` for macOS packaging, and replace “manual interaction” with script-driven UI automation for both the local Windows host and the Windows VM.
- Validation: Confirmed the authenticated GitHub CLI account is `samm81`; GitHub credential setup is available to the repository’s HTTPS remote.
- Result: Build intent and test ownership are unambiguous: the automation performs the UI actions and captures the evidence.
- Next: Commit and push this clarification, then begin prerequisite installation and platform-layer implementation.
