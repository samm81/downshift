# development environment

- use the Codex-provided checkout for normal development.
- do not assume that the repository is on a remote macos host.
- for remote macos provisioning, use linux as the orchestration host.
- run `dev/linux/bootstrap-01.bash` from linux with connection details from `.env`.
- then run `dev/mac/bootstrap-02.bash` on the remote macos checkout.
- check the bootstrap from start to finish.
- tell the user that the machine is ready after bootstrap completes.
- the mac provisioning flow must install and check `codex` on the remote macos host.
- keep shared source code, build tooling, scripts, and release workflows unix-first.
- keep windows-specific material in identified windows-only files, targets, workflow sections, or conditional code.
- on windows, use `wsl` for unix-oriented commands and scripts.
- use powershell only for genuinely windows-specific packaging, installer, or system integration work.
- keep unix scripts and configuration portable across linux, macos, and wsl.
- keep line endings as lf.
- bootstrap scripts can read `.env`.
- application and runtime code cannot read `.env`.
- read the [runtime and ui](runtime-and-ui.md) rules for the complete requirement.
