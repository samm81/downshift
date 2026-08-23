# Downshift Windows Migration Plan

## Goal

Port Downshift so the repository builds and ships both the existing macOS application and a consumer-ready Windows x64 application without regressing the macOS experience.

The Windows port will preserve the current user-facing behavior, including the native context menu, diagnostics clipboard copying, launch at login, single-instance activation, breathing-pattern dialogs, settings persistence, update checks, and telemetry behavior.

## Explicitly unsupported in this migration

- Windows Virtual Desktop/workspace-wide widget visibility is not supported. The Windows build intentionally keeps one widget window on the virtual desktop where it was launched; it does not create one widget per desktop or pin the widget to every desktop.
- This is a scope decision rather than a known defect. Implementing it would require multiple synchronized WebView2 windows and fragile Windows virtual-desktop APIs, followed by dedicated desktop-switching automation and VM coverage.
- macOS Spaces remain unchanged: the existing macOS implementation makes one window join all Spaces rather than creating duplicate processes or independent widgets.

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
- Add one cross-platform tagged-release orchestrator that owns the release tag, creates or updates one draft GitHub release, and coordinates the macOS and Windows build jobs.
- Keep platform-specific work reusable: macOS performs signing/notarization and its GUI smoke gate; Windows performs conditional Authenticode signing, Inno packaging, checksums, and installer smoke validation.
- Make the orchestrator publish only after both platform asset sets and all required release gates pass. The macOS finalize workflow must become subordinate to this gate or be folded into it so macOS cannot publish a release before Windows has contributed its assets.
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
- Treat launch-at-login as a registry/configuration assertion for now; the disposable Sandbox boots cleanly for each run but does not perform an in-guest Windows reboot. A persistent-VM reboot test is a separate follow-up.
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
- [x] Explicitly document Windows Virtual Desktop/workspace-wide widget visibility as unsupported for this migration.
- [x] Add the conditional Authenticode signing hook with an unsigned fallback.
- [x] Validate interactive and silent installer install/uninstall flows, shortcuts, registry entries, WebView2 runtime data cleanup, and installed-binary UI smoke.
- [x] Validate the Windows build, tests, packaging, and installer wizard/install/uninstall path on a GitHub-hosted Windows runner.
- [x] Run the macOS pull-request build, tests, Clippy, and unsigned app-bundle packaging on a GitHub-hosted macOS runner.
- [x] Add the unified tagged-release orchestrator with reusable macOS and Windows release jobs, one draft release, and one final publish gate.
- [x] Validate the release workflow/docs locally and run the branch macOS/Windows CI checks for the orchestrator change.
- [x] Provision and validate the interactive Windows VM.
- [ ] Complete coordinated tagged-release verification for macOS and Windows.

The current shell has Git and an authenticated GitHub CLI. Rust stable, rustfmt, the Visual C++ Build Tools MSVC linker, Node.js, and Inno Setup are installed. Windows Sandbox is enabled and provides a disposable clean Windows x64 desktop backed by Hyper-V for the full interactive installer and WebView2 UI smoke. The hosted Windows runner continues to validate the build and installer actions; its desktop cannot reliably provide a composited WebView2 surface or trustworthy visual screenshots for the installed-app UI smoke.

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

### 2026-08-22 — transparent resize hosted verification

- Branch: `codex/windows-port`
- Change: Pushed the transparent resize fix and its process-scoped resize regression smoke coverage as commit `b0a4c3a`.
- Validation: Windows run `32567232872` passed the Windows x64 build, Rust tests, Clippy, unsigned Inno build, and hosted installer smoke. macOS run `32567232889` passed the explicit macOS-target app-bundle build, tests, Clippy, and unsigned packaging.
- Result: The Windows-specific WebView2 host change does not regress the macOS target, and the hosted cross-platform checks remain green.
- Next: Provision the clean interactive Windows VM and run the installed-app screenshot checklist there; retain the WebView2-missing/bootstrapper scenario as a separate VM check.

### 2026-08-22 — Windows workspace visibility scope checkpoint

- Branch: `codex/windows-port`
- Change: Documented Windows Virtual Desktop/workspace-wide widget visibility as explicitly unsupported for this migration. The current Windows behavior remains one single-instance widget on the desktop where it was launched.
- Validation: Reviewed the platform code: macOS explicitly joins all Spaces, while Windows has no corresponding all-desktops implementation and keeps the existing single-instance guard.
- Result: The behavior is now recorded as an intentional scope boundary rather than an open migration defect.
- Next: Continue with the interactive VM and coordinated release verification work; revisit Windows workspace-wide visibility only as a separate feature.

### 2026-08-22 — Windows taskbar visibility checkpoint

- Branch: `codex/windows-port`
- Change: Configured the Windows widget window with Winit's `with_skip_taskbar(true)` so the breathing widget is not represented by a taskbar button.
- Validation: Local Windows fast checks passed. The complete scripted UI smoke passed on the rebuilt release binary; live Windows UI Automation found no `downshift` control under `Shell_TrayWnd` while the widget was running. Hosted Windows run `32580490471` and macOS run `32580490422` both passed.
- Result: The Windows widget remains visible and interactive without appearing in the taskbar; macOS behavior is unchanged.
- Next: Continue with the interactive VM and coordinated release verification work.

### 2026-08-23 — coordinated release orchestrator decision

- Branch: `codex/windows-port`
- Change: Confirmed that macOS and Windows should be released by one cross-platform tagged-release orchestrator rather than by independent workflows racing to update or publish the same draft release.
- Validation: Reviewed the existing workflow split: `release-macos` owns macOS signing/notarization and currently performs the final publication, while `build-windows` only runs branch/PR CI and unsigned installer smoke. The migration plan still has coordinated tagged-release verification pending.
- Result: The intended design is now explicit: one draft release, platform-specific build/sign/validation jobs, and one final publish gate requiring both platforms.
- Next: Implement the reusable Windows release job and cross-platform orchestration after the interactive VM gate is available.

### 2026-08-23 — unified release orchestrator implementation

- Branch: `codex/windows-port`
- Change: Converted the macOS release workflow into a reusable build/notarize/staple/verify/GUI-smoke job, added a reusable Windows release job with conditional Authenticode signing and installer smoke, added the top-level `release` orchestrator, and removed the independent macOS finalizer.
- Validation: The workflow structure and release asset contract are ready for static validation and a hosted dry run. No release tag was published during implementation.
- Result: Tagged releases now have one owner for draft creation and publication; the final gate requires both macOS and Windows artifacts and checksums. Windows signing is optional when both protected certificate secrets are absent and fails clearly on partial configuration.
- Next: Validate the workflow syntax, push the branch, and run a coordinated tagged-release verification before marking the release gate complete.

### 2026-08-23 — unified release branch verification

- Branch: `codex/windows-port`
- Change: Published the unified release implementation as commit `0a66c40`, including the reusable platform workflows, single draft/publish orchestrator, optional Windows signing, release documentation, and removal of the independent macOS finalizer.
- Validation: The new release YAML files passed Prettier parsing; the README and migration plan passed Markdownlint; the local Windows installer fallback produced `NotSigned` with no certificate; branch macOS run `32618373928` and Windows run `32618373929` both passed. A safe manual-dispatch probe confirmed GitHub will not dispatch a workflow that exists only on this non-default branch, and it created no release or draft.
- Result: The branch is ready for integration. The existing published `v0.1.28` tag must not be reused, and coordinated release verification remains pending for a new Cargo-version-matching tag after the workflow is available from the default branch.
- Next: Complete the interactive Windows VM gate, integrate the branch, then create and run the next version tag through `release`.

### 2026-08-23 — Windows startup console fix and VM host setup

- Branch: `codex/windows-port`
- Change: Added the Windows GUI-subsystem attribute so the per-user launch-at-login entry can start `downshift.exe` without opening a console window. Enabled the Hyper-V and Windows Sandbox Windows features on the local Windows 10 Pro host.
- Validation: Rebuilt `target\\release\\downshift.exe`; PE inspection reports Windows GUI subsystem value `2`, `cargo test --locked` passed all 70 tests, and a fresh launch produced no child `conhost.exe` process. The Hyper-V management tools and Windows Sandbox feature are enabled; a final reboot is required before launching the disposable test desktop.
- Result: The release binary should no longer produce the startup command prompt shown during login. The VM path is now Windows Sandbox, avoiding the impractical throttled 21.7 GB evaluation-image download.
- Next: Launch Windows Sandbox, run the full installer/UI screenshot smoke, exercise the WebView2-missing path, and record the artifacts.

### 2026-08-23 — Windows Sandbox qualification

- Branch: `codex/windows-port`
- Change: Provisioned a disposable Windows Sandbox test desktop with mapped installer/UI-smoke tooling and host WebView2 runtime staging. A clean guest exposed the need to statically link the MSVC runtime, so the Windows target now uses `-C target-feature=+crt-static` through `.cargo/config.toml`.
- Validation: The final Sandbox run passed the interactive Inno wizard with screenshots, installed-binary GUI smoke with scripted mouse/keyboard/UI Automation, resize regression (`large → small → large`), menu and updates interaction, silent install/uninstall, Start Menu shortcut, and uninstall-registry cleanup. The result is retained under `C:\Users\BBG\Documents\ChatGPT\downshift-vm\sandbox\logs\vm-installer-smoke-clientstate`. The release executable's PE dependency list contains no MSVC CRT or `WebView2Loader.dll` dependency.
- WebView2 note: The clean guest had no WebView2 runtime. The installer reached the Microsoft bootstrapper-download path, but the download did not complete in the disposable Sandbox network environment. The WebView2-present path was then validated by copying a known runtime into the guest and registering both the EdgeUpdate and ClientState metadata; the installed app passed the complete UI smoke.
- Result: The interactive Windows VM gate is complete. Windows Sandbox is retained for future local visual regression runs; it is disposable rather than a persistent development VM. Coordinated tagged-release verification remains the only migration checklist item still open.
- Next: Integrate the branch, create a new Cargo-version-matching tag (not the existing `v0.1.28`), and run the unified macOS/Windows release orchestrator.

### 2026-08-23 — one-command Windows Sandbox smoke

- Branch: `codex/windows-port`
- Change: Added the `smoke-windows-vm` Make target plus reusable host and guest PowerShell runners. The host runner builds and stages the current installer, generates a machine-specific Windows Sandbox configuration, launches the disposable guest, waits for the result, and reports the screenshot/log directory.
- Validation: The new host runner passed a complete Sandbox run: interactive installer, installed GUI smoke, resize regression, menus, updates, silent install/uninstall, Start Menu shortcut, and uninstall-registry cleanup.
- Result: The VM test can now be run with `make smoke-windows-vm` from the repository when GNU Make is available; the underlying command is `windows\smoke-vm.ps1`.
- Next: Integrate the branch, create a new Cargo-version-matching tag (not the existing `v0.1.28`), and run the unified macOS/Windows release orchestrator.

### 2026-08-23 — Windows Sandbox lifecycle and launch cleanup

- Branch: `codex/windows-port`
- Change: Made the Sandbox host runner close the disposable guest after pass or failure. The normal run now skips the visible pre-install WebView2 diagnostic probe; `-ProbeRuntime` keeps that probe available when diagnosing runtime startup. The installer smoke now waits for and stops the Inno post-install launch before starting the installed-app GUI smoke, and captures GUI-smoke PowerShell output for diagnosis.
- Validation: The default no-probe VM run passed interactive install, installed GUI smoke, resize regression, menus, updates, silent install/uninstall, Start Menu shortcut, and uninstall-registry cleanup. The host runner returned success with no Windows Sandbox processes left running.
- Result: A normal run now shows the installer and one installed-app GUI run, then closes automatically. The smoke verifies the launch-at-login registry value but does not reboot Windows; Windows Sandbox itself is a clean boot, not a persistent reboot test.
- Next: Add a persistent-VM reboot scenario only if launch-at-login behavior after an actual Windows restart becomes a release requirement; otherwise integrate the branch and run the coordinated tagged release.

### 2026-08-23 — `v0.2.0-rc.1` version bump

- Branch: `codex/windows-port`
- Change: Bumped the Cargo package and lockfile to `0.2.0-rc.1`, intended for a future `v0.2.0-rc.1` tag. Added a numeric Windows product-version mapping so Inno can package the prerelease as `0.2.0.1` while retaining the human-readable prerelease `AppVersion`.
- Validation: `cargo metadata --locked` and `cargo fmt --check` passed. The Windows release build and unsigned Inno installer compiled successfully as `Downshift-Setup-0.2.0-rc.1.exe`; no signing certificate was configured, so the installer remained `NotSigned`.
- Result: The branch now contains release-candidate version metadata suitable for a future tag. No tag, push, or release workflow was run.
- Next: Integrate the branch into the default branch, then explicitly run coordinated tagged-release verification for `v0.2.0-rc.1`.

### 2026-08-23 — prerelease release-workflow support

- Branch: `codex/windows-port`
- Change: Updated the unified release orchestrator and both reusable platform workflows to accept Cargo-compatible prerelease tags such as `v0.2.0-rc.1`, while continuing to require exact tag/Cargo version agreement. Documented the tag form in the README.
- Validation: The exact `make verify-release` target passed in a clean LF-normalized verification checkout: Rust formatting, 70 Rust tests, Clippy, Prettier, shfmt, ESLint, Stylelint, ShellCheck, and Markdownlint all passed. Workflow YAML passed Prettier validation, and the prerelease regex accepted `v0.2.0-rc.1`.
- Result: The planned RC tag will now reach the coordinated macOS/Windows release jobs instead of being rejected by the stable-version-only validation.
- Next: Commit these workflow changes, synchronize the release branch, push the branch and `v0.2.0-rc.1` tag, then inspect the GitHub release run and artifacts.

### 2026-08-23 — first tagged RC workflow test

- Branch: `codex/windows-port`, tag `v0.2.0-rc.1`, workflow run `32636338161`
- Change: Pushed the rebased release branch and tag. The unified orchestrator created the draft release and entered both platform jobs.
- Validation: Tag resolution and draft creation passed. Windows formatting, x64 build, 70 Rust tests, and Clippy passed, but installer packaging stopped because the workflow passed the release version through a positional PowerShell argument array and `0.2.0-rc.1` was interpreted as the `Configuration` parameter. macOS built through the unsigned app/package preparation, then stopped before signing because the configured `MACOS_CERT_P12_B64` and password did not decode to a valid PKCS#12 certificate.
- Result: The draft was not published. The Windows argument-binding defect is fixed in the working branch. The macOS signing error was initially attributed to the stored certificate/password, but the secret metadata shows they have not changed since the successful `v0.1.28` release.
- Next: Make macOS certificate validation compatible with the newer `macos-latest` OpenSSL 3 runner, then rerun the RC workflow.

### 2026-08-23 — macOS release signing regression diagnosis

- Branch: `codex/windows-port`
- Change: Compared the successful `v0.1.28` macOS release with RC run `32636338161`. The certificate validation recipe was unchanged, but `macos-latest` now resolves to the macOS 26 arm64 image, whereas the April release ran on the earlier macOS 15 arm64 image. The hosted image moved from OpenSSL 1.1.1 to OpenSSL 3.x, which requires the legacy provider for older PKCS#12 encryption formats.
- Validation: The current run received non-empty masked `MACOS_CERT_P12_B64` and `MACOS_CERT_P12_PASSWORD` environment values and failed specifically at `openssl pkcs12`; the GitHub secret update timestamps remain March 6–7, before the successful April release. The Makefile now retries PKCS#12 validation with `openssl pkcs12 -legacy` and preserves a safe OpenSSL diagnostic if both validations fail.
- Result: The likely regression is runner-image/OpenSSL compatibility, not automatic GitHub secret expiry. The certificate bytes and password remain protected; no secret values are logged.
- Next: Push the compatibility fix and rerun the coordinated RC workflow. If it still fails, use the emitted OpenSSL diagnostic to distinguish a malformed bundle from a password mismatch.

### 2026-08-23 — prerelease installer smoke path correction

- Branch: `codex/windows-port`
- Change: Updated `windows/smoke-installer.ps1` to derive its default installer filename from the root `Cargo.toml` version instead of retaining the historical `0.1.28` filename.
- Validation: The hosted Windows CI run `32638610808` built the current RC installer successfully, then exposed the stale default path when invoking the smoke script. The failure occurred before installer execution and was unrelated to the macOS signing change.
- Result: Local and branch installer smoke now remain aligned with prerelease and future version bumps when no explicit installer path is supplied.
- Next: Record the green branch checks, then run the coordinated release again with a new Cargo-version-matching tag; the existing `v0.2.0-rc.1` tag remains on the pre-fix commit.

### 2026-08-23 — release regression fixes verified by branch CI

- Branch: `codex/windows-port`, commit `a50edb0`
- Change: Published the OpenSSL 3 PKCS#12 compatibility fix and the prerelease installer-smoke path correction.
- Validation: macOS run `32639036444` passed the target build, tests, Clippy, and unsigned app packaging. Windows run `32639036441` passed formatting, Windows x64 build, Rust tests, Clippy, Inno packaging, and scripted installer smoke.
- Result: The branch is green after both release-workflow-adjacent regressions were corrected. No release was published and the old RC tag was not moved.
- Next: Create a new version-matching release-candidate tag and run the unified signed macOS/Windows release workflow.

### 2026-08-23 — release runner pinning

- Branch: `codex/windows-port`
- Change: Pinned macOS build, release, and GUI-smoke jobs to `macos-15` and pinned unified-release orchestration jobs to `ubuntu-24.04`. Windows jobs were already pinned to `windows-2022`.
- Validation: Workflow labels were reviewed across build, release, and GUI-smoke workflows. The macOS 15 pin preserves the Apple Silicon/OpenSSL environment used by the successful `v0.1.28` release while the `-legacy` compatibility fallback remains in place for future migrations.
- Result: Major hosted-runner migrations will now require an intentional workflow change rather than silently changing the release environment.
- Next: Run the full release verification, bump the version to `0.2.0-rc.2`, and execute the coordinated RC release.

### 2026-08-23 — `v0.2.0-rc.2` unified release attempt

- Branch: `codex/windows-port`, tag `v0.2.0-rc.2`, workflow run `32641523105`
- Change: Ran the tagged unified release orchestrator after the runner pinning and release-regression fixes.
- Validation: Tag resolution and draft creation passed. The Windows platform job passed x64 build, Rust tests, Clippy, Inno packaging, conditional unsigned-signing path, checksum verification, and scripted installer smoke. macOS passed repository verification, signed app/DMG creation, and certificate validation, then Apple notarization submission returned HTTP 403 because a required Apple Developer agreement is missing or expired.
- Result: The draft release was not published; the immutable RC2 tag has no release assets. This is an external Apple-account gate, not a repository or certificate-secret failure.
- Next: Accept the required Apple Developer agreement, then create the next Cargo-version-matching RC tag. The existing RC2 tag must remain unchanged.

### 2026-08-23 — macOS Homebrew trust hardening

- Branch: `codex/windows-port`
- Change: Set `HOMEBREW_NO_REQUIRE_TAP_TRUST=1` on macOS build, release, and GUI-smoke jobs so hosted-runner tap-trust policy changes do not interrupt tool installation.
- Validation: The setting is scoped to the macOS CI jobs that invoke Homebrew; RC2’s actual release blocker remained Apple notarization HTTP 403.
- Result: The next tagged candidate will carry the Homebrew CI hardening, while the Apple agreement remains the prerequisite for a publishable release.

### 2026-08-23 — RC2 rerun after Apple agreement acceptance

- Branch/tag: `codex/windows-port`, `v0.2.0-rc.2`; rerun of workflow `32641523105`
- Change: Reran the immutable RC2 tag after the Apple Developer agreement was accepted.
- Validation: Apple notarization submission, wait, stapling, DMG validation, and macOS artifact upload all passed. The Windows installer built and checksum/signature checks passed, but installer smoke failed during interactive uninstall cleanup because the test directory remained. The macOS GUI smoke launched the notarized app, found a visible window, captured screenshots, then rejected ImageMagick’s scientific-notation diff metric (`7.97426e+06`) as invalid.
- Result: The Apple account gate is resolved. The candidate still did not publish because both platform smoke gates failed; evidence artifacts were retained by the workflow.
- Next: Make the Windows process cleanup and macOS diff parser accept these hosted-runner conditions, verify locally, then issue a new Cargo-version-matching RC tag.

### 2026-08-23 — hosted smoke harness fixes

- Branch: `codex/windows-port`
- Change: Windows installer smoke now stops the installed app and directory-scoped WebView2 descendants before and during Inno uninstall cleanup. macOS GUI smoke now accepts ImageMagick diff metrics in either decimal or scientific notation.
- Validation: PowerShell and Bash syntax checks passed; Windows `npm run check:windows`, Cargo formatting, local RC2 installer build, and the complete `-SkipInstalledGui` installer smoke passed, including interactive and silent uninstall cleanup.
- Result: The two RC2 rerun failures are addressed without changing application behavior.
- Next: Run `make verify-release`, bump to `0.2.0-rc.3`, and rerun the unified release workflow.

### 2026-08-23 — RC3 release publish-job diagnosis

- Branch/tag: `codex/windows-port`, `v0.2.0-rc.3`; workflow run `32645990068`
- Validation: macOS repository verification, signed build, Apple notarization, stapling, DMG validation, artifact upload, and GUI smoke all passed. Windows formatting, x64 build, Rust tests, Clippy, conditional signing path, Inno packaging, checksum verification, installer smoke, and artifact upload all passed.
- Result: The final publish job failed before uploading assets because `gh release upload` was invoked in a job without a checkout and without an explicit repository, so GitHub CLI reported `fatal: not a git repository`. No release assets were published.
- Next: Make the upload command pass `--repo "$GITHUB_REPOSITORY"`, verify the workflow change, and issue the next immutable Cargo-version-matching RC tag.
