# [downshift](https://getdownshift.app)

**a quiet desktop breathing cue for steadier focus during long screen sessions**

keep a small animated cue on screen to guide your breathing without interrupting work.

## demo

see the interactive preview and current platform downloads at [getdownshift.app](https://getdownshift.app).

## install

open [getdownshift.app/#download](https://getdownshift.app/#download) and download the installer for your platform.

Linux x86_64 users can download the `Downshift-linux-x86_64-v<version>.tar.gz`
archive and run its rootless `./install.sh` installer. See
[README-linux.md](README-linux.md) for GTK/WebKitGTK requirements and
compositor notes.

## quickstart

1. install and open Downshift.
2. leave the cue visible while you work. it expands for 5.5 seconds as you breathe in and contracts for 5.5 seconds as you breathe out.
3. drag the cue to a comfortable position.
4. open its context menu to pause, snooze, resize, or choose another breathing pattern.

## modes

- **coherent breathing:** breathe in for 5.5 seconds and out for 5.5 seconds.
- **box breathing:** breathe in for 4 seconds, hold for 4, breathe out for 4, then hold for 4.
- **4-7-9:** breathe in for 4 seconds, hold for 7, then breathe out for 9.
- **custom:** set each phase and save named presets.

## controls

- **snooze:** hide the cue for 5, 10, 15, 30, 60, or custom minutes.
- **follow cursor:** keep the cue near the pointer while you work.
- **resize:** choose S (64px), M (96px), L (128px), or XL (160px), or scroll the cue to resize it.
- **start at login:** launch Downshift automatically with your desktop.

## privacy

Downshift runs without an account and does not use a camera, microphone, window titles, text, or browsing data. the native menu has separate controls for anonymous usage data and crash reports; see [telemetry.md](telemetry.md) for the current data inventory.

## requirements

- **macOS:** 13 or later on Apple Silicon.
- **Windows:** x64; the installer adds Microsoft Edge WebView2 when the runtime is missing.
- **Linux:** x86_64 with GTK3 and WebKitGTK 4.1; `gtk-layer-shell` is optional.

for local builds, tests, packaging, and release instructions, see [CONTRIBUTING.md](CONTRIBUTING.md).

## ai disclaimer

completely vibecoded.
