# downshift github pages landing page

this folder is the source for a static site deployed by the custom GitHub Pages Actions workflow.
The workflow copies this folder to a staging directory, generates release metadata there, and
deploys the staging directory as a Pages artifact.

## files

- `index.html`: single-page landing structure, content, and local release-metadata fixture
- `styles.css`: desktop-first styling
- `script.js`: embedded release metadata enhancement (no product-copy overrides)
- `../src/ui/polygon-animation.js`: shared animation source copied into the generated site
- `release.json`: checked-in stable-release fixture for local preview and smoke tests
- `assets/icon.png`: placeholder app icon (locally generated)
- `assets/mac-desktop-generic.svg`: placeholder desktop preview backdrop (locally generated)

## source-of-truth principle

- `index.html` is the source of truth for all user-visible product narrative and baseline links.
- the page must remain coherent and usable with JavaScript disabled.
- `script.js` may only enhance behavior that cannot be done statically (reading embedded release
  metadata and wiring download links).
- the Pages workflow owns the deployed release metadata; the checked-in manifest and embedded block
  are local fixtures and are not updated by releases.
- do not add JS-driven overrides for brand/product copy like app name, tagline, hero text, or trust claims.

## placeholder asset source/license

- `assets/icon.png` and `assets/mac-desktop-generic.svg` are original, locally generated placeholders created for this repo.
- they are safe to use as copyright-free placeholders and can be replaced later with final branded assets.

## enable GitHub Pages with GitHub Actions

1. open your repository on GitHub.
2. go to **Settings** → **Pages**.
3. under **Build and deployment**, set **Source** to **GitHub Actions**.
4. save. The `deploy-pages` workflow will publish the site URL.

## preview locally

The local preview builds the same staging shape as the Pages workflow, using the checked-in stable
release fixture:

```bash
make pages-preview
```

Open <http://127.0.0.1:4173/> in a browser and press `Ctrl-C` in the terminal to stop the server. Use
`PAGES_PREVIEW_PORT=4174` if another local service is already using port 4173. The generated preview
is written under `dist/pages`, which is ignored by Git.

For fast validation, run:

```bash
make pages-check
```

To build only the Pages artifact, run `make pages-build`.

The browser smoke test exercises the generated Pages artifact with macOS-only, macOS-plus-Windows,
macOS-plus-Windows-plus-Linux, malformed, missing, and JavaScript-disabled cases:

```bash
npm ci
npx playwright install chromium
make pages-smoke
```

## release sync approach (implemented): custom Pages build

The Pages Actions workflow builds a staging copy from the published GitHub release. It fetches the
latest stable release for ordinary `main` pushes, or receives the exact stable tag from the unified
release workflow, then generates `release.json` and embeds an exact copy in the staging
`index.html`. `script.js` reads that embedded JSON synchronously, so the first page load does not
depend on a second network request.

The checked-in `release.json` and embedded block are fixtures for local preview and smoke tests.
The deployed artifact is generated in Actions and is not committed back to `main`.

The manifest contains:

- stable version tag
- `.dmg` macOS download link, when present
- `.exe` Windows x64 download link, when present
- canonical Linux x86_64 `.tar.gz` download link, when present
- release notes link
- optional checksum link

The manifest is deliberately small and is validated in both Node-based tooling and the browser.
Only stable tags and HTTPS URLs for `samm81/downshift` are accepted. Static HTML has no
platform-specific direct download links or version text. A `<noscript>` block links to the latest
releases page when JavaScript is disabled. If embedded manifest parsing or validation fails, the
page shows a concise latest-releases fallback and hides direct download buttons. If a stable release
is missing a platform artifact, that platform's download card stays hidden. Prereleases never update the
manifest.

After a stable release is published, the unified release workflow calls the Pages build/deployment
workflow with its exact tag. No release-metadata commit is created. The generated release metadata
never contains credentials or private release data.

`release.json` uses these fields:

- `version`: required stable tag, such as `v0.2.0`.
- `release_url`: required matching GitHub release-notes URL.
- `published_at`: release publication timestamp.
- `macos_url`: nullable notarized `.dmg` asset URL.
- `windows_url`: nullable Windows `.exe` asset URL.
- `linux_url`: nullable canonical Linux x86_64 `.tar.gz` asset URL.
- `checksums_url`: nullable `SHA256SUMS.txt` asset URL.

To rebuild the deployed site for an existing stable release without rebuilding application
artifacts, run this from the repository's Actions tab after Pages is configured for GitHub Actions:

```bash
gh workflow run deploy-pages.yml --ref main -f release_tag=v0.3.0
```

The workflow validates the exact stable release, generates a temporary Pages artifact, and deploys
it without changing the working tree or creating a commit.

### update content and release metadata

Edit `index.html` for product narrative, contact details, the email capture URL, and the
no-JavaScript fallback releases link. The release metadata block and `release.json` in this folder
are local fixtures; the Pages build overwrites them in its staging artifact from the selected
published release.

## notes

- all external links open in a new tab.
- the supported release targets are macOS Apple Silicon, Windows x64, and Linux x86_64.
- Windows ARM64 is not currently supported.
- the website follows the latest stable release and does not automatically serve release candidates.
- this page explicitly states that there is no intensity feature.
