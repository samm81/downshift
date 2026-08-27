# AGENTS.md

## project overview

downshift is a tiny desktop breathing companion. it renders a small, gentle visual breathing cue (an expanding/contracting ball) intended to reduce screen apnea and encourage a steady 5.5s inhale / 5.5s exhale rhythm.

update reminders are badge-based: dismissing the current update badge snoozes it for 24 hours, and users can separately choose not to be reminded about the current update version again from the updates menu.

## execution environment

the agent runs in an already-provisioned development environment.

linux is used only as an orchestration host to provision a remote mac.
all actual development work happens on the macos checkout.

## cross-platform development

- keep shared source code, build tooling, scripts, and release workflows Unix-first
- confine Windows-specific material as much as possible to clearly identified Windows-only files, targets, workflow sections, or conditional code
- on Windows, use `wsl` for Unix-oriented commands and scripts when needed; prefer the Unix toolchain over adding PowerShell or `cmd.exe` paths to shared workflows
- use PowerShell only for genuinely Windows-specific operations, such as Windows packaging, installer validation, or Windows-only system integration
- keep Unix scripts and configuration portable across Linux, macOS, and WSL, including consistent LF line endings

## workspace lifecycle

- Never create, remove, switch, or otherwise manage Codex workspaces or git worktrees on your own. Workspaces and worktrees must always be created and managed by Codex.
- Do not use `git worktree add`, manually create a replacement workspace directory, or create a feature-worktree workaround from the agent.
- Work only in the workspace provided by Codex. Do not create or check out new feature or release branches yourself.
- If the current workspace is the default workspace and the user asks to begin a substantial chunk of work, stop before making changes. Remind the user that starting a new Codex workspace may be preferable for isolation, then ask whether the agent should proceed in the current workspace or wait for a new Codex-managed workspace.
- Treat multi-file features, refactors, migrations, release work, and other changes likely to span multiple commits as substantial. Small read-only investigations and narrowly scoped edits may proceed without this pause.
- If the user explicitly chooses the current workspace, proceed there without creating another workspace or worktree. If they choose a new workspace, wait for Codex to provide it before continuing.
- When working in a non-default Codex-managed workspace, do not create additional workspaces or worktrees; use the workspace provided for the task.

## bootstrap policy (linux-driven)

the linux environment is allowed and expected to bootstrap the remote macos machine for this repo.

that means:

- run `dev/linux/bootstrap-01.bash` from linux when bootstrap/provisioning is requested
- use the connection details in `.env`
- verify bootstrap end-to-end from linux
- after bootstrap completes, explicitly tell the user the machine is ready and they can remote-desktop in to start development

## codex requirement

the mac provisioning flow must install and verify `codex` on the remote macos host.

## normal work

once the environment is ready, the agent should proceed with normal repo tasks (editing files, running project commands, tests, and checks) in the current local checkout.

- keep `telemetry.md` up to date whenever app telemetry, site analytics, telemetry sinks, event names, payloads, or privacy controls change

## env handling rule

- app/runtime code must not load, parse, or reference `.env` or any `.env.*` files.
- app/runtime code may only read environment variables provided by the process environment (including compile-time `option_env!` fallbacks when explicitly intended).
- prod-only release/build metadata must fail during compilation or build-script execution, never by aborting app startup at runtime.

## testing

- by default, add or update tests in conjunction with code changes; do not treat tests as optional follow-up work.
- prefer `cargo test` as the default test command; it covers unit tests and non-gui integration tests.
- `npm run check` is lint/format only and does not run rust tests.
- artifact-based mac gui verification is available through the `gui-smoke-macos` github actions workflow; it can target a specific release tag or the latest published dmg, launches `Downshift.app`, checks for a visible app window, triggers a warmup capture, and records cropped screenshot plus diff artifacts.
- release publishing is gated on `gui-smoke-macos`; `release-macos` must not dispatch `release-macos-finalize` unless that smoke workflow passes for the target tag.
- when adding a new feature, add telemetry events when they are meaningful for product/usage visibility.

## notarization policy

- never run notarization submission as part of agent work unless the user explicitly asks for it in that turn.
- specifically, do not execute `xcrun notarytool submit` or `make release-notarized` unless the user explicitly asks for it in that turn.
- standalone post-notarization verification is allowed when requested, including `make verify-notarized-dmg`.

## pre-commit policy

- treat pre-commit hook failures as first-class blocking issues.
- when hooks fail, attempt to fix all reported issues (including unrelated pre-existing issues) before committing.
- do not use `--no-verify` unless the user explicitly asks for it.

## ui constraints

- do not implement dialogs/tooltips/popovers that must escape the circular widget bounds inside the embedded webview html/css.
- the webview is clipped to the app window; overflow ui will be cut off.
- for explanatory/help content (for example, "What we collect…"), use a native window/dialog or a separate webview window instead of an in-webview modal.
