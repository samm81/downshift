# downshift github pages landing page

this folder is a no-build static site intended for deployment via GitHub Pages from `/docs` on the `main` branch.

## files

- `index.html`: single-page landing structure and content
- `styles.css`: desktop-first styling
- `script.js`: release sync enhancement + plausible event tracking (no product-copy overrides)
- `assets/icon.png`: placeholder app icon (locally generated)
- `assets/mac-desktop-generic.svg`: placeholder desktop preview backdrop (locally generated)

## source-of-truth principle

- `index.html` is the source of truth for all user-visible product narrative and baseline links.
- the page must remain coherent and usable with JavaScript disabled.
- `script.js` may only enhance behavior that cannot be done statically (latest release fetch, analytics wiring).
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

## release sync approach (implemented): runtime fetch from GitHub Releases API

this page uses `script.js` to fetch:

- `https://api.github.com/repos/<owner>/<repo>/releases/latest`

and then auto-fills:

- latest version tag
- `.dmg` download link
- release notes link
- optional checksum link

there is no fallback DMG link/version in static html. if release fetch fails, the page keeps direct
download disabled and tells users to use the latest releases page.

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

## plausible analytics setup

`index.html` includes the Plausible script tag directly (`https://plausible.io/js/pa-....js`).
if you rotate or replace your Plausible site script id, update that script URL in `index.html`.

tracked custom events:

- `download_click`
- `checksum_click`
- `email_capture_click`
- `faq_open`
- `github_click`
- `release_notes_click`

### verify events

1. open the published page in a browser.
2. click download/checksum/release notes/email/github links and open any FAQ item.
3. in Plausible, check **Events** for the custom event names above.

## notes

- all external links open in a new tab.
- this page explicitly states Apple Silicon only and that there is no intensity feature.
