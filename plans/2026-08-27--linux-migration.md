# Linux migration plan

Date: 2026-08-27  
Status: planned implementation after the entrypoint refactor

## Summary

Ship Linux as a best-effort portable build for common X11 and Wayland sessions without implementing every desktop environment independently.

The Linux implementation will use one host integration with runtime capability selection:

```text
shared application core
└── Linux host
    ├── X11/EWMH window
    ├── generic Wayland window
    └── optional Wayland layer-shell overlay
```

The normal window path is the compatibility baseline. Layer-shell is an enhancement selected only when the compositor, optional library, and drag behavior are verified. The app will never require a compositor-specific overlay in order to start.

The first Linux release will publish a canonical x86_64 downloadable tarball. AppImage, Flatpak, and native distro packages are deferred until real usage demonstrates that they are worth maintaining.

## Starting point

The entrypoint refactor has landed and provides useful boundaries:

- `src/app_core.rs` contains shared events, activity/update helpers, telemetry helpers, and pure application calculations.
- `src/window_policy.rs` contains monitor snapshots, size presets, persisted-position validation, and placement policy.
- `src/host/` contains common window/WebView helpers, menus, instance handling, launch-at-login, monitor conversion, platform configuration, and window operations.
- `src/main.rs` is smaller, but `App` still owns the concrete Winit `Window`/Wry `WebView` handles and calls host functions directly. Linux work may deepen this boundary where backend-specific ownership requires it; it should not repeat the original monolithic split.

Important current gaps:

- `src/host/platform.rs` uses Wry's non-Windows `build_as_child` path, which is not the Wayland-capable GTK embedding path.
- Linux currently has no native menu, no clipboard implementation, and a no-op launch-at-login implementation.
- `src/host/window.rs` assumes that absolute outer positioning and visibility operations are available on every platform.
- Settings persist physical window coordinates and a basic monitor fingerprint; Wayland overlay placement needs an output-relative representation.
- Ubuntu CI already installs WebKitGTK development packages for quality checks, but the full-quality and release pipelines only build macOS and Windows.
- Release metadata and the Pages site currently expose only `macos_url` and `windows_url`.

## Product and compatibility decisions

### Support promise

- Support common X11 and Wayland environments on a best-effort basis.
- Treat the generic Wayland window as a supported fallback, even when it cannot provide all macOS/X11 behavior.
- Do not promise identical always-on-top, absolute-position, or all-workspace behavior on every Wayland compositor.
- Keep macOS and Windows behavior unchanged.
- Target Linux x86_64 first. Add other architectures only after the build and runtime dependency story is stable.

### Workspaces, tiling, and window identity

- Do not implement one code path per desktop environment.
- Do not create one process or one window per workspace.
- Use the protocol/capability available from the session rather than treating `XDG_CURRENT_DESKTOP` as the primary behavior switch.
- Use a stable Linux application identity, `com.samm81.downshift`, and the existing `downshift` title so compositor rules can target the app.
- On X11, request the standard EWMH behaviors where supported: utility window type, above, sticky/all desktops, skip taskbar, and skip pager. A window manager may ignore any of these requests.
- On Wayland, use layer-shell only where the protocol is supported. Otherwise use a regular borderless window and document compositor-specific floating/sticky rules for users who need them.
- Provide concise recipes for KDE/KWin, Sway, Hyprland, i3, and common Xfce/MATE/Cinnamon setups. These are documentation, not runtime branches.

### Linux window mode

Add a persisted `linux_window_mode` setting with these values:

- `auto` (default): use layer-shell when support is available; otherwise use a normal window.
- `normal_window`: never attempt layer-shell.
- `overlay`: request layer-shell, but fall back to a normal window with a diagnostic if the capability is unavailable. The fallback must remain usable.

Unknown or malformed values must sanitize to `auto` without invalidating the rest of the settings file. Existing macOS, Windows, and legacy position fields remain compatible.

## Implementation plan

### 1. Finish the host boundary

Keep the shared event loop and application state in the common layer, but move backend-sensitive operations behind a focused host facade or capability-oriented modules.

The host boundary must cover:

- creation and destruction of the main window;
- creation of the update, snooze, breathing-pattern, and telemetry-info windows;
- Wry WebView construction and bounds synchronization;
- visibility, resizing, positioning, and drag operations;
- monitor/output discovery and placement updates;
- context-menu presentation and menu event translation;
- clipboard, external URL, and launch-at-login integration;
- platform diagnostics and capability reporting.

The shared core should retain activity/snooze state, settings, update behavior, telemetry, IPC dispatch, and pure placement policy. It must not contain desktop-environment conditionals or depend on raw GTK/X11/Wayland details.

Do not turn this into a single large `Platform` trait. Prefer small interfaces or a concrete host object with explicit capabilities. Keep `AppEvent::MenuActivated` and the existing high-level IPC semantics intact unless a Wayland-specific input limitation requires an atomic IPC change.

### 2. Establish the Linux GTK/WebView host

- Use the GTK-backed Wry embedding path required for Linux Wayland for the main WebView and every child WebView.
- Initialize and service GTK on the Linux event-loop path.
- Keep macOS and Windows WebView construction unchanged.
- Keep dialogs as regular native windows; do not make explanatory or settings content depend on an in-widget HTML popover.
- Ensure missing optional overlay support does not prevent GTK/WebKitGTK startup.

Wry documents `build_gtk` as the Linux path for Wayland support; the existing `build_as_child` path must not remain the only Linux implementation. See the references below.

### 3. Implement the X11 strategy

Use one X11 backend for GNOME-on-Xorg, KDE-on-X11, Xfce, MATE, Cinnamon, i3, bspwm, and similar sessions.

- Apply EWMH window properties after creation where the window manager supports them.
- Preserve the transparent, undecorated, fixed-size widget behavior.
- Preserve absolute placement and manual dragging using physical coordinates.
- Reapply relevant properties after recreation if necessary.
- Treat ignored window-manager requests as a documented limitation, not a startup failure.

The X11 backend should not contain branches for individual desktop environments.

### 4. Implement the generic Wayland strategy

- Detect the Wayland session through the actual window/display backend and available protocol capabilities.
- Use a regular borderless GTK/Wry window when layer-shell is unavailable or disabled.
- Do not claim that `AlwaysOnTop`, sticky workspaces, or global absolute positioning are guaranteed in this mode.
- Replace global browser `screenX`/`screenY` as the authoritative drag input with host-neutral local pointer deltas or an equivalent input model. Update the embedded JavaScript and Rust IPC atomically, with integration tests for the wire format.
- Preserve best-effort dragging where the compositor supports it. Report compositor limitations in diagnostics.
- Persist and restore placement using output-relative data when global coordinates are unavailable; retain existing physical coordinates for hosts that support them.

### 5. Add the optional layer-shell overlay

Use the GTK3 layer-shell C API because the current Wry/WebKitGTK integration is GTK3. Keep this dependency isolated from the shared core and make it optional at runtime.

- Check `gtk_layer_is_supported` before selecting the overlay.
- Use the overlay layer with a non-exclusive zone and no keyboard interactivity.
- Anchor the widget to an output edge using margins rather than relying on global window coordinates.
- Rebind the surface to a new output when a verified drag crosses outputs.
- Translate drag deltas into output selection plus anchor/margin updates.
- Select this backend when the runtime capability checks pass.
- If the library is absent, unsupported, or fails during setup, fall back to a normal window and retain a clear diagnostic reason.

The overlay is for the main breathing widget only. Child dialogs remain regular windows.

### 6. Complete Linux desktop integrations

- Enable the GTK-compatible native menu path for Linux where it avoids the clipped in-WebView menu. Keep menu IDs and `AppEvent::MenuActivated` stable.
- Use the GTK clipboard for copying diagnostics on both X11 and Wayland.
- Implement XDG user autostart at `~/.config/autostart/com.samm81.downshift.desktop`; enabling/disabling launch-at-login must be per-user, idempotent, and removable.
- Keep `xdg-open` for external URLs.
- Keep the existing Unix single-instance socket and ensure activation remains safe when the main window is hidden or snoozed.
- Add local diagnostics for session backend, selected mode, overlay support, and fallback reason. Do not collect desktop-environment or workspace identity as usage telemetry by default.

If usage or crash telemetry is changed later, update `telemetry.md` in the same change and preserve the existing privacy toggles.

### 7. Migrate settings and input safely

- Add `linux_window_mode` with a default of `auto` and tolerant sanitization.
- Add output-relative placement only for Linux backends that need it; do not remove or reinterpret existing `physical_x`, `physical_y`, legacy `x`, `y`, or monitor fields.
- Define a stable output matching order: connector/name when available, then monitor geometry/scale, then the primary output as fallback.
- Save placement after a successful host move, output change, resize, or mode transition.
- Avoid writing transient or invalid coordinates when a Wayland compositor does not report global position.
- Keep the embedded UI protocol versioned only if the drag-message change cannot remain backward-compatible; update IPC integration tests accordingly.

### 8. Add Linux build, packaging, and release support

The canonical first artifact is a downloadable tarball, not an AppImage.

Publish an artifact named in the form `Downshift-linux-x86_64-v<version>.tar.gz` containing:

- the `downshift` executable;
- a stable `.desktop` file;
- the application icon;
- `README-linux.md` with dependencies, installation, removal, and compositor notes;
- a rootless install helper or equivalent documented copy commands.

The install path must be user-writable by default, use a stable absolute executable path from the generated `.desktop` file, and avoid requiring `sudo`. Generate SHA-256 checksums alongside the existing release assets.

Document required GTK/WebKitGTK runtime packages by distribution family. The optional layer-shell library must be described separately and must not be required for the normal-window path.

Add a reusable Linux build workflow on Ubuntu that:

- installs the GTK/WebKitGTK development dependencies;
- runs formatting, tests, and Clippy;
- builds the x86_64 release binary;
- validates the tarball contents, `.desktop` file, executable bit, and checksums;
- uploads smoke-test evidence.

Add Linux to the full-quality gate and tagged release workflow. Extend the Pages release manifest and validation to support an optional `linux_url` ending in `.tar.gz`, and add a Linux download card only when that asset exists. AppImage and distro-specific packages remain follow-up work.

## Tests and acceptance criteria

### Automated tests

- Unit-test `LinuxWindowMode` serialization, defaulting, and malformed-value sanitization.
- Unit-test capability selection and fallback decisions for X11, generic Wayland, supported layer-shell, and missing layer-shell.
- Unit-test output-relative placement, output matching, output changes, and drag delta calculations.
- Preserve and run all existing library and integration tests.
- Test the Linux tarball manifest, desktop entry, install layout, and release-manifest `linux_url` validation.

### Linux runtime smoke tests

- X11: start under a headless X server with GTK/WebKitGTK, verify the widget renders, menus work, resize works, drag works, and the process exits cleanly.
- Generic Wayland: start under a Wayland compositor without relying on global coordinates, verify rendering, fallback mode, dialogs, and diagnostics.
- Layer-shell: run under a compositor with layer-shell support, verify overlay creation, output anchoring, movement across outputs, resize, and normal-window fallback.
- Optional dependency: repeat startup with the layer-shell library missing and confirm the app remains usable.

### Representative manual matrix

Run the release candidate on at least:

- GNOME Wayland: generic fallback and documented limitations;
- KDE Wayland: layer-shell when available, otherwise normal fallback;
- Sway or Hyprland: layer-shell and output movement;
- GNOME Xorg and KDE X11: EWMH behavior;
- Xfce, MATE, Cinnamon, and i3: X11 placement, floating behavior, and autostart.

Linux acceptance means the core experience works across this matrix; it does not mean every compositor provides identical workspace semantics.

## Rollout order

1. Use the completed entrypoint refactor as the baseline and finish only the host seam needed for Linux-specific window/WebView ownership.
2. Land GTK/Wry Linux hosting and a functional normal-window path.
3. Add X11 EWMH behavior, Linux menu/clipboard/autostart, and generic Wayland fallback behavior.
4. Add output-relative placement and host-neutral drag input.
5. Add the optional layer-shell backend behind capability checks.
6. Add Linux CI, tarball packaging, release assets, Pages metadata, and documentation.
7. Mark Linux as best-effort in release notes, collect community compositor recipes, and revisit AppImage/Flatpak/native packages only from concrete support demand.

## References

- [Wry Linux and GTK/WebView integration](https://docs.rs/wry/0.53.5/wry/)
- [Winit window limitations and positioning](https://docs.rs/winit/latest/winit/window/struct.Window.html)
- [Winit `WindowLevel` support](https://docs.rs/winit/latest/winit/window/enum.WindowLevel.html)
- [EWMH window-manager hints](https://specifications.freedesktop.org/wm/latest-single/)
- [Desktop Entry Specification](https://xdg.pages.freedesktop.org/xdg-specs/desktop-entry/latest-single/)
- [XDG Autostart Specification](https://zbrown.pages.freedesktop.org/xdg-specs/autostart-spec/latest/ar01s02.html)
- [wlr-layer-shell protocol](https://github.com/wlroots/wlroots/blob/master/protocol/wlr-layer-shell-unstable-v1.xml)
- [GTK4 layer-shell support matrix](https://github.com/wmww/gtk4-layer-shell)
- [KDE KWin window rules](https://docs.kde.org/stable_kf6/en/kwin/kcontrol/windowspecific/overview.html)
