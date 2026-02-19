# dev environment

## source of truth

- use `dev-spec.md` as the entrypoint (it points to `docs/spec-v1.md`).

## one-time setup

on macos, run:

```bash
./dev/mac/bootstrap_homebrew.bash
./dev/mac/setup_dev_env.bash
./dev/mac/bootstrap_gui_permissions.bash
```

this installs and verifies:

- `shellcheck`
- `shfmt`
- `pre-commit`
- `node` / `npm`
- `codex`
- `rustc` / `cargo`

the gui permissions step must be run from a non-headless desktop session. it intentionally triggers macos permission prompts for:

- automation (terminal controlling `System Events`)
- accessibility/ui scripting
- screen recording (used by screenshot-based gui smoke tests)

## daily checks

```bash
npm run check
```

this runs:

- shell format check (`shfmt`)
- shell lint (`shellcheck`)
- markdown lint (`markdownlint-cli2`)

## gui smoke test (macos)

to quickly verify the gui app launches and animates:

```bash
./dev/mac/smoke_gui.bash
```

optional args:

- first arg: screenshot count (default `3`, minimum `2`)
- second arg: interval seconds between screenshots (default `1`)

the script writes screenshots and a summary result file under `logs/gui-smoke-*/`.

## mcp servers

the following codex mcp servers are useful for this repo:

- `filesystem`: local repo file access
- `git`: repo-aware git operations
- `fetch`: fetch web/docs content
- `github`: github operations (requires token)
- `openaiDeveloperDocs`: streamable openai docs mcp endpoint

verify:

```bash
codex mcp list
```

note: this repo owns codex config at `dev/codex/config.toml`; bootstrap links `~/.codex/config.toml` to that file on the remote mac.

for github server auth, set:

```bash
export GITHUB_PERSONAL_ACCESS_TOKEN=your_token_here
```
