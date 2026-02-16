# Breathing Cue MVP Spec (Bare-Minimum PoC)

## Goal

Build the smallest possible proof of concept:

- A desktop app using `wry`
- A single WebKit/WebView window
- A circle that expands and contracts in a loop

That is all.

## In Scope

### Window

- Create one window on app launch
- Use `wry` + embedded webview content
- Borderless window
- Transparent background (best effort per platform)
- Fixed square size (no runtime resizing)
- Default position can be static (any reasonable corner or centered)

### Visual

- Render one circle in HTML/CSS
- Circle remains centered in the window
- Transparent page background

### Animation

- Continuous breathing loop
- Expand phase + contract phase
- Default total cycle: 11 seconds
- Smooth easing (`ease-in-out` is enough)

## Out of Scope (Explicitly Not in MVP)

- Drag-to-move
- Scroll-to-resize
- Right-click/context menu
- Pause/resume controls
- Speed controls
- Keyboard shortcuts
- Persistence/settings file
- Tray/menu-bar integration
- Multi-monitor restore logic
- Workspace/Spaces behavior tuning
- Packaging/runtime checks

## Technical Constraints

- Prefer inline HTML/CSS/JS loaded directly into the webview for fastest implementation
- No external assets required
- Keep code path simple and single-window only

## Acceptance Criteria

- App launches and opens one transparent `wry` window
- A centered ball visibly expands and contracts forever
- No interaction features are implemented beyond simply showing the animation
- App can be closed via normal OS close behavior
