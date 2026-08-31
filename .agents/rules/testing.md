# testing

- add or update tests with code changes.
- do not defer tests as follow-up work.
- make smoke tests flake-resistant.
- poll for observable state transitions instead of using fixed readiness sleeps.
- check that the expected platform-native menu or dialog is open before interaction.
- preserve failure evidence for every smoke test.
- keep logs and a diagnostic screenshot or equivalent artifact.
- configure ci to upload the evidence with `always()`.
- add smoke-test coverage in the same change as every new feature.
- keep feature-level smoke scenarios in parity between macos and windows.
- use `cargo test` for rust unit tests and non-gui integration tests.
- use `npm run check` for linting and formatting. it does not run rust tests.
- the `gui-smoke-macos` github actions workflow can check a release tag or the latest published dmg.
- the workflow launches `Downshift.app` and checks for a visible window.
- the workflow triggers a warmup capture and records cropped screenshot and diff artifacts.
- do not let `release-macos` dispatch `release-macos-finalize` until `gui-smoke-macos` passes for the target tag.
