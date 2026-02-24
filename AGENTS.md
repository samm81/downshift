# AGENTS.md

## project overview

downshift is a tiny desktop breathing companion. it renders a small, gentle visual breathing cue (an expanding/contracting ball) intended to reduce screen apnea and encourage a steady 5.5s inhale / 5.5s exhale rhythm.

## execution environment

the agent runs in an already-provisioned development environment.

linux is used only as an orchestration host to provision a remote mac.
all actual development work happens on the macos checkout.

## bootstrap policy (linux-driven)

the linux environment is allowed and expected to bootstrap the remote macos machine for this repo.

that means:

- run `dev/linux/bootstrap_macos.bash` from linux when bootstrap/provisioning is requested
- use the connection details in `.env`
- verify bootstrap end-to-end from linux
- after bootstrap completes, explicitly tell the user the machine is ready and they can remote-desktop in to start development

## codex requirement

the mac provisioning flow must install and verify `codex` on the remote macos host.

## normal work

once the environment is ready, the agent should proceed with normal repo tasks (editing files, running project commands, tests, and checks) in the current local checkout.

## testing

- by default, add or update tests in conjunction with code changes; do not treat tests as optional follow-up work.
- prefer `cargo test` as the default test command; it covers unit tests and non-gui integration tests.
- gui e2e is macos-only and experimental: run `./dev/mac/e2e_interactions.bash` (or `npm run test:gui:e2e:experimental`) only when explicitly needed.
- `npm run check` is lint/format only and does not run rust tests.
