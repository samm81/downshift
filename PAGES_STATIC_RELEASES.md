# GitHub Pages static release manifest plan

## Goal

Replace the website's client-side request to the unauthenticated GitHub Releases API with a
small static release manifest committed under `/docs`.

The website will continue to show the latest stable release and platform-specific downloads, but
release metadata will be served by GitHub Pages from the same origin. Visitors will not consume
GitHub's public API quota, and the site will not depend on CORS behavior or a browser's shared
network egress address.

## Why this is needed

The current `docs/script.js` fetches:

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

- Replace the GitHub API URL in `docs/script.js` with `./release.json`.
- Fetch the manifest from the same origin with browser caching disabled or revalidated so a Pages
  deployment can pick up a new release promptly.
- Validate the required fields and accepted URL shape before updating the page.
- Reuse the existing platform-card behavior:
  - show the macOS card only when `macos_url` is present;
  - show the Windows card only when `windows_url` is present;
  - show checksums only when `checksums_url` is present;
  - link release notes to `release_url`.
- Keep the static HTML fallback usable if JavaScript is disabled.
- Keep the failure state concise and point users to the GitHub releases page.
- Remove the `api.github.com` dependency and its request-specific headers.

### 2. Seed and document the manifest

- Add `docs/release.json` for the current stable release.
- Document the schema and update ownership in `docs/README.md`.
- State that the release workflow owns updates after publication; manual edits are for recovery only.
- Add a JSON validation command to the local Pages checks.

### 3. Update the unified release workflow

Add a stable-only manifest update stage after both platform artifacts have been uploaded and the
GitHub release has been published.

The stage should:

1. Resolve the published release by its exact tag.
2. Read the release URL, publication timestamp, and exact asset names from GitHub using the workflow's
   authenticated `GITHUB_TOKEN`.
3. Select the notarized macOS DMG, Windows installer when present, and combined `SHA256SUMS.txt`.
4. Generate `docs/release.json` with deterministic formatting.
5. Check out the `main` branch, because GitHub Pages publishes `/docs` from `main`.
6. Commit the manifest as a bot change such as `chore: update Pages release manifest for v0.2.0`.
7. Push the commit to `main` using the existing workflow contents-write permission.

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
- validating the generated JSON before pushing it.

Recovery must be explicit and tag-based. It must not silently select a prerelease or an arbitrary
draft release.

### 5. Add local and hosted tests

Local iteration:

- `make pages-preview` serves the real `/docs` directory.
- Add a manifest validation check that confirms JSON syntax, required fields, HTTPS URLs, and the
  expected repository.
- Use the browser smoke test to confirm that the page loads without a GitHub API request or a
  `403` console warning.
- Test the seeded macOS-only manifest and confirm that the Windows card is absent.
- Test a temporary fixture containing a Windows `.exe` URL and confirm that both platform cards and
  both buttons appear.
- Test malformed and missing manifests and confirm the concise fallback state.

Hosted verification:

- Add the manifest validation to the Pages-related GitHub Actions checks.
- Add a hosted Pages smoke workflow that verifies the static manifest request returns `200`.
- Verify that the browser console has no request to `api.github.com`.
- After a test stable release, verify that the release workflow updates `main`, GitHub Pages deploys
  the new manifest, and the published website shows the new asset set.
- Verify that a prerelease leaves the previously published stable manifest unchanged.

## Rollout order

1. Add and validate the manifest schema and the initial current-stable file.
2. Switch the website from the GitHub API to the local manifest.
3. Run local macOS-only and Windows-present fixture smoke tests.
4. Add the release workflow manifest-update stage.
5. Run a test release or controlled stable-release rehearsal and inspect the workflow summary and
   Pages deployment.
6. Remove the old API-fetch code and update the migration/release documentation.

## Rollback

- If the manifest update is wrong, restore the previous valid `docs/release.json` and push a normal
  Pages update.
- If the website code is wrong, revert the website change while retaining the manifest for the next
  deployment.
- If the release workflow update fails after publication, run the authenticated recovery path for
  the published tag. Do not create another release tag solely to repair the website metadata.

## Current status

- Status: implementation complete locally; hosted workflow verification is pending.
- Working branch: `codex/pages-static-releases`.
- The website now fetches same-origin `docs/release.json`; it no longer calls GitHub's
  unauthenticated Releases API.
- The checked-in manifest describes the current stable `v0.2.0` release, with macOS and Windows
  assets plus checksums.
- The release workflow now updates `main` after a stable release is published. Prereleases leave
  the existing manifest unchanged.
- `make pages-preview`, `make pages-check`, `make pages-smoke`, and the tag-based
  `make pages-release-manifest` recovery target are available.
- Local manifest validation and browser smoke pass on macOS-plus-Windows, macOS-only, malformed,
  missing, and JavaScript-disabled cases.
- The new hosted Pages smoke workflow has not yet been run on GitHub from this branch.

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
