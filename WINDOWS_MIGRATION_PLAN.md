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

- Add Windows pull-request CI on `windows-latest` for compilation, tests, fast process/window checks, and unsigned installer validation. Keep full composited WebView2 UI automation on the local desktop and dedicated interactive VM because hosted runner desktops may not render WebView2 surfaces reliably.
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

GitHub-hosted Windows runners provide the routine clean CI environment for compilation and installer/wizard/install/uninstall checks. A local interactive VM is reserved for full visual confidence: it must run the installed-app WebView2 interaction and screenshot checklist and is not part of every edit/test cycle.

## Current status

- [x] Confirmed the repository remote is `https://github.com/samm81/downshift.git`.
- [x] Checked out local branch `codex/windows-port` from `origin/main`.
- [x] Agreed to keep generic builds host-native and make the macOS target explicit.
- [x] Defined local and VM GUI verification as scripted UI automation, not human interaction.
- [x] Commit this plan file locally as `1c394a5`.
- [x] Push `codex/windows-port` to GitHub after local GitHub CLI authentication is available.
- [x] Install Rust stable/MSVC tooling, Node.js, and Inno Setup prerequisites that do not require the Visual C++ workload.
- [x] Add the initial Windows platform layer: native menus, clipboard copying, launch-at-login, and single-instance activation.
- [x] Add an initial Windows x64 GitHub Actions build/test workflow.
- [x] Complete local MSVC workload installation and validate the Windows target locally and in GitHub Actions.
- [x] Add the first reusable unsigned Windows x64 Inno packaging path, including WebView2 detection/bootstrapper logic and SHA-256 output.
- [x] Add fast local Rust/web checks and a scripted local Windows GUI smoke test with screenshots.
- [x] Reproduce and fix the Windows transparent WebView2 stale-surface bug during repeated resize cycles; retain the regression screenshots in the local UI smoke test.
- [x] Add the conditional Authenticode signing hook with an unsigned fallback.
- [x] Validate interactive and silent installer install/uninstall flows, shortcuts, registry entries, WebView2 runtime data cleanup, and installed-binary UI smoke.
- [x] Validate the Windows build, tests, packaging, and installer wizard/install/uninstall path on a GitHub-hosted Windows runner.
- [x] Run the macOS pull-request build, tests, Clippy, and unsigned app-bundle packaging on a GitHub-hosted macOS runner.
- [ ] Provision and validate the interactive Windows VM.
- [ ] Complete coordinated tagged-release verification for macOS and Windows.

The current shell has Git and an authenticated GitHub CLI. Rust stable, rustfmt, the Visual C++ Build Tools MSVC linker, Node.js, and Inno Setup are installed. Local VM tooling has not yet been provisioned. The hosted Windows runner validates the build and installer actions; its desktop cannot reliably provide a composited WebView2 surface or trustworthy visual screenshots for the installed-app UI smoke, so that portion remains a local/interactive-VM gate.

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

### 2026-08-22 — branch publication checkpoint

- Branch: `codex/windows-port`
- Change: Published the migration branch to GitHub with commit `5a4b9c1`.
- Validation: `git push -u origin codex/windows-port` succeeded; the branch tracks `origin/codex/windows-port`; `main` was not modified.
- Result: The migration plan and its clarifications are available for review on the separate branch.
- Next: Install Rust/MSVC, Node.js, and Inno Setup, then begin the platform-layer implementation.

### 2026-08-22 — initial Windows platform checkpoint

- Branch: `codex/windows-port`
- Change: Added Windows-native menu support through `muda`, `clip.exe` diagnostics copying, per-user launch-at-login registration, named-mutex/named-pipe single-instance activation, explicit macOS target packaging, and the first Windows x64 build/test workflow in commit `09d008b`.
- Validation: `cargo fmt --check` passes. The first local Windows-target Cargo check reached compilation but was blocked by the missing `link.exe`; dependency resolution completed and the GitHub Actions run is queued.
- Result: The first platform implementation is on the separate branch; no changes were made to `main`.
- Next: Finish the local MSVC workload, rerun the Windows compile/tests, then add the fast scripted UI smoke path and installer.

### 2026-08-22 — Windows build and initial packaging checkpoint

- Branch: `codex/windows-port`
- Change: Finished the local MSVC workload, added `windows/installer.iss` and `windows/build-installer.ps1`, and wired the installer to detect WebView2 and run the Evergreen Bootstrapper only when the runtime is missing.
- Validation: Local `cargo check --locked`, release build, and Rust tests pass for `x86_64-pc-windows-msvc`; the GitHub Actions run `32561631885` passed formatting, release build, and all Rust tests. The full packaging script produced `dist/windows/Downshift-Setup-0.1.28.exe` and `dist/windows/SHA256SUMS.txt` without signing credentials; Inno Setup compiled successfully and reported `NotSigned` as expected.
- Result: The repository now has a repeatable unsigned Windows installer build and a tested conditional-signing hook. No certificate material is stored in the repository.
- Next: Add the fast local process/UI smoke checks, then exercise the installer and WebView2 paths with scripted screenshots locally and in a clean Windows VM.

### 2026-08-22 — fast checks and local GUI smoke checkpoint

- Branch: `codex/windows-port`
- Change: Added `windows/fast-check.ps1`, `windows/smoke-ui.ps1`, the Windows-safe `npm run check:windows` script, and Windows CI coverage for web checks and Clippy.
- Validation: `windows/fast-check.ps1` passed Rust formatting, Windows-target check/tests/Clippy, and Prettier/ESLint/Stylelint/Markdownlint. `windows/smoke-ui.ps1` passed locally against the release binary and produced inspected screenshots for the widget, native menus, diagnostics clipboard, and updates dialog; it also passed second-instance, pause/resume, and launch-at-login checks.
- Result: Windows development now has a fast repeatable path plus a scripted visual smoke path. The smoke script performs all UI input and restores the pre-test settings and Run registry value.
- Next: Test the compiled Inno installer with scripted install/uninstall interaction, provision the clean Windows VM, and run the same smoke flow there.

### 2026-08-22 — local installer smoke checkpoint

- Branch: `codex/windows-port`
- Change: Added `windows/smoke-installer.ps1` and documented the fast silent and full interactive installer test paths. The Inno script now removes the per-install WebView2 cache on uninstall.
- Validation: Rebuilt the unsigned installer with `windows/build-installer.ps1`. The scripted wizard passed locally with mouse automation and screenshots; the installed binary passed the full Windows GUI smoke checklist, including clipboard copying, second-instance activation, settings persistence, updates dialog, and launch-at-login. Silent install/uninstall then passed with Start Menu and Add/Remove Programs checks, and the WebView2 cache was removed.
- Result: Local installer packaging, interactive install, installed-app UI, silent install, and clean uninstall are covered without human interaction. The smoke output is retained under `logs/installer-smoke-windows-20260822-170742`.
- Next: Provision the clean Windows x64 VM, run the same installer and screenshot checklist there, then add/execute macOS regression coverage on a GitHub-hosted macOS runner.

### 2026-08-22 — hosted UI rendering checkpoint

- Branch: `codex/windows-port`
- Change: Added `-SkipInstalledGui` to `windows/smoke-installer.ps1`. Hosted Windows CI keeps the scripted installer wizard, install, silent install/uninstall, Start Menu, and registry checks, while the full installed-app GUI smoke remains enabled for local and interactive-VM runs.
- Validation: Local `windows/smoke-installer.ps1 -SkipInstalledGui` passed. macOS run `32565334868` passed. Windows run `32565334900` passed every build, test, packaging, and setup step, but its installed-app UI step failed after the hosted desktop created a blank WebView2 surface; hiding automation consoles, restoring focus, and adding a render-settle delay did not make that hosted surface interactive.
- Result: The failure is isolated to the hosted runner’s WebView2 compositing/input environment, not compilation or installer behavior. The CI workflow now retains reliable clean-runner installer evidence without masking the need for a real interactive VM UI run.
- Next: Push this CI adjustment, confirm the hosted Windows workflow is green, then provision the local interactive Windows VM and run the full installed-app screenshot checklist there.

### 2026-08-22 — cross-platform hosted CI green checkpoint

- Branch: `codex/windows-port`
- Change: Pushed the hosted-runner adjustment as `65cc720`; the Windows workflow now uses `-SkipInstalledGui` while retaining the full GUI path for local and interactive-VM runs.
- Validation: Windows run `32565568929` and macOS run `32565568971` both passed. The Windows artifact confirms scripted wizard navigation, interactive install, silent install/uninstall, Start Menu cleanup, and uninstall-registry cleanup. The hosted wizard screenshots were captured, but the runner desktop also shows blank/overlapping surfaces, so they are functional evidence rather than a visual rendering sign-off.
- Result: Cross-platform build/test/packaging CI is green. Local full GUI screenshots remain the reliable visual baseline; a real interactive Windows VM is still required for the second visual environment.
- Next: Provision the local Windows x64 VM, install the WebView2-present package there, run the full installed-app smoke, and then exercise the WebView2-missing/bootstrapper path.

### 2026-08-22 — Windows transparent resize regression checkpoint

- Branch: `codex/windows-port`
- Change: Reproduced the Windows sequence `large → small → large`, which showed a white opaque square retained at the previous WebView2 surface size. The Windows main WebView now uses Wry's automatic-resize path, and the transparent host window opts out of Winit's DWM redirection bitmap with `with_no_redirection_bitmap(true)`.
- Validation: `windows/fast-check.ps1 -Release` passed formatting, Windows-target check/tests/Clippy, web checks, and the release build. The local scripted smoke passed the size sequence `108 → 173 → 86 → 173`, all menu/input/clipboard/update/launch-at-login checks, and produced inspected initial, large, small, and restored screenshots without the white square. The full-screen screenshot also shows the desktop through the transparent corners.
- Result: The ball now resizes correctly in both directions and the stale opaque background is gone in the local Windows release path. The smoke harness is process-scoped so it cannot accidentally select another `downshift` window.
- Next: Commit and push this regression fix, run the hosted Windows/macOS CI again, then validate the installed build in the interactive Windows VM.
