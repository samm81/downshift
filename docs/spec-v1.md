# Breathing Ball v1 Spec

## Goals

- A tiny, always-available **visual breathing pacer** (expanding/contracting ball)
- **Transparent background**, minimal chrome
- **User-adjustable placement + size** without settings complexity
- Cross-platform desktop via `wry` (macOS, Windows, Linux/Wayland/X11) with best-effort “always on top”

## Non-goals (v1)

- Click-through
- Deep compositor integration (layer-shell)
- System-wide hotkeys (optional)
- Mobile platforms

---

# 1) Window behavior

## Window properties

- **Borderless**, no title bar
- **Transparent background**
- **Square window** (width == height)
- **Always-on-top**: best effort
  - If “always-on-top” fails on some WMs, still usable (user can keep it floating manually)

- **No taskbar/dock entry** where feasible (tray/menu-bar controls instead)
- **No focus stealing**
  - On show/restore, do not grab keyboard focus
  - On interaction, avoid disrupting typing as much as possible (platform-dependent)

## Placement defaults

- On first run:
  - Place in **top-right** of the primary monitor with a margin (e.g. 12–16px)
  - Default size preset: **M** (see size below)

- On subsequent runs:
  - Restore last position + size **per monitor** if possible
  - If saved monitor missing, fallback to primary monitor corner

## Size presets (for reset + menu)

- **S**: 24 px (logical px; will scale with DPI)
- **M**: 32 px
- **L**: 48 px
- **XL**: 64 px

(These are starting points; users can wheel-resize to anything.)

---

# 2) Visual + animation spec (HTML/CSS side)

## Visual design

- Draw a circle (SVG or div with border-radius: 9999px)
- Circle fill: semi-opaque (e.g. 0.35–0.6 alpha)
- Optional subtle outline: 1px with lower alpha
- Background must remain fully transparent

## Animation

- Breathing cycle: default **11.0 seconds total**
  - Inhale: 5.5s expand
  - Exhale: 5.5s contract

- Easing: **sinusoidal-like** (no linear corners)
  - CSS `ease-in-out` is acceptable v1
  - Better: a custom cubic-bezier that approximates sine

## Motion bounds

- Scale range: **0.65 → 1.0** (or similar)
- Keep some padding so the circle never touches the window edge

## Pause behavior

- Paused state should be visually obvious but subtle:
  - stop motion + slightly reduce opacity OR show a tiny “pause” notch

---

# 3) Interaction spec

## Drag-to-move

**Primary interaction**

- Drag gesture:
  - **Left click + hold 200ms**, then drag moves the window
  - The hold prevents accidental nudges

- Drag surface:
  - Only initiate drag when pointer down occurs **inside the circle** (not transparent margins)

- While dragging:
  - Show a faint window bounds outline (optional but helpful)
  - Animation can keep running or pause; recommended:
    - **Pause during drag**, resume on release (feels intentional)

## Scroll-to-resize (core)

**Mouse wheel** over the circle scales the widget.

- Increase size: wheel up
- Decrease size: wheel down
- Clamp: min 16px, max 160px (or similar)
- Modifier for fine adjust:
  - `Shift + wheel` = smaller step
- Resizing changes the **window size** (not just CSS scale), so hit-testing stays correct.

## Right-click menu (must-have)

Right-click anywhere on the circle opens a small menu:

- **Pause / Resume**
- **Speed…**
  - presets: 4.5/4.5 (fast), 5.5/5.5 (default), 6.5/6.5 (slow)
- **Size**
  - S / M / L / XL
- **Reset**
  - Reset position to default corner on current monitor
  - Reset size to M
  - Reset speed to default (optional separate item)
- **Quit**

(If tray/menu-bar exists, mirror these there too.)

## Optional keyboard shortcuts (nice-to-have)

If you can do it without global hotkeys:

- `Space`: pause/resume (when focused)
- `Esc`: reset position/size (when focused)

Global hotkeys can come later.

---

# 4) Multi-monitor + workspace behavior

## Monitor selection

v1: implicit

- The widget “belongs” to whichever monitor it’s currently on.
- Dragging across monitors should “just work.”

## Workspaces / spaces

- macOS: optionally “appear on all Spaces” (recommended)
- Linux: don’t fight WM; users can set sticky if they want
- Windows: normal always-on-top window behavior is fine

---

# 5) Persistence (settings storage)

Store:

- last size (px)
- last speed preset (cycle length)
- paused/running state
- last position:
  - ideally per-monitor, keyed by something stable (monitor name + resolution + scale)
  - fallback: store global x/y + last known screen bounds

Format:

- a tiny TOML file in platform-appropriate config dir

---

# 6) Packaging expectations (v1)

- Windows: handle missing WebView2 runtime gracefully (show a clear message)
- Linux: document dependency expectations (or ship via Flatpak later)
- macOS: if distributing broadly, you’ll eventually need signing/notarization (can be skipped for personal/internal use)

---

# 7) Acceptance criteria (what “done” means)

- Transparent, borderless widget appears on launch
- Smooth breathing animation at 11s cycle
- Drag works reliably with the hold threshold
- Wheel-resize changes actual window size, persists across restarts
- Right-click menu works everywhere
- No persistent focus stealing during normal usage
- Works on:
  - macOS
  - Windows
  - Linux (Wayland + X11) with “good enough” window behavior
