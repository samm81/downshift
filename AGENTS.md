# AGENTS.md

## project overview

breath-ball is a tiny desktop breathing companion. it renders a small, gentle visual breathing cue (an expanding/contracting ball) intended to reduce screen apnea and encourage a steady 5.5s inhale / 5.5s exhale rhythm.

## execution environment

the agent runs in an already-provisioned development environment.

the agent will be working in one of two contexts:

- linux workspace checkout
- macos workspace checkout

## hard rule: do not bootstrap macos

the agent must **never** bootstrap a remote macos machine itself.

that means:

- do not run `dev/boostrap_macos.bash`
- do not run `dev/bootstrap_macos_remote.bash`
- do not execute `ssh ... 'bash -s' < ...` bootstrap flows
- do not install or provision host-level dependencies on a rented mac from this repo

if bootstrap/provisioning is requested, the agent should stop and ask the user to run bootstrap manually, then continue once the environment is ready.

## normal work

once the environment is ready, the agent should proceed with normal repo tasks (editing files, running project commands, tests, and checks) in the current local checkout.

## user's preferences

- the user's writing style is lowercase (except for "I" and "I'm"), so comments should begin with lowercase characters instead of uppercase ones.
