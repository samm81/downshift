# release

- if the user does not explicitly ask in the same turn, do not submit notarization as agent work.
- if the user does not explicitly ask in the same turn, do not execute `xcrun notarytool submit` or `make release-notarized`.
- run `make verify-notarized-dmg` only after the user requests standalone post-notarization checks.
- use the repository-local [release skill](../skills/release/SKILL.md) for the complete release workflow.
