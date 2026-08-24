# downshift github pages landing page

this folder is a no-build static site intended for deployment via GitHub Pages from `/docs` on the `main` branch.

## files

- `index.html`: single-page landing structure, content, and generated release metadata block
- `styles.css`: desktop-first styling
- `script.js`: embedded release metadata enhancement (no product-copy overrides)
- `release.json`: generated manifest for the latest stable release
- `assets/icon.png`: placeholder app icon (locally generated)
- `assets/mac-desktop-generic.svg`: placeholder desktop preview backdrop (locally generated)

## source-of-truth principle

- `index.html` is the source of truth for all user-visible product narrative and baseline links.
- the page must remain coherent and usable with JavaScript disabled.
- `script.js` may only enhance behavior that cannot be done statically (reading embedded release
  metadata and wiring download links).
- do not add JS-driven overrides for brand/product copy like app name, tagline, hero text, or trust claims.

## placeholder asset source/license

- `assets/icon.png` and `assets/mac-desktop-generic.svg` are original, locally generated placeholders created for this repo.
- they are safe to use as copyright-free placeholders and can be replaced later with final branded assets.

## enable GitHub Pages from `/docs`

1. open your repository on GitHub.
2. go to **Settings** → **Pages**.
3. under **Build and deployment**, set:
   - **Source**: Deploy from a branch
   - **Branch**: `main`
   - **Folder**: `/docs`
4. save. GitHub will publish the site URL.

## preview locally

GitHub Pages serves this folder as a no-build static site, so the local preview can use the same
files directly:

```bash
make pages-preview
```

Open <http://127.0.0.1:4173/> in a browser and press `Ctrl-C` in the terminal to stop the server. Use
`PAGES_PREVIEW_PORT=4174` if another local service is already using port 4173. The preview serves
the exact `/docs` directory that GitHub Pages publishes; it does not alter or build the site.

For fast validation, run:

```bash
make pages-check
```

The browser smoke test exercises the real `/docs` directory with macOS-only, macOS-plus-Windows,
malformed, missing, and JavaScript-disabled cases:

```bash
npm ci
npx playwright install chromium
make pages-smoke
```

## release sync approach (implemented): static release manifest with embedded copy

`docs/release.json` is the canonical release manifest. The release workflow generates it from the
published GitHub release and embeds an exact generated copy in `index.html`. `script.js` reads that
embedded JSON synchronously, so the first page load does not depend on a second network request.
The standalone `release.json` remains published for inspection, validation, and recovery tooling;
it is not loaded by the browser at runtime.

The manifest contains:

- stable version tag
- `.dmg` macOS download link, when present
- `.exe` Windows x64 download link, when present
- release notes link
- optional checksum link

The manifest is deliberately small and is validated in both Node-based tooling and the browser.
Only stable tags and HTTPS URLs for `samm81/downshift` are accepted. Static HTML has no
platform-specific direct download links or version text. A `<noscript>` block links to the latest
releases page when JavaScript is disabled. If embedded manifest parsing or validation fails, the
page shows a concise latest-releases fallback and hides direct download buttons. If a stable release
is missing a platform artifact, that platform's download card stays hidden. Prereleases never update the
manifest.

After a stable release is published, the unified release workflow uses its authenticated
`GITHUB_TOKEN` to regenerate `docs/release.json`, embed it into `docs/index.html`, commit both files
to `main`, and push them for GitHub Pages to deploy. The committed release metadata never contains
credentials or private release data. The update is idempotent and exact-tag based.

`release.json` uses these fields:

- `version`: required stable tag, such as `v0.2.0`.
- `release_url`: required matching GitHub release-notes URL.
- `published_at`: release publication timestamp.
- `macos_url`: nullable notarized `.dmg` asset URL.
- `windows_url`: nullable Windows `.exe` asset URL.
- `checksums_url`: nullable `SHA256SUMS.txt` asset URL.

To repair or deliberately move the manifest to an existing stable release without rebuilding
artifacts, run this from `main`:

```bash
make pages-release-manifest TAG=v0.2.0
```

That updates the working tree only. It regenerates the canonical manifest and its embedded copy.
Add `PUSH=1` to create the bot-style release-metadata commit and push it to `main` after verifying
the result.

### update content and release metadata

Edit `index.html` for product narrative, contact details, the email capture URL, and the
no-JavaScript fallback releases link. Do not hand-edit the generated release metadata block in
`index.html`. Platform download URLs, checksums, release notes, and the version are sourced from
`release.json` through the release workflow or the explicit tag-based recovery target above.

## notes

- all external links open in a new tab.
- the supported release targets are macOS Apple Silicon and Windows x64.
- Windows ARM64 is not currently supported.
- the website follows the latest stable release and does not automatically serve release candidates.
- this page explicitly states that there is no intensity feature.
