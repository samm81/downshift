# downshift

a tiny desktop breathing companion: one small animated ball that gently expands and contracts to cue slower, steadier breathing while you work.

## motivation

when people focus on screens, they often unconsciously hold their breath or breathe shallowly (often called screen apnea). this project is meant to be a continuous, low-friction visual cue that nudges healthier breathing without interrupting flow.

the default rhythm is **5.5 seconds in / 5.5 seconds out** (11-second cycle), inspired by breathing cadence guidance discussed in james nestor's _breath_.

you can now open `breathing pattern…` from the widget context menu to switch between:

- `coherent breathing` (`5.5 / 0 / 5.5 / 0`)
- `box breathing` (`4 / 4 / 4 / 4`)
- `4-7-9` (`4 / 7 / 9 / 0`)
- `custom`, with the option to save named presets

## principles

- **tiny footprint**: small, calm, always-there companion
- **gentle over noisy**: subtle visual pacing, no aggressive prompts
- **low friction**: zero setup, open and breathe

## disclaimer

this is a wellness-oriented companion, not a medical device or medical advice.

## development

bootstrap flow (linux -> macos):

```bash
# step 1 (linux host)
source .env
./dev/linux/bootstrap-01.bash

# step 2 (on the remote mac checkout)
./dev/mac/bootstrap-02.bash
```

run the app locally:

```bash
make run
```

enable launch at login:

- open the widget context menu and toggle `start at login`
- this writes a per-user macos `LaunchAgent` and takes effect on the next login

reset saved position/settings before launching:

```bash
make run RESET=1
```

telemetry env vars (alpha analytics):

```bash
export DOWNSHIFT_TELEMETRY_ENABLED=true
export DOWNSHIFT_BETTERSTACK_LOGS_TOKEN='...'
export DOWNSHIFT_BETTERSTACK_LOGS_HOST='in.logs.betterstack.com'
export DOWNSHIFT_BETTERSTACK_ERRORS_DSN='https://<token>@<host>/1'
export DOWNSHIFT_BUILD_CHANNEL='alpha'
```

build-time app metadata:

```bash
export DOWNSHIFT_GITHUB_ISSUES_URL='https://github.com/samm81/downshift/issues'
export DOWNSHIFT_SUPPORT_EMAIL='support@example.com'
export DOWNSHIFT_DOWNLOAD_RELEASE_URL='https://github.com/samm81/downshift/releases/latest'
```

these values are compiled into the app binary via `option_env!`. changing them after the binary is built has no effect; rebuild the app to pick up new values.

breathing animation sync note:

- the breathing cue is implemented in two places: app webview (`src/main.rs`, `BREATH_HTML`) and docs preview (`docs/styles.css`).
- keep them aligned conceptually: the app now drives a four-phase pattern (`expand / expanded_hold / compress / compressed_hold`) from javascript, while the docs preview still shows the default coherent-breathing loop.

## capture short demo webm (mac)

capture a short motion clip of the running app and export a cropped webm:

```bash
./dev/mac/capture_demo_webm.bash 8
```

this writes artifacts under `logs/demo-capture-<timestamp>/`, including:

- `downshift-demo.webm`
- `raw.mp4`
- `result.txt`

requirements: macos desktop session with screen recording permission enabled for terminal (`ffmpeg` is installed by `./dev/mac/bootstrap-02.bash`).

## mac distribution (unsigned)

minimum supported macOS for packaged app builds: `13.0`

build local app bundle:

```bash
make app
open dist/Downshift.app
```

build release archives:

```bash
make release
```

this creates:

- `dist/Downshift-unsigned.zip`
- `dist/Downshift-unsigned.dmg`
- `dist/SHA256SUMS.txt`

the `.dmg` now stages:

- `Downshift.app`
- `Applications -> /Applications`
- a hidden Finder background image used for the drag-to-install window layout

so users get the standard drag-`Downshift.app`-to-`Applications` install flow when they open the disk image.

## mac distribution (signed + notarized dmg)

required env vars:

```bash
# generate from your .p12 file as a single line:
# base64 < developer-id.p12 | tr -d '\n' | pbcopy
export MACOS_CERT_P12_B64='...'
export MACOS_CERT_P12_PASSWORD='...'
export MACOS_KEYCHAIN_PASSWORD='...'
export MACOS_SIGNING_IDENTITY='Developer ID Application: Example, Inc. (TEAMID)'
export MACOS_NOTARY_APPLE_ID='name@example.com'
export MACOS_NOTARY_APP_PASSWORD='app-specific-password'
export MACOS_NOTARY_TEAM_ID='TEAMID'
```

build signed + notarized release archives:

```bash
make release-notarized
```

this creates:

- `dist/Downshift-signed.zip`
- `dist/Downshift-notarized.dmg`
- `dist/SHA256SUMS.txt`

## versioning and tag sync

rust stores the app version in `Cargo.toml` under `[package].version`.

`make release` reads version from `Cargo.toml` and writes it into the app bundle `Info.plist`.

to enforce that a git tag matches the cargo version, pass `TAG`:

```bash
make release TAG=v0.1.0
```

with `TAG` set, the release archives include the version in the filename:

- `dist/Downshift-unsigned-v0.1.0.zip`
- `dist/Downshift-unsigned-v0.1.0.dmg`

for notarized releases, filenames are:

- `dist/Downshift-signed-v0.1.0.zip`
- `dist/Downshift-notarized-v0.1.0.dmg`

this fails if:

- tag is `v0.1.0` but `Cargo.toml` version is not `0.1.0`

recommended release sequence:

```bash
# 1) bump cargo version first
# edit Cargo.toml -> version = "0.1.0"

# 2) verify packaging + tag sync
make release-notarized TAG=v0.1.0

# 3) commit, tag, push
git add Cargo.toml Cargo.lock Makefile README.md
git commit -m "release v0.1.0"
git tag -a v0.1.0 -m "release v0.1.0"
git push origin <branch>
git push origin v0.1.0
```

when the tag-triggered github actions release workflow runs, it creates the github release as a draft first and lets github generate release notes automatically. the draft is published after notarization finalize succeeds.
