# dev environment

## source of truth

- use `dev-spec.md` as the entrypoint (it points to `docs/spec-v1.md`).

## one-time setup

on macos, run:

```bash
./dev/mac/bootstrap_homebrew.bash
./dev/mac/setup_dev_env.bash
```

this installs and verifies:

- `shellcheck`
- `shfmt`
- `pre-commit`
- `node` / `npm`
- `codex`

## daily checks

```bash
npm run check
```

this runs:

- shell format check (`shfmt`)
- shell lint (`shellcheck`)
- markdown lint (`markdownlint-cli2`)

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
