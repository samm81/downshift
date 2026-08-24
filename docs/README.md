# downshift github pages landing page

this folder is a no-build static site intended for deployment via GitHub Pages from `/docs` on the `main` branch.

## files

- `index.html`: single-page landing structure and content
- `styles.css`: desktop-first styling
- `script.js`: release sync enhancement (no product-copy overrides)
- `assets/icon.png`: placeholder app icon (locally generated)
- `assets/mac-desktop-generic.svg`: placeholder desktop preview backdrop (locally generated)

## source-of-truth principle

- `index.html` is the source of truth for all user-visible product narrative and baseline links.
- the page must remain coherent and usable with JavaScript disabled.
- `script.js` may only enhance behavior that cannot be done statically (latest release fetch).
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
wsl make pages-preview
```

Open <http://127.0.0.1:4173/> in a browser and press `Ctrl-C` in the terminal to stop the server. Use
`PAGES_PREVIEW_PORT=4174` if another local service is already using port 4173. The preview serves
the exact `/docs` directory that GitHub Pages publishes; it does not alter or build the site.

## release sync approach (implemented): runtime fetch from GitHub Releases API

this page uses `script.js` to fetch:

- `https://api.github.com/repos/<owner>/<repo>/releases/latest`

and then auto-fills:

- latest version tag
- `.dmg` macOS download link
- `.exe` Windows x64 download link
- release notes link
- optional checksum link

there is no fallback download link/version in static html. if release fetch fails, the page keeps
direct downloads disabled and tells users to use the latest releases page. if a stable release is
missing a platform artifact, that platform's download card stays hidden. prereleases are not used
for the automatic download links.

### update placeholders

edit `index.html` and set:

- `REPO_URL`
- release notes URL (`#release-notes-link`)
- optional checksum URL (`#checksum-link`)
- contact email text
- email capture URL (`#email-capture-link`)
- no-js fallback releases link in `#download-help`

edit `index.html` data attributes for JS enhancement:

- `#download[data-github-api-latest-release]`

## notes

- all external links open in a new tab.
- the supported release targets are macOS Apple Silicon and Windows x64.
- Windows ARM64 is not currently supported.
- the website follows the latest stable release and does not automatically serve release candidates.
- this page explicitly states that there is no intensity feature.
