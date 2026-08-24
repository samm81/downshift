# GitHub Pages static release manifest plan

## Goal

Replace the website's client-side request to the unauthenticated GitHub Releases API with a
small static release manifest committed under `/docs` and embedded into the landing page.

The website will continue to show the latest stable release and platform-specific downloads, but
release metadata will be generated from the canonical `docs/release.json` and made available in the
initial HTML. Visitors will not consume GitHub's public API quota, and the site will not depend on
CORS behavior, a second manifest request, or a browser's shared network egress address.

## Why this is needed

Before this work, `docs/script.js` fetched:

```text
https://api.github.com/repos/samm81/downshift/releases/latest
```

That request is unauthenticated. GitHub associates unauthenticated requests with the originating
IP and limits them to 60 requests per hour. The in-app preview browser received `403` responses,
while the local PowerShell request and authenticated `gh` request succeeded. Repeated reloads and
shared browser egress can therefore make the website appear broken even when GitHub, the repository,
and the release are healthy.

The website must never receive or embed the user's `gh` token.

## Success criteria

- The website makes no request to `api.github.com` at runtime.
- The website makes no runtime request to `release.json`; the embedded manifest is available during
  the initial page load.
- `docs/release.json` remains the canonical copy and matches the generated block in
  `docs/index.html` exactly.
- A stable release updates the static manifest after its release assets are published.
- Prereleases leave the stable manifest unchanged.
- The page shows the stable macOS asset when present.
- The Windows download card is hidden when the stable manifest has no Windows asset.
- The Windows download card appears when a stable manifest includes a Windows installer.
- Checksums and release notes continue to use the published release URLs.
- A malformed, missing, or stale manifest fails safely with a usable latest-releases link.
- Local Pages preview and hosted Pages smoke tests cover both macOS-only and macOS-plus-Windows
  manifest states.
- No credentials, signing material, or private release data are written into `/docs`.

## Proposed manifest

Add `docs/release.json` as the website's source for the current stable release. Keep the schema
small and explicit so the browser does not need to infer asset names.

Example:

```json
{
  "version": "v0.2.0",
  "release_url": "https://github.com/samm81/downshift/releases/tag/v0.2.0",
  "published_at": "2026-08-24T09:05:35Z",
  "macos_url": "https://github.com/samm81/downshift/releases/download/v0.2.0/Downshift-notarized-v0.2.0.dmg",
  "windows_url": "https://github.com/samm81/downshift/releases/download/v0.2.0/Downshift-Setup-0.2.0.exe",
  "checksums_url": "https://github.com/samm81/downshift/releases/download/v0.2.0/SHA256SUMS.txt"
}
```

Rules:

- `version` and `release_url` are required for a usable manifest.
- `macos_url` and `windows_url` are nullable platform assets.
- `checksums_url` is nullable and may be omitted when no checksum file exists.
- All URLs must be HTTPS URLs for the expected GitHub repository and release tag.
- The manifest represents the latest stable release only. Release candidates must not overwrite it.
- The checked-in manifest should describe the current stable release. At implementation time that
  release is `v0.2.0`, with macOS and Windows installers.

## Implementation plan

### 1. Add static manifest loading to the website

- Add a marked `<script type="application/json">` block to `docs/index.html`.
- Generate that block from the canonical `docs/release.json`.
- Read the embedded manifest synchronously in `docs/script.js`; do not fetch either GitHub's API or
  `release.json` at runtime.
- Validate the required fields and accepted URL shape before updating the page.
- Reuse the existing platform-card behavior:
  - show the macOS card only when `macos_url` is present;
  - show the Windows card only when `windows_url` is present;
  - show checksums only when `checksums_url` is present;
  - link release notes to `release_url`.
- Keep the static HTML fallback usable if JavaScript is disabled.
- Keep the failure state concise and point users to the GitHub releases page.
- Remove the `api.github.com` dependency and its request-specific headers.
- Keep `release.json` available as a directly inspectable and recoverable static artifact.

### 2. Seed and document the manifest

- Add `docs/release.json` for the current stable release.
- Document the schema and update ownership in `docs/README.md`.
- State that the release workflow owns updates after publication; manual edits are for recovery only.
- Add JSON and embedded-block validation commands to the local Pages checks.

### 3. Update the unified release workflow

Add a stable-only manifest update stage after both platform artifacts have been uploaded and the
GitHub release has been published.

The stage should:

1. Resolve the published release by its exact tag.
2. Read the release URL, publication timestamp, and exact asset names from GitHub using the workflow's
   authenticated `GITHUB_TOKEN`.
3. Select the notarized macOS DMG, Windows installer when present, and combined `SHA256SUMS.txt`.
4. Generate `docs/release.json` with deterministic formatting.
5. Embed the validated manifest into the marked block in `docs/index.html`.
6. Check out the `main` branch, because GitHub Pages publishes `/docs` from `main`.
7. Commit both files as a bot change such as `chore: update Pages release metadata for v0.2.0`.
8. Push the commit to `main` using the existing workflow contents-write permission.

Additional workflow requirements:

- Do not update the manifest for prereleases.
- Make the update idempotent. Rerunning a release must not create a new commit when the manifest is
  already current.
- Use a concurrency group or a rebase-and-retry strategy so two releases cannot overwrite each
  other's manifest update.
- Make a manifest-update failure fail the workflow summary even though the GitHub release itself may
  already be published. This makes the website update an explicit release gate and leaves a clear
  repair path.
- Keep the release asset upload and release publication before the manifest commit, so the website
  never points at assets that were not successfully published.
- Do not use a personal access token, signing secret, or any credential in the committed manifest.

### 4. Add recovery tooling

Add a small authenticated workflow step or Make target for rebuilding the manifest from an existing
stable tag. It should support:

- repairing a failed Pages-manifest update without rebuilding binaries;
- moving the manifest back to a previous stable tag;
- validating the generated JSON and embedded HTML block before pushing it.

Recovery must be explicit and tag-based. It must not silently select a prerelease or an arbitrary
draft release.

### 5. Add local and hosted tests

Local iteration:

- `make pages-preview` serves the real `/docs` directory.
- Add a manifest validation check that confirms JSON syntax, required fields, HTTPS URLs, and the
  expected repository.
- Use the browser smoke test to confirm that the page loads from its embedded manifest without a
  GitHub API request, a `release.json` request, or a `403` console warning.
- Test the seeded macOS-only manifest and confirm that the Windows card is absent.
- Test a temporary fixture containing a Windows `.exe` URL and confirm that both platform cards and
  both buttons appear.
- Test malformed and missing manifests and confirm the concise fallback state.

Hosted verification:

- Add the manifest validation to the Pages-related GitHub Actions checks.
- Add a hosted Pages smoke workflow that verifies the directly published static manifest returns
  `200` and matches the embedded manifest in the deployed HTML.
- Verify that the browser console has no request to `api.github.com`.
- After a test stable release, verify that the release workflow updates `main`, GitHub Pages deploys
  the new manifest, and the published website shows the new asset set.
- Verify that a prerelease leaves the previously published stable manifest unchanged.

## Rollout order

1. Add and validate the manifest schema and the initial current-stable file.
2. Generate and validate the embedded copy in the landing page.
3. Switch the website from the GitHub API to the embedded manifest.
4. Run local macOS-only and Windows-present fixture smoke tests.
5. Add the release workflow manifest-update stage.
6. Run a test release or controlled stable-release rehearsal and inspect the workflow summary and
   Pages deployment.
7. Remove the old API-fetch code and update the migration/release documentation.

## Rollback

- If the release metadata update is wrong, restore the previous valid `docs/release.json`, regenerate
  the embedded block, and push a normal Pages update.
- If the website code is wrong, revert the website change while retaining the manifest for the next
  deployment.
- If the release workflow update fails after publication, run the authenticated recovery path for
  the published tag. Do not create another release tag solely to repair the website metadata.

## Current status

- Status: embedded-manifest implementation complete on a follow-up branch; Windows-native local
  checks and browser smoke pass. Hosted verification is pending until the branch is pushed.
- Working branch: `codex/embed-pages-release-manifest`.
- Base commit: `75bd2ba`.
- The website now reads a generated embedded copy synchronously; it no longer requests either
  GitHub's unauthenticated Releases API or `release.json` at runtime.
- The checked-in manifest describes the current stable `v0.2.0` release, with macOS and Windows
  assets plus checksums.
- The release workflow now regenerates both `docs/release.json` and the embedded block in
  `docs/index.html` after a stable release is published. Prereleases leave the existing release
  metadata unchanged.
- `make pages-preview`, `make pages-check`, `make pages-smoke`, and the tag-based
  `make pages-release-manifest` recovery target are available.
- Local manifest validation and browser smoke pass on macOS-plus-Windows, macOS-only, malformed,
  missing, and JavaScript-disabled cases, including embedded and no-runtime-request assertions.
- Hosted Pages smoke passed on GitHub run `32729625828` for commit `75d6108`.
- The macOS build workflow passed on run `32729625780`; the Windows build, installer, and scripted
  smoke workflow passed on run `32729625726`.
- Final-tip macOS build passed on run `32731641555`, and the final-tip Windows build, installer, and
  scripted smoke workflow passed on run `32731641535`.
- Final-tip Pages browser smoke passed on run `32731950482`.
- The hosted workflow's live `https://getdownshift.app` check was correctly skipped on this branch;
  it runs only for pushes to `main` after Pages deployment.

## Change log

### 2026-08-24: plan created

- Documented the unauthenticated API quota problem.
- Chose a release-generated static manifest served from `/docs`.
- Recorded stable-only updates, recovery, testing, and rollback requirements.

### 2026-08-24: implementation on `codex/pages-static-releases`

- Added and validated `docs/release.json` for stable `v0.2.0`.
- Replaced the browser's unauthenticated GitHub Releases API request with same-origin manifest
  loading and defensive URL validation.
- Added authenticated, exact-tag manifest generation and recovery tooling.
- Added the stable-only release workflow update, idempotent bot commit, and serialized manifest
  update gate.
- Added local Playwright smoke coverage and a hosted Pages smoke workflow, including live deployed
  manifest checks on pushes to `main`.

### 2026-08-24: hosted verification

- Ran the Pages smoke workflow on GitHub for the branch; manifest validation, Chromium setup, and
  all browser fixture cases passed.
- Confirmed the existing macOS and Windows build workflows still pass on the implementation branch.

### 2026-08-24: final branch validation

- The first final-tip Windows run exposed an installer-smoke cleanup race: WebView2
  child processes could outlive `downshift.exe` and leave the temporary install directory locked.
- Updated `windows/smoke-installer.ps1` to terminate the full process tree before uninstall and to
  allow bounded filesystem cleanup. The exact CI-style and full local installer smokes passed.
- Re-ran macOS, Windows, and Pages smoke on `96e93a4`; all passed. The live hosted-site check remains
  intentionally deferred to the first push on `main`.

### 2026-08-24: embedded manifest follow-up

- Started `codex/embed-pages-release-manifest` from `main` at `75bd2ba`; `main` remains untouched.
- Added deterministic generation, embedding, removal, and validation helpers for the marked
  release-metadata block in `docs/index.html`.
- Updated the website, release updater, local checks, and hosted Pages smoke verification to keep
  `docs/release.json` and the embedded copy synchronized and to assert that browsers do not request
  `release.json`.
