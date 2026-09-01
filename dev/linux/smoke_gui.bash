#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s <x11|wayland|layer-shell|missing-layer-shell> [binary] [output-dir]\n' "$(basename "$0")"
}

have() {
  command -v "$1" >/dev/null 2>&1
}

die() {
  printf '[linux-smoke] error: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '[linux-smoke] %s\n' "$*"
}

wait_until() {
  local description="$1"
  shift
  local attempts=120
  while ((attempts > 0)); do
    if "$@"; then
      return 0
    fi
    attempts=$((attempts - 1))
    sleep 0.25
  done
  die "timed out waiting for ${description}"
}

x11_display_ready() {
  xprop -root >/dev/null 2>&1
}

require_tools() {
  local required=(python3 jq identify convert)
  if [[ "$SCENARIO" == x11 ]]; then
    required+=(Xvfb xcompmgr xdotool xwd xprop xwininfo)
  elif [[ "$SCENARIO" == layer-shell ]]; then
    required+=(weston sway swaymsg grimshot grim slurp)
  else
    required+=(Xvfb xdotool xprop xwininfo sway swaymsg grimshot grim slurp)
  fi
  required+=(compare)

  local missing=()
  local tool
  for tool in "${required[@]}"; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  ((${#missing[@]} == 0)) || die "missing required tools: ${missing[*]}"
}

write_settings() {
  local mode="$1"
  local output_name="${2:-}"
  local output_width="${3:-}"
  local output_height="${4:-}"
  local output_scale="${5:-}"
  local settings_path="$XDG_CONFIG_HOME/downshift/settings.toml"

  mkdir -p "$(dirname "$settings_path")"
  {
    printf 'size = 96.0\n'
    printf 'paused = false\n'
    printf 'launch_at_login = false\n'
    printf 'linux_window_mode = %s\n' "\"$mode\""
    if [[ -n "$output_name" ]]; then
      printf '\n[linux_output_placement]\n'
      printf 'output_name = %s\n' "\"$output_name\""
      printf 'anchor = "top_right"\n'
      printf 'margin_x = 24\nmargin_y = 24\n'
      printf '\n[linux_output_placement.output]\n'
      printf 'width = %s\nheight = %s\nscale_factor = %s\n' \
        "$output_width" "$output_height" "$output_scale"
    fi
  } >"$settings_path"
}

start_x11() {
  local display_number=99
  while ((display_number < 120)); do
    export DISPLAY=":$display_number"
    if ! x11_display_ready; then
      Xvfb "$DISPLAY" -screen 0 2560x720x24 -nolisten tcp >"$OUT_DIR/xvfb.log" 2>&1 &
      XSERVER_PID=$!
      wait_until "Xvfb" x11_display_ready
      export GDK_BACKEND=x11
      export XDG_SESSION_TYPE=x11
      unset WAYLAND_DISPLAY
      xcompmgr -n >"$OUT_DIR/xcompmgr.log" 2>&1 &
      COMPOSITOR_PID=$!
      wait_until "X11 compositor" kill -0 "$COMPOSITOR_PID"
      return 0
    fi
    display_number=$((display_number + 1))
  done
  die "could not find an unused X display"
}

write_sway_config() {
  SWAY_CONFIG="$SMOKE_ROOT/sway.conf"
  {
    printf '%s\n' 'output * mode 1280x720'
    printf '%s\n' 'default_border none'
    printf '%s\n' 'default_floating_border none'
    printf '%s\n' 'focus_follows_mouse no'
    printf '%s\n' 'seat seat0 hide_cursor 0'
  } >"$SWAY_CONFIG"
}

start_sway_x11() {
  write_sway_config
  local display_number=120
  while ((display_number < 140)); do
    export DISPLAY=":$display_number"
    if ! x11_display_ready; then
      Xvfb "$DISPLAY" -screen 0 3840x768x24 -nolisten tcp >"$OUT_DIR/xvfb.log" 2>&1 &
      XSERVER_PID=$!
      wait_until "Xvfb" x11_display_ready
      break
    fi
    display_number=$((display_number + 1))
  done
  [[ "$display_number" -lt 140 ]] || die "could not find an unused X display"
  unset WAYLAND_DISPLAY SWAYSOCK WLR_WAYLAND_DISPLAY WLR_HEADLESS_OUTPUTS WLR_LIBINPUT_NO_DEVICES
  unset DISPLAY
  export GDK_BACKEND=wayland
  export XDG_SESSION_TYPE=wayland
  export XDG_CURRENT_DESKTOP=sway
  # Sway's X11 backend supplies a seat through XTest without physical devices.
  export WLR_RENDERER="${WLR_RENDERER:-pixman}"
  export WLR_BACKENDS=x11
  export WLR_X11_OUTPUTS=2
  export WLR_X11_SCALE=1
  export WLR_NO_HARDWARE_CURSORS=1
  export DISPLAY=":$display_number"
  sway -c "$SWAY_CONFIG" >"$OUT_DIR/sway.log" 2>&1 &
  COMPOSITOR_PID=$!
  wayland_display_ready() {
    find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -print -quit | grep -q .
  }
  wait_until "Wayland display socket" wayland_display_ready
  WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' -quit)"
  export WAYLAND_DISPLAY
  sway_socket_ready() {
    find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit | grep -q .
  }
  wait_until "Sway IPC socket" sway_socket_ready
  SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit)"
  export SWAYSOCK
  wait_until "Sway IPC" swaymsg -t get_version
  grimshot check >"$OUT_DIR/grimshot-check.log" 2>&1 || die "grimshot check failed"
}

start_sway_nested() {
  write_sway_config
  PARENT_WAYLAND_DISPLAY="downshift-parent-${BASHPID}"
  export WAYLAND_DISPLAY="$PARENT_WAYLAND_DISPLAY"
  unset DISPLAY SWAYSOCK
  export GDK_BACKEND=wayland
  export XDG_SESSION_TYPE=wayland
  export XDG_CURRENT_DESKTOP=sway
  export WLR_RENDERER="${WLR_RENDERER:-pixman}"
  weston \
    --backend=headless \
    --socket="$PARENT_WAYLAND_DISPLAY" \
    --width=2560 \
    --height=720 \
    --renderer=pixman \
    --no-config >"$OUT_DIR/weston.log" 2>&1 &
  PARENT_COMPOSITOR_PID=$!
  wait_until "parent Weston Wayland socket" test -S "$XDG_RUNTIME_DIR/$PARENT_WAYLAND_DISPLAY"
  wait_until "parent Weston compositor" process_running "$PARENT_COMPOSITOR_PID"
  export WLR_BACKENDS=wayland,headless
  export WLR_WAYLAND_DISPLAY="$PARENT_WAYLAND_DISPLAY"
  export WLR_HEADLESS_OUTPUTS=2
  export WLR_LIBINPUT_NO_DEVICES=1
  sway -c "$SWAY_CONFIG" >"$OUT_DIR/sway.log" 2>&1 &
  COMPOSITOR_PID=$!
  wayland_display_ready() {
    find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -print -quit | grep -q .
  }
  wait_until "Wayland display socket" wayland_display_ready
  WAYLAND_DISPLAY="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'wayland-*' -printf '%f\n' -quit)"
  export WAYLAND_DISPLAY
  sway_socket_ready() {
    find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit | grep -q .
  }
  wait_until "Sway IPC socket" sway_socket_ready
  SWAYSOCK="$(find "$XDG_RUNTIME_DIR" -maxdepth 1 -type s -name 'sway-ipc.*.sock' -print -quit)"
  export SWAYSOCK
  wait_until "Sway IPC" swaymsg -t get_version
  grimshot check >"$OUT_DIR/grimshot-check.log" 2>&1 || die "grimshot check failed"
}

start_display() {
  if [[ "$SCENARIO" == x11 ]]; then
    start_x11
  elif [[ "$SCENARIO" == layer-shell ]]; then
    start_sway_nested
  else
    start_sway_x11
  fi
}

x11_window_id() {
  local window_id geometry width height area
  local best_window=""
  local best_area=0
  while read -r window_id; do
    geometry="$(xdotool getwindowgeometry --shell "$window_id" 2>/dev/null)" || continue
    width="$(sed -n 's/^WIDTH=//p' <<<"$geometry")"
    height="$(sed -n 's/^HEIGHT=//p' <<<"$geometry")"
    [[ "$width" =~ ^[0-9]+$ && "$height" =~ ^[0-9]+$ ]] || continue
    area=$((width * height))
    if ((area > best_area)); then
      best_area=$area
      best_window="$window_id"
    fi
  done < <(xdotool search --onlyvisible --class '^com\.samm81\.downshift$' 2>/dev/null)
  [[ -n "$best_window" ]] && printf '%s\n' "$best_window"
}

x11_geometry() {
  local window_id geometry
  window_id="$(x11_window_id)"
  [[ -n "$window_id" ]] || return 1
  geometry="$(xdotool getwindowgeometry --shell "$window_id")"
  local x y width height
  x="$(sed -n 's/^X=//p' <<<"$geometry")"
  y="$(sed -n 's/^Y=//p' <<<"$geometry")"
  width="$(sed -n 's/^WIDTH=//p' <<<"$geometry")"
  height="$(sed -n 's/^HEIGHT=//p' <<<"$geometry")"
  [[ "$x" =~ ^-?[0-9]+$ && "$y" =~ ^-?[0-9]+$ && "$width" =~ ^[0-9]+$ && "$height" =~ ^[0-9]+$ ]] || return 1
  printf '%s %s %s %s\n' "$x" "$y" "$width" "$height"
}

sway_has_title() {
  local title="$1"
  swaymsg -t get_tree 2>/dev/null | jq -e --arg title "$title" '
    any(.. | objects;
      (.visible? == true) and
      (((.name? // "") | ascii_downcase) == ($title | ascii_downcase)
        or ((.app_id? // "") | ascii_downcase) == ($title | ascii_downcase)
        or ((.window_properties.title? // "") | ascii_downcase) == ($title | ascii_downcase))
    )
  ' >/dev/null
}

sway_main_geometry() {
  local geometry
  geometry="$(swaymsg -t get_tree 2>/dev/null | jq -r '
    [
      .. | objects
      | select(.visible? == true)
      | select(
          (((.app_id? // "") | ascii_downcase) == "com.samm81.downshift")
          or (((.name? // "") | ascii_downcase) == "downshift")
          or (((.window_properties.title? // "") | ascii_downcase) == "downshift")
        )
      | .rect
      | select(type == "object")
    ]
    | max_by(.width * .height)
    | "\(.x) \(.y) \(.width) \(.height)"
  ')" || return 1
  [[ "$geometry" =~ ^-?[0-9]+\ -?[0-9]+\ [0-9]+\ [0-9]+$ ]] || return 1
  printf '%s\n' "$geometry"
}

main_window_geometry() {
  if [[ "$SCENARIO" == x11 ]]; then
    x11_geometry
  else
    sway_main_geometry
  fi
}

main_window_width() {
  main_window_geometry | awk '{print $3}'
}

window_width_increased() {
  local before="$1"
  local current
  current="$(main_window_width)" || return 1
  [[ "$current" =~ ^[0-9]+$ && "$before" =~ ^[0-9]+$ ]] || return 1
  ((current > before))
}

layer_rendered_width_increased() {
  local before="$1"
  local geometry width
  rendered_capture layer-shell-resized || return 1
  geometry="$(trim_geometry "$OUT_DIR/layer-shell-resized.png")" || return 1
  width="$(geometry_value "$geometry" width)" || return 1
  [[ "$before" =~ ^[0-9]+$ ]] || return 1
  ((width > before))
}

window_exists() {
  if [[ "$SCENARIO" == x11 ]]; then
    if [[ "${1:-downshift}" == downshift ]]; then
      [[ -n "$(x11_window_id)" ]]
    else
      xdotool search --onlyvisible --name "$1" >/dev/null 2>&1
    fi
  else
    sway_has_title "${1:-downshift}"
  fi
}

capture() {
  local name="$1"
  local path="$OUT_DIR/$name.png"
  if [[ "$SCENARIO" == x11 ]]; then
    xwd -root -silent 2>/dev/null | convert xwd:- "png:$path" >/dev/null 2>&1 || return 1
  else
    grimshot save screen "$path" >/dev/null 2>&1 || return 1
  fi
  [[ -s "$path" ]] || return 1
  identify "$path" >/dev/null 2>&1 || return 1
  printf '%s\n' "$path"
}

window_absent() {
  ! window_exists "$1"
}

prime_wayland_input() {
  local output_window
  output_window="$(xwininfo -root -children 2>/dev/null | sed -n 's/^ *\(0x[0-9a-fA-F]*\).*/\1/p' | head -1)"
  [[ -n "$output_window" ]] || die "could not find Sway X11 output window"
  xdotool windowfocus --sync "$output_window"
  xdotool mousemove --window "$output_window" 600 300
}

trim_geometry() {
  local path="$1"
  convert "$path" -fuzz 8% -trim -format '%wx%h%O' info: 2>/dev/null | tr -d '\n'
}

pixel_difference() {
  local result
  result="$(compare -metric AE "$1" "$2" null: 2>&1 || true)"
  result="${result%%$'\n'*}"
  [[ "$result" =~ ^[0-9]+([.][0-9]+)?([eE][+-]?[0-9]+)?$ ]] || return 1
  printf '%s\n' "$result"
}

baseline_stable() {
  capture baseline >/dev/null || return 1
  capture baseline-next >/dev/null || return 1
  local changed
  changed="$(pixel_difference "$OUT_DIR/baseline.png" "$OUT_DIR/baseline-next.png")" || return 1
  rm -f -- "$OUT_DIR/baseline-next.png"
  awk -v changed="$changed" 'BEGIN { exit !(changed == 0) }'
}

geometry_value() {
  local geometry="$1"
  local field="$2"
  if [[ "$geometry" =~ ^([0-9]+)x([0-9]+)\+(-?[0-9]+)\+(-?[0-9]+)$ ]]; then
    case "$field" in
      width) printf '%s\n' "${BASH_REMATCH[1]}" ;;
      height) printf '%s\n' "${BASH_REMATCH[2]}" ;;
      x) printf '%s\n' "${BASH_REMATCH[3]}" ;;
      y) printf '%s\n' "${BASH_REMATCH[4]}" ;;
      *) return 1 ;;
    esac
  else
    return 1
  fi
}

rendered_capture() {
  local name="$1"
  local path="$OUT_DIR/$name.png"
  capture "$name" >/dev/null || return 1
  local changed
  changed="$(pixel_difference "$OUT_DIR/baseline.png" "$path")" || return 1
  awk -v changed="$changed" 'BEGIN { exit !(changed >= 1000) }' || return 1
  local geometry
  geometry="$(trim_geometry "$path")"
  local width height
  width="$(geometry_value "$geometry" width)" || return 1
  height="$(geometry_value "$geometry" height)" || return 1
  ((width >= 8 && height >= 8))
}

wait_for_rendered_capture() {
  local name="$1"
  local path="$OUT_DIR/$name.png"
  wait_until "${name} widget rendering" rendered_capture "$name"
  printf '%s\n' "$path"
}

send_ipc() {
  local command="$1"
  local response
  response="$(
    python3 - "$SMOKE_SOCKET" "$command" <<'PY'
import socket
import sys

path, command = sys.argv[1:]
with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as client:
    client.settimeout(5)
    client.connect(path)
    client.sendall((command + "\n").encode())
    client.shutdown(socket.SHUT_WR)
    response = client.recv(128).decode().strip()
print(response)
PY
  )"
  [[ "$response" == ok ]] || die "smoke command was rejected: $command ($response)"
}

settings_contains() {
  grep -F "$1" "$XDG_CONFIG_HOME/downshift/settings.toml" >/dev/null
}

log_contains() {
  grep -F "$1" "$APP_LOG" >/dev/null
}

start_app() {
  rm -f -- "$SMOKE_SOCKET"
  APP_LOG="$OUT_DIR/app-${RUN_NUMBER}.log"
  "$BINARY" >"$APP_LOG" 2>&1 &
  APP_PID=$!
  wait_until "smoke control socket" test -S "$SMOKE_SOCKET"
  wait_until "Linux host startup" log_contains "linux host:"
  if [[ "$SCENARIO" != layer-shell ]]; then
    wait_until "visible Downshift window" window_exists
  fi
}

wait_for_app_exit() {
  local attempts=80
  while ((attempts > 0)) && process_running "$APP_PID"; do
    attempts=$((attempts - 1))
    sleep 0.25
  done
  if process_running "$APP_PID"; then
    return 1
  fi
  local status
  if wait "$APP_PID"; then
    status=0
  else
    status=$?
  fi
  APP_PID=""
  log "app exit status=$status"
  ((status == 0))
}

process_running() {
  local pid="$1"
  local state
  kill -0 "$pid" >/dev/null 2>&1 || return 1
  state="$(ps -o stat= -p "$pid" 2>/dev/null | tr -d ' ')"
  [[ -n "$state" && "$state" != Z* ]]
}

stop_process() {
  local pid="$1"
  local signal="$2"
  local attempts=20
  kill -"$signal" "$pid" >/dev/null 2>&1 || true
  while ((attempts > 0)) && process_running "$pid"; do
    attempts=$((attempts - 1))
    sleep 0.25
  done
  if process_running "$pid"; then
    kill -KILL "$pid" >/dev/null 2>&1 || true
  fi
  wait "$pid" >/dev/null 2>&1 || true
}

stop_app() {
  if [[ -z "${APP_PID:-}" ]]; then
    return 0
  fi
  log "requesting clean app exit"
  send_ipc '{"cmd":"quit"}'
  wait_for_app_exit || die "app did not exit cleanly"
}

smoke_dialog() {
  local label="$1"
  local open_command="$2"
  local title="$3"
  local close_command="$4"
  send_ipc "$open_command"
  wait_until "${label} window" window_exists "$title"
  capture "$label"
  send_ipc "$close_command"
  wait_until "${label} window close" window_absent "$title"
}

native_menu_is_rendered() {
  local initial_geometry="$1"
  local menu_geometry menu_width menu_height initial_width initial_height
  capture context-menu >/dev/null || return 1
  menu_geometry="$(trim_geometry "$OUT_DIR/context-menu.png")"
  menu_width="$(geometry_value "$menu_geometry" width)"
  menu_height="$(geometry_value "$menu_geometry" height)"
  initial_width="$(geometry_value "$initial_geometry" width)"
  initial_height="$(geometry_value "$initial_geometry" height)"
  ((menu_width > initial_width || menu_height > initial_height + 100))
}

smoke_common_windows() {
  local initial_capture
  initial_capture="$(wait_for_rendered_capture initial)"
  local initial_geometry
  initial_geometry="$(trim_geometry "$initial_capture")"

  log "opening native context menu"
  if [[ "$SCENARIO" != x11 ]]; then
    prime_wayland_input
  fi
  send_ipc '{"cmd":"show_context_menu","x":48,"y":48}'
  wait_until "native context menu rendering" native_menu_is_rendered "$initial_geometry"
  xdotool key --clearmodifiers Home Return
  wait_until "native pause menu action" settings_contains 'paused = true'
  send_ipc '{"cmd":"set_paused","paused":false}'
  xdotool key --clearmodifiers Escape

  log "resizing widget through IPC"
  local before_window_width
  before_window_width="$(main_window_width)" || die "could not read main window geometry"
  send_ipc '{"cmd":"set_size","size":160.0}'
  wait_until "size setting" settings_contains 'size = 160.0'
  wait_until "main window resize" window_width_increased "$before_window_width"
  wait_for_rendered_capture resized >/dev/null

  smoke_dialog breathing-pattern \
    '{"cmd":"show_breathing_pattern"}' \
    'add breathing pattern' \
    '{"cmd":"close_breathing_pattern"}'
  smoke_dialog custom-snooze \
    '{"cmd":"show_custom_snooze"}' \
    'custom snooze' \
    '{"cmd":"close_custom_snooze"}'
  smoke_dialog telemetry-info \
    '{"cmd":"show_telemetry_info"}' \
    'what we collect' \
    '{"cmd":"close_telemetry_info"}'
}

smoke_x11_drag() {
  local before after before_x after_x window_id
  before="$(x11_geometry)"
  before_x="$(awk '{print $1}' <<<"$before")"
  window_id="$(x11_window_id)"
  local x y width height
  read -r x y width height <<<"$before"
  xdotool mousemove --sync "$((x + width / 2))" "$((y + height / 2))"
  xdotool mousedown 1
  xdotool mousemove --sync "$((x + width / 2 + 120))" "$((y + height / 2 + 40))"
  xdotool mouseup 1
  x11_window_moved() {
    local current current_x
    current="$(x11_geometry)"
    current_x="$(awk '{print $1}' <<<"$current")"
    [[ -n "$current_x" && "$current_x" != "$1" ]]
  }
  wait_until "X11 drag movement" x11_window_moved "$before_x"
  after="$(x11_geometry)"
  after_x="$(awk '{print $1}' <<<"$after")"
  log "X11 drag moved window x=${before_x}->${after_x} (window ${window_id})"
}

layer_output_info() {
  local index="$1"
  swaymsg -t get_outputs | jq -r ".[$index] | [.name, .rect.width, .rect.height, .scale] | @tsv"
}

two_outputs() {
  (($(swaymsg -t get_outputs | jq length) >= 2))
}

smoke_layer_shell() {
  local first second first_name first_width first_height first_scale
  local second_name second_width second_height second_scale
  wait_until "two Sway outputs" two_outputs
  first="$(layer_output_info 0)"
  second="$(layer_output_info 1)"
  read -r first_name first_width first_height first_scale <<<"$first"
  read -r second_name second_width second_height second_scale <<<"$second"
  [[ "$first_name" =~ ^[A-Za-z0-9._-]+$ && "$second_name" =~ ^[A-Za-z0-9._-]+$ ]] || die "unexpected Sway output name"

  write_settings overlay "$second_name" "$second_width" "$second_height" "$second_scale"
  start_app
  wait_until "layer-shell backend" log_contains 'window_backend=wayland_layer_shell'
  wait_until "layer-shell support diagnostic" log_contains 'overlay_supported=true'
  local second_capture second_geometry
  second_capture="$(wait_for_rendered_capture layer-shell-output-2)"
  second_geometry="$(trim_geometry "$second_capture")"
  smoke_dialog layer-shell-telemetry \
    '{"cmd":"show_telemetry_info"}' \
    'what we collect' \
    '{"cmd":"close_telemetry_info"}'
  send_ipc '{"cmd":"set_paused","paused":true}'
  wait_until "layer-shell pause setting" settings_contains 'paused = true'
  local before_capture before_geometry
  before_capture="$(wait_for_rendered_capture layer-shell-resize-before)"
  before_geometry="$(trim_geometry "$before_capture")"
  send_ipc '{"cmd":"set_size","size":160.0}'
  wait_until "layer-shell resize setting" settings_contains 'size = 160.0'
  wait_until "layer-shell resize rendering" \
    layer_rendered_width_increased "$(geometry_value "$before_geometry" width)"
  send_ipc '{"cmd":"set_paused","paused":false}'
  wait_until "layer-shell resume setting" settings_contains 'paused = false'
  stop_app

  write_settings overlay "$first_name" "$first_width" "$first_height" "$first_scale"
  RUN_NUMBER=$((RUN_NUMBER + 1))
  start_app
  wait_until "second layer-shell backend" log_contains 'window_backend=wayland_layer_shell'
  local first_capture first_geometry
  first_capture="$(wait_for_rendered_capture layer-shell-output-1)"
  first_geometry="$(trim_geometry "$first_capture")"
  if (($(geometry_value "$first_geometry" x) >= $(geometry_value "$second_geometry" x) - 500)); then
    die "layer-shell output placement did not move between Sway outputs"
  fi
  log "layer-shell output movement passed: $second_geometry -> $first_geometry"
}

write_result() {
  {
    printf 'scenario=%s\n' "$SCENARIO"
    printf 'binary=%s\n' "$BINARY"
    printf 'status=passed\n'
  } >"$OUT_DIR/result.txt"
}

capture_failure_evidence() {
  local status="$1"
  printf 'exit_status=%s\n' "$status" >"$OUT_DIR/failure.txt"
  if [[ "$SCENARIO" == x11 ]]; then
    xwininfo -root -tree >"$OUT_DIR/x11-tree.txt" 2>&1 || true
    xwd -root -silent 2>/dev/null | convert xwd:- "png:$OUT_DIR/failure.png" >/dev/null 2>&1 || true
  else
    swaymsg -t get_tree >"$OUT_DIR/wayland-tree.json" 2>&1 || true
    grimshot save screen "$OUT_DIR/failure.png" >/dev/null 2>&1 || true
  fi
  cp "$XDG_CONFIG_HOME/downshift/settings.toml" "$OUT_DIR/settings.toml" 2>/dev/null || true
  log "saved failure evidence to $OUT_DIR"
}

cleanup() {
  local status="$?"
  trap - EXIT
  if ((status != 0)); then
    capture_failure_evidence "$status"
  fi
  if [[ -n "${APP_PID:-}" ]] && process_running "$APP_PID"; then
    stop_process "$APP_PID" INT
  fi
  if [[ -n "${COMPOSITOR_PID:-}" ]] && process_running "$COMPOSITOR_PID"; then
    stop_process "$COMPOSITOR_PID" TERM
  fi
  if [[ -n "${PARENT_COMPOSITOR_PID:-}" ]] && process_running "$PARENT_COMPOSITOR_PID"; then
    stop_process "$PARENT_COMPOSITOR_PID" TERM
  fi
  if [[ -n "${XSERVER_PID:-}" ]] && process_running "$XSERVER_PID"; then
    stop_process "$XSERVER_PID" TERM
  fi
  rm -rf -- "$SMOKE_ROOT"
  exit "$status"
}

if (($# < 1 || $# > 3)); then
  usage >&2
  exit 2
fi

SCENARIO="$1"
case "$SCENARIO" in
  x11 | wayland | layer-shell | missing-layer-shell) ;;
  *)
    usage >&2
    exit 2
    ;;
esac

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repo_root="$(cd -- "$script_dir/../.." && pwd -P)"
BINARY="${2:-$repo_root/target/release/downshift}"
if [[ "$BINARY" != /* ]]; then
  BINARY="$repo_root/$BINARY"
fi
[[ -x "$BINARY" ]] || die "binary is not executable: $BINARY"
OUT_DIR="${3:-$repo_root/logs/linux-gui-smoke-$SCENARIO-$(date +%Y%m%d-%H%M%S)}"
mkdir -p "$OUT_DIR"
SMOKE_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/downshift-linux-gui.XXXXXX")"
XDG_RUNTIME_DIR="$SMOKE_ROOT/runtime"
XDG_CONFIG_HOME="$SMOKE_ROOT/config"
XDG_DATA_HOME="$SMOKE_ROOT/data"
XDG_CACHE_HOME="$SMOKE_ROOT/cache"
XDG_STATE_HOME="$SMOKE_ROOT/state"
SMOKE_SOCKET="$SMOKE_ROOT/control.sock"
RUN_NUMBER=1
APP_PID=""
COMPOSITOR_PID=""
XSERVER_PID=""
PARENT_COMPOSITOR_PID=""
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$XDG_CACHE_HOME" "$XDG_STATE_HOME"
chmod 700 "$XDG_RUNTIME_DIR"
export HOME="$SMOKE_ROOT/home"
mkdir -p "$HOME"
export XDG_RUNTIME_DIR XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_STATE_HOME
export DOWNSHIFT_SMOKE_SOCKET="$SMOKE_SOCKET"
export DOWNSHIFT_LOG_DIR="$OUT_DIR/runtime-logs"
export DOWNSHIFT_ENV=smoke
# Keep headless WebKit/GTK rendering on software paths in CI and containers.
export LIBGL_ALWAYS_SOFTWARE=1
export WEBKIT_DISABLE_COMPOSITING_MODE=1
export WEBKIT_DISABLE_DMABUF_RENDERER=1
if [[ "$SCENARIO" == missing-layer-shell ]]; then
  export DOWNSHIFT_LINUX_DISABLE_LAYER_SHELL=1
else
  unset DOWNSHIFT_LINUX_DISABLE_LAYER_SHELL
fi

trap cleanup EXIT
exec > >(tee "$OUT_DIR/run.log") 2>&1
require_tools
start_display
wait_until "stable compositor baseline" baseline_stable

case "$SCENARIO" in
  x11)
    write_settings auto
    start_app
    wait_until "X11 backend" log_contains 'window_backend=x11_ewmh'
    smoke_common_windows
    smoke_x11_drag
    ;;
  wayland)
    write_settings normal_window
    start_app
    wait_until "generic Wayland fallback" log_contains 'window_backend=wayland_normal'
    smoke_common_windows
    if grep -F 'physical_x =' "$XDG_CONFIG_HOME/downshift/settings.toml" >/dev/null 2>&1 ||
      grep -F 'physical_y =' "$XDG_CONFIG_HOME/downshift/settings.toml" >/dev/null 2>&1; then
      die "generic Wayland smoke persisted global coordinates"
    fi
    ;;
  layer-shell)
    smoke_layer_shell
    ;;
  missing-layer-shell)
    write_settings overlay
    start_app
    wait_until "missing layer-shell fallback" log_contains 'window_backend=wayland_normal'
    wait_until "missing layer-shell diagnostic" log_contains 'fallback_reason=gtk-layer-shell is unavailable'
    smoke_common_windows
    ;;
esac

stop_app
write_result
log "Linux $SCENARIO GUI smoke passed"
