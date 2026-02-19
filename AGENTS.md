# AGENTS.md

## project overview

breath-ball is a tiny desktop breathing companion. it renders a small, gentle visual breathing cue (an expanding/contracting ball) intended to reduce screen apnea and encourage a steady 5.5s inhale / 5.5s exhale rhythm.

## execution environment

the agent runs in an already-provisioned development environment.

the agent will be working in one of two contexts:

- linux workspace checkout
- macos workspace checkout

## bootstrap policy (linux-driven)

the linux environment is allowed and expected to bootstrap the remote macos machine for this repo.

that means:

- run `dev/boostrap_macos.bash` from linux when bootstrap/provisioning is requested
- use the connection details in `.env`
- verify bootstrap end-to-end from linux
- after bootstrap completes, explicitly tell the user the machine is ready and they can remote-desktop in to start development

## codex requirement

bootstrap must install and verify `codex` on the remote macos host as part of provisioning.

## normal work

once the environment is ready, the agent should proceed with normal repo tasks (editing files, running project commands, tests, and checks) in the current local checkout.

## user's preferences

- the user's writing style is lowercase (except for "I" and "I'm"), so comments should begin with lowercase characters instead of uppercase ones.
