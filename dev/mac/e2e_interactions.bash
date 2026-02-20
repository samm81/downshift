#!/usr/bin/env bash
set -euo pipefail

# experimental:
# this script is a best-effort ui automation experiment for macos and may fail
# in some sessions even when permissions appear correctly granted.

log() {
  printf '[e2e-gui] %s\n' "$*"
}

die() {
  printf '[e2e-gui] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

skip() {
  printf '[e2e-gui] skip: %s\n' "$*"
  exit 0
}

require_tools() {
  local missing=()
  for tool in cargo osascript screencapture shasum cliclick swift; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
}

verify_accessibility_permissions() {
  local probe
  probe="$(
    osascript <<'APPLESCRIPT'
try
  tell application "System Events"
    return count of processes
  end tell
on error err_msg
  return "ERROR:" & err_msg
end try
APPLESCRIPT
  )"
  if [[ "$probe" == ERROR:* ]]; then
    die "accessibility/automation is not granted for this shell (${probe}). run ./dev/mac/bootstrap_gui_permissions.bash from a desktop terminal session, then verify terminal is enabled in settings > privacy & security > accessibility and settings > privacy & security > automation (terminal -> system events)"
  fi
}

load_rust_env_if_present() {
  if have cargo; then
    return
  fi

  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi
}

app_settings_path() {
  printf '%s/Library/Application Support/breath-ball/settings.toml' "$HOME"
}

read_setting_value() {
  local path="$1"
  local key="$2"
  [[ -f "$path" ]] || return 1
  awk -F' = ' -v key="$key" '$1 == key { gsub(/"/, "", $2); print $2; exit }' "$path"
}

get_window_frame() {
  osascript <<'APPLESCRIPT'
try
  tell application "System Events"
    if not (exists process "breath-ball") then error "process not found"
    tell process "breath-ball"
      if (count of windows) is 0 then error "window not found"
      set p to position of window 1
      set s to size of window 1
      return (item 1 of p as integer as text) & "," & (item 2 of p as integer as text) & "," & (item 1 of s as integer as text) & "," & (item 2 of s as integer as text)
    end tell
  end tell
on error err_msg
  return "ERROR:" & err_msg
end try
APPLESCRIPT
}

wait_for_window() {
  local tries=0
  local last_error=""
  while ((tries < 80)); do
    local frame
    frame="$(get_window_frame)"
    if [[ "$frame" != ERROR:* ]]; then
      printf '%s\n' "$frame"
      return 0
    fi
    last_error="$frame"
    sleep 0.25
    tries=$((tries + 1))
  done
  if [[ -n "$last_error" ]]; then
    printf '%s\n' "$last_error"
  fi
  return 1
}

abs() {
  local value="$1"
  if ((value < 0)); then
    echo $((-value))
  else
    echo "$value"
  fi
}

parse_frame() {
  local frame="$1"
  IFS=',' read -r FRAME_X FRAME_Y FRAME_W FRAME_H <<<"$frame"
}

capture() {
  local file="$1"
  screencapture -x "$file"
}

emit_scroll() {
  local amount="$1"
  swift -e "import CoreGraphics\nif let e = CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: ${amount}, wheel2: 0, wheel3: 0) { e.post(tap: .cghidEventTap) }"
}

click_pause_or_resume() {
  local cx="$1"
  local cy="$2"
  local click_x="$3"
  local click_y="$4"
  cliclick "rc:${cx},${cy}" "w:180" "c:${click_x},${click_y}"
}

cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${SETTINGS_PATH:-}" ]]; then
    if [[ -n "${SETTINGS_BACKUP_PATH:-}" && -f "${SETTINGS_BACKUP_PATH}" ]]; then
      cp "${SETTINGS_BACKUP_PATH}" "${SETTINGS_PATH}" >/dev/null 2>&1 || true
    elif [[ "${SETTINGS_CREATED_BY_TEST:-0}" == "1" ]]; then
      rm -f "${SETTINGS_PATH}" >/dev/null 2>&1 || true
    fi
  fi
}

main() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    skip "macos gui e2e is only available on darwin hosts"
  fi

  load_rust_env_if_present
  require_tools
  verify_accessibility_permissions

  local script_dir repo_root
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "${script_dir}/../.." && pwd)"
  cd "$repo_root"

  local stamp out_dir run_log settings_path
  stamp="$(date +%Y%m%d-%H%M%S)"
  out_dir="logs/gui-e2e-$stamp"
  run_log="$out_dir/run.log"
  settings_path="$(app_settings_path)"
  SETTINGS_PATH="$settings_path"
  SETTINGS_BACKUP_PATH=""
  SETTINGS_CREATED_BY_TEST=0

  mkdir -p "$out_dir"
  mkdir -p "$(dirname "$settings_path")"
  if [[ -f "$settings_path" ]]; then
    SETTINGS_BACKUP_PATH="$out_dir/settings.backup.toml"
    cp "$settings_path" "$SETTINGS_BACKUP_PATH"
  else
    SETTINGS_CREATED_BY_TEST=1
  fi

  log "starting app with cargo run --quiet"
  cargo run --quiet >"$run_log" 2>&1 &
  APP_PID=$!
  trap cleanup EXIT

  local frame frame_initial
  frame="$(wait_for_window)" || die "app window did not become available (${frame:-unknown reason})"
  frame_initial="$frame"
  parse_frame "$frame"

  local center_x center_y
  center_x=$((FRAME_X + (FRAME_W / 2)))
  center_y=$((FRAME_Y + (FRAME_H / 2)))

  capture "$out_dir/shot-1-initial.png"

  # drag should move the widget
  local drag_target_x drag_target_y
  drag_target_x=$((center_x + 90))
  drag_target_y=$((center_y + 55))
  cliclick "dd:${center_x},${center_y}" "dm:${drag_target_x},${drag_target_y}" "du:${drag_target_x},${drag_target_y}"
  sleep 0.35

  local frame_after_drag drag_x drag_y drag_w drag_h
  frame_after_drag="$(get_window_frame)"
  [[ "$frame_after_drag" != ERROR:* ]] || die "failed to read window frame after held drag"
  IFS=',' read -r drag_x drag_y drag_w drag_h <<<"$frame_after_drag"

  local drag_dx drag_dy
  drag_dx="$(abs $((drag_x - FRAME_X)))"
  drag_dy="$(abs $((drag_y - FRAME_Y)))"
  if ((drag_dx < 20 && drag_dy < 20)); then
    die "held drag check failed: window did not move enough (dx=${drag_dx}, dy=${drag_dy})"
  fi

  FRAME_X="$drag_x"
  FRAME_Y="$drag_y"
  FRAME_W="$drag_w"
  FRAME_H="$drag_h"
  center_x=$((FRAME_X + (FRAME_W / 2)))
  center_y=$((FRAME_Y + (FRAME_H / 2)))

  capture "$out_dir/shot-2-after-drag.png"

  # wheel should resize the actual window
  cliclick "m:${center_x},${center_y}"
  local before_scroll_size scroll_amount used_scroll_amount
  before_scroll_size="$FRAME_W"
  used_scroll_amount=0
  for scroll_amount in -6 6; do
    emit_scroll "$scroll_amount"
    sleep 0.25
    frame="$(get_window_frame)"
    [[ "$frame" != ERROR:* ]] || die "failed to read window frame after scroll"
    parse_frame "$frame"
    if ((FRAME_W != before_scroll_size)); then
      used_scroll_amount="$scroll_amount"
      break
    fi
  done
  if ((used_scroll_amount == 0)); then
    die "scroll resize check failed: wheel event did not change window size"
  fi

  local coarse_delta
  coarse_delta="$(abs $((FRAME_W - before_scroll_size)))"
  if ((coarse_delta < 2)); then
    die "scroll resize check failed: size change too small (${coarse_delta}px)"
  fi

  # shift+wheel should apply finer resizing
  center_x=$((FRAME_X + (FRAME_W / 2)))
  center_y=$((FRAME_Y + (FRAME_H / 2)))
  local size_before_fine size_after_fine fine_delta
  size_before_fine="$FRAME_W"
  cliclick "m:${center_x},${center_y}" "kd:shift"
  emit_scroll "$used_scroll_amount"
  cliclick "ku:shift"
  sleep 0.25

  frame="$(get_window_frame)"
  [[ "$frame" != ERROR:* ]] || die "failed to read window frame after fine scroll"
  parse_frame "$frame"
  size_after_fine="$FRAME_W"
  fine_delta="$(abs $((size_after_fine - size_before_fine)))"
  if ((fine_delta < 1 || fine_delta > 3)); then
    die "fine scroll check failed: expected 1-3px change, got ${fine_delta}px"
  fi

  capture "$out_dir/shot-3-after-scroll.png"

  # right click pause and resume via the in-app menu
  center_x=$((FRAME_X + (FRAME_W / 2)))
  center_y=$((FRAME_Y + (FRAME_H / 2)))
  local menu_click_x menu_click_y paused_value
  menu_click_x=$((FRAME_X + (FRAME_W / 2) + 20))
  menu_click_y=$((FRAME_Y + (FRAME_H / 2) + 14))

  click_pause_or_resume "$center_x" "$center_y" "$menu_click_x" "$menu_click_y"
  sleep 0.35
  paused_value="$(read_setting_value "$settings_path" "paused" || true)"
  [[ "$paused_value" == "true" ]] || die "menu pause check failed: expected paused=true, got '${paused_value:-missing}'"

  click_pause_or_resume "$center_x" "$center_y" "$menu_click_x" "$menu_click_y"
  sleep 0.35
  paused_value="$(read_setting_value "$settings_path" "paused" || true)"
  [[ "$paused_value" == "false" ]] || die "menu resume check failed: expected paused=false, got '${paused_value:-missing}'"

  capture "$out_dir/shot-4-after-menu.png"

  cat >"$out_dir/result.txt" <<RESULT
window_initial=${frame_initial}
window_after_drag=${frame_after_drag}
coarse_scroll_delta_px=${coarse_delta}
fine_scroll_delta_px=${fine_delta}
settings_path=${settings_path}
run_log=${run_log}
RESULT

  log "result written to $out_dir/result.txt"
  log "all gui interaction checks passed"
}

main "$@"
