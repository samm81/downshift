# breath-ball

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

development setup and quality tooling are documented in `docs/dev-environment.md`.

## mac distribution (unsigned)

build local app bundle:

```bash
make app
open dist/BreathBall.app
```

build release archives:

```bash
make release
```

this creates:

- `dist/BreathBall-unsigned.zip`
- `dist/BreathBall-unsigned.dmg`
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
