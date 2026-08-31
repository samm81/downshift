# downshift

downshift is a tiny desktop breathing companion with a gentle visual cue and badge-based update reminders.

## AGENTS.md

- follow the progressive disclosure policy. target fewer than 50 lines for the top-level `AGENTS.md`. never exceed 100 lines
- limit the root `AGENTS.md` to:
  - one-line project description
  - package manager for projects that do not use npm
  - non-obvious commands only. skip standard commands such as `npm test` and `npm run build`
  - links to `.agents/rules/` files with brief descriptions
  - verification section
- put remaining instructions in `.agents/rules/` files by category
- do not put the following content in `AGENTS.md` or `.agents/` files:
  - api docs — link to external docs
  - code examples — agents can infer them from source files
  - interface and type definitions — keep them in the code
  - generic advice — remove phrases such as "write clean code"
  - obvious instructions — remove phrases such as "use typescript for `.ts` files"
  - redundant info — remove content already in the operating instructions
  - vague instructions — remove instructions that do not give an action

## package managers

- use `cargo` for the rust application and `npm` for web tooling and page scripts.

## commands

- `make verify-rust` — check rust formatting, tests, and clippy.
- `npm run check` — check web and shell formatting, linting, and generated pages.
- `make smoke-linux` — run Linux X11, Wayland, layer-shell, and fallback GUI smokes.
- `make verify-release` — run the full rust and web verification pass required before releases.
- `make pages-preview` — build and serve a local preview of the published pages.

## rules

- [development environment](.agents/rules/development-environment.md) — workspace, provisioning, and cross-platform boundaries.
- [testing](.agents/rules/testing.md) — test, smoke, ci, and release-gate expectations.
- [telemetry](.agents/rules/telemetry.md) — event and privacy inventory updates.
- [runtime and ui](.agents/rules/runtime-and-ui.md) — environment-variable and embedded-webview constraints.
- [release](.agents/rules/release.md) — release and notarization boundaries.

## verification

after making changes, run:

- `make verify-rust` — run rust formatting, tests, and clippy.
- `npm run check` — run web and shell checks plus pages validation.
- `make verify-release` — run the complete release verification pass for release-related changes.
