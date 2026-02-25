# dev scripts

primary bootstrap path:

```bash
# linux host
source .env
./dev/linux/bootstrap-01.bash

# then on remote macos
./dev/mac/bootstrap-02.bash

# if tools like cargo are not in PATH yet in your current terminal:
exec zsh
```

notes:

- `bootstrap-01` syncs local `~/.codex/auth.json` to remote `~/.codex/auth.json` by default.
- to skip that sync, run with `SYNC_CODEX_AUTH=0`.

helper scripts are prefixed with `helper_` and are normally invoked by the bootstrap entrypoints:

- `dev/mac/helper_remote_repo_bootstrap.bash`
- `dev/mac/helper_install_homebrew_and_tools.bash`
- `dev/mac/helper_setup_dev_env.bash`
- `dev/mac/helper_gui_permissions.bash`

legacy entrypoints are kept as thin compatibility wrappers:

- `dev/linux/bootstrap_macos.bash`
- `dev/mac/bootstrap_macos_remote.bash`
- `dev/mac/bootstrap_homebrew.bash`
- `dev/mac/setup_dev_env.bash`
- `dev/mac/bootstrap_gui_permissions.bash`
