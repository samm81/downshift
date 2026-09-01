# Downshift on Linux

Downshift supports Linux x86_64 with a GTK3 desktop and WebKitGTK 4.1. The
regular-window path is the compatibility baseline. X11 sessions add EWMH
window hints for an unobtrusive, above-and-sticky widget. Wayland sessions use
a borderless regular window unless the optional layer-shell path is selected
and supported by the compositor.

## Runtime requirements

Install the runtime libraries for your distribution before starting the
application:

- Debian or Ubuntu: `libgtk-3-0`, `libwebkit2gtk-4.1-0`, and `xdg-utils`.
- Fedora: `gtk3`, `webkit2gtk4.1`, and `xdg-utils`.
- Arch: `gtk3`, `webkit2gtk-4.1`, and `xdg-utils`.

Package names can differ between distributions and releases. The application
does not require `gtk-layer-shell`; it detects that library at runtime and
falls back to a regular window when it is missing.

## Layer-shell support

Layer-shell is an optional Wayland protocol for desktop surfaces. Downshift
uses it to place the breathing widget above normal application windows.

Known compatible compositor examples include:

- wlroots-based compositors, such as Sway, Wayfire, labwc, and river;
- Hyprland;
- niri;
- KDE Plasma Wayland with KWin;
- Mir-based compositors that enable `zwlr_layer_shell_v1`.

GNOME Wayland and X11 do not support layer-shell. The compositor must expose
`zwlr_layer_shell_v1`, and the `libgtk-layer-shell` runtime library must be
installed. Downshift probes both requirements at runtime.

If either requirement is missing, Downshift starts as a normal window. That
fallback does not provide a portable always-on-top or all-workspace guarantee.

See the [GTK layer-shell support matrix](https://github.com/wmww/gtk-layer-shell#supported-desktops)
and the [niri layer-shell documentation](https://github.com/niri-wm/niri/wiki/Layer%E2%80%90Shell-Components)
for compositor-specific details.

## GUI smoke tests

Contributors can run the X11, generic Wayland, layer-shell, and missing-library
smoke matrix with:

```bash
make smoke-linux
```

On Debian or Ubuntu, install the test-only dependencies with:

```bash
sudo apt-get install python3 xvfb xcompmgr xdotool x11-apps x11-utils imagemagick libmagickcore-6.q16-7-extra weston sway grim slurp grimshot jq wl-clipboard libnotify-bin libgtk-layer-shell0
```

The runner saves logs, screenshots, compositor state, and failure diagnostics
under `logs/`.

## Install and remove

Extract the release archive, then run the included installer as your normal
user:

```bash
./install.sh
```

The installer writes to `~/.local/share/downshift`, the per-user application
menu, and the per-user icon directory. It does not require `sudo`. Remove the
installation with:

```bash
./install.sh --uninstall
```

The application also provides a **Start at login** menu action. It creates an
XDG autostart entry under `~/.config/autostart` and removes that entry when the
setting is disabled.

## Window behavior

The Linux window mode setting accepts `auto`, `normal_window`, and `overlay`.
Unknown or malformed values use `auto`.

- X11 uses a regular GTK window with EWMH utility, above, sticky, skip-taskbar,
  and skip-pager hints.
- Wayland `normal_window` uses a borderless regular window. The compositor
  controls its global position, stacking, and workspace behavior.
- Wayland `overlay` requests `gtk-layer-shell`, but Downshift uses it only when
  the library and compositor support it. Otherwise it records the fallback
  and uses a regular window.

Wayland does not provide a portable promise for global coordinates, always-on-
top behavior, or all-workspace placement. Downshift stores output-relative
placement data so a monitor layout change can choose the closest matching
output.

## Compositor recipes

These examples are optional compositor configuration. Match the actual title
or application identifier shown by your compositor if it differs.

Sway or Hyprland:

```text
for_window [app_id="com.samm81.downshift"] floating enable, border pixel 0
```

i3:

```text
for_window [class="com.samm81.downshift"] floating enable, sticky enable, border pixel 0
```

KDE Plasma, Xfce, MATE, and Cinnamon provide equivalent per-window rules or
window-menu actions for floating, keeping a window above others, and showing
it on all workspaces. Use those rules only for the regular-window fallback;
layer-shell compositors decide overlay placement themselves.

AppImage, Flatpak, and distribution-specific packages are not part of this
first Linux release. The supported artifact is the x86_64 tarball named
`Downshift-linux-x86_64-v<version>.tar.gz`.
