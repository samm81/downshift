# downshift

a tiny desktop breathing companion: one small animated ball that gently expands and contracts to cue slower, steadier breathing while you work.

## motivation

when people focus on screens, they often unconsciously hold their breath or breathe shallowly (often called screen apnea). this project is meant to be a continuous, low-friction visual cue that nudges healthier breathing without interrupting flow.

the default rhythm is **5.5 seconds in / 5.5 seconds out** (11-second cycle), inspired by breathing cadence guidance discussed in james nestor's _breath_.

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

breathing animation sync note:

- the breathing cue is implemented in two places: app webview (`src/main.rs`, `BREATH_HTML`) and docs preview (`docs/styles.css`).
- keep them aligned: `5.5s` means one half-breath (inhale or exhale), and `alternate` provides the in/out cycle.

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

## versioning and tag sync

rust stores the app version in `Cargo.toml` under `[package].version`.

`make release` reads version from `Cargo.toml` and writes it into the app bundle `Info.plist`.

to enforce that a git tag matches the cargo version, pass `TAG`:

```bash
make release TAG=v0.1.0
```

this fails if:

- tag is `v0.1.0` but `Cargo.toml` version is not `0.1.0`

recommended release sequence:

```bash
# 1) bump cargo version first
# edit Cargo.toml -> version = "0.1.0"

# 2) verify packaging + tag sync
make release TAG=v0.1.0

# 3) commit, tag, push
git add Cargo.toml Cargo.lock Makefile README.md
git commit -m "release v0.1.0"
git tag -a v0.1.0 -m "release v0.1.0"
git push origin <branch>
git push origin v0.1.0
```
