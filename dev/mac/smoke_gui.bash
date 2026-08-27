#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Downshift"
APP_BUNDLE_NAME="${APP_NAME}.app"
REPO_SLUG="${DOWNSHIFT_RELEASE_REPO:-samm81/downshift}"
SMOKE_OUT_DIR=""

log() {
  printf '[smoke-gui] %s\n' "$*"
}

die() {
  printf '[smoke-gui] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

require_macos() {
  [[ "$(uname -s)" == "Darwin" ]] || die "this script is for macos only"
}

ensure_tools() {
  local missing=()
  local tools=(compare magick open pgrep screencapture swift)
  if [[ -z "${DOWNSHIFT_APP_PATH:-}" ]]; then
    tools+=(hdiutil)
  fi
  if [[ -z "${DOWNSHIFT_RELEASE_DMG_PATH:-}" && -z "${DOWNSHIFT_APP_PATH:-}" ]]; then
    tools+=(gh)
  fi
  for tool in "${tools[@]}"; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
}

cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi

  pkill -x downshift >/dev/null 2>&1 || true
  pkill -x "$APP_NAME" >/dev/null 2>&1 || true

  if [[ -n "${MOUNT_POINT:-}" ]]; then
    hdiutil detach "$MOUNT_POINT" >/dev/null 2>&1 || true
  fi
}

capture_failure_evidence() {
  local status="$1"
  if [[ -z "$SMOKE_OUT_DIR" ]]; then
    return 0
  fi

  printf 'exit_status=%s\n' "$status" >"$SMOKE_OUT_DIR/failure.txt" || true
  if have screencapture; then
    screencapture -x "$SMOKE_OUT_DIR/failure.png" >/dev/null 2>&1 || true
  fi
  log "saved failure evidence to $SMOKE_OUT_DIR"
}

on_exit() {
  local status="$?"
  trap - EXIT
  if ((status != 0)); then
    capture_failure_evidence "$status"
  fi
  cleanup
  exit "$status"
}
require_github_auth() {
  gh auth status >/dev/null 2>&1 || die "gh is not authenticated"
}

resolve_release_tag() {
  if [[ -n "${DOWNSHIFT_RELEASE_TAG:-}" ]]; then
    printf '%s\n' "$DOWNSHIFT_RELEASE_TAG"
    return 0
  fi
  gh release view --repo "$REPO_SLUG" --json tagName --jq '.tagName'
}

download_release_dmg() {
  local tag="$1"
  local release_dir="$2"

  mkdir -p "$release_dir"
  gh release download "$tag" \
    --repo "$REPO_SLUG" \
    --pattern "${APP_NAME}-notarized-*.dmg" \
    --dir "$release_dir"

  DMG_PATH="$(find "$release_dir" -maxdepth 1 -type f -name "${APP_NAME}-notarized-*.dmg" | head -n 1)"
  [[ -n "$DMG_PATH" ]] || die "failed to download notarized dmg for ${tag}"
}

mount_release_dmg() {
  local attach_log
  attach_log="$(hdiutil attach "$DMG_PATH" -nobrowse)"
  MOUNT_POINT="$(printf '%s\n' "$attach_log" | awk '/\/Volumes\// { print $NF; exit }')"
  [[ -n "$MOUNT_POINT" ]] || die "failed to determine mounted dmg path"

  APP_PATH="${MOUNT_POINT}/${APP_BUNDLE_NAME}"
  [[ -d "$APP_PATH" ]] || die "mounted dmg does not contain ${APP_BUNDLE_NAME}"
}

prepare_app_bundle() {
  if [[ -n "${DOWNSHIFT_APP_PATH:-}" ]]; then
    APP_PATH="$(cd "${DOWNSHIFT_APP_PATH}" && pwd)"
    [[ -d "$APP_PATH" ]] || die "provided app path does not exist: ${APP_PATH}"
    DMG_PATH=""
    MOUNT_POINT=""
    RELEASE_TAG="${DOWNSHIFT_RELEASE_TAG:-local}"
    log "using provided app bundle: ${APP_PATH}"
    return 0
  fi

  mount_release_dmg
  log "mounted dmg at ${MOUNT_POINT}"
}

launch_app_bundle() {
  log "launching ${APP_PATH}"
  open -a "$APP_PATH"
}

activate_running_instance() {
  local executable="${APP_PATH}/Contents/MacOS/downshift"
  [[ -x "$executable" ]] || die "app bundle executable is not runnable: ${executable}"
  log "launching a second instance through ${executable}"
  if ! "$executable" >/dev/null 2>&1; then
    die "second app launch failed"
  fi
}

find_app_pid() {
  pgrep -x downshift | head -n 1 || pgrep -x "$APP_NAME" | head -n 1 || true
}

wait_for_app_process() {
  local tries=0
  while ((tries < 40)); do
    APP_PID="$(find_app_pid)"
    if [[ -n "$APP_PID" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.5
  done
  return 1
}

count_visible_windows() {
  local output
  output="$(
    swift -e '
import CoreGraphics

let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
let names = Set(["Downshift", "downshift"])
let count = windows.filter { info in
  guard let owner = info[kCGWindowOwnerName as String] as? String else {
    return false
  }
  return names.contains(owner)
}.count
print(count)
' 2>/dev/null
  )" || return 1

  printf '%s\n' "$output" | tr -d '[:space:]'
}

wait_for_window() {
  local tries=0
  while ((tries < 40)); do
    WINDOW_COUNT="$(count_visible_windows || true)"
    if [[ "${WINDOW_COUNT:-0}" =~ ^[0-9]+$ ]] && ((WINDOW_COUNT > 0)); then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.5
  done
  return 1
}

smoke_snooze_input() {
  swift "$repo_root/dev/mac/smoke_snooze_input.swift" "$@"
}

smoke_follow_input() {
  swift "$repo_root/dev/mac/smoke_input.swift" "$@"
}

smoke_input() {
  case "$1" in
    move|tray-rect|tray-click|left-click)
      smoke_follow_input "$@"
      ;;
    *)
      smoke_snooze_input "$@"
      ;;
  esac
}

wait_for_native_menu_item() {
  local title="$1"
  if smoke_input menu-visible "$title" "$APP_PID" >/dev/null 2>&1; then
    log "native context menu contains '$title'"
    return 0
  fi
  return 1
}

read_window_bounds() {
  local bounds
  bounds="$(smoke_input window-bounds "$APP_PID")" || return 1
  read -r WINDOW_X WINDOW_Y WINDOW_WIDTH WINDOW_HEIGHT <<<"$bounds"
  [[ "$WINDOW_X" =~ ^-?[0-9]+$ && "$WINDOW_Y" =~ ^-?[0-9]+$ && "$WINDOW_WIDTH" =~ ^[0-9]+$ && "$WINDOW_HEIGHT" =~ ^[0-9]+$ ]]
}

wait_for_window_bounds() {
  local tries=0
  while ((tries < 40)); do
    if read_window_bounds && ((WINDOW_WIDTH >= 50 && WINDOW_HEIGHT >= 50)); then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.25
  done
  return 1
}

read_tray_rect() {
  local bounds
  bounds="$(smoke_follow_input tray-rect)" || return 1
  read -r TRAY_X TRAY_Y TRAY_WIDTH TRAY_HEIGHT <<<"$bounds"
  [[ "$TRAY_X" =~ ^-?[0-9]+$ && "$TRAY_Y" =~ ^-?[0-9]+$ && "$TRAY_WIDTH" =~ ^[0-9]+$ && "$TRAY_HEIGHT" =~ ^[0-9]+$ ]]
}

wait_for_tray_rect() {
  local tries=0
  while ((tries < 40)); do
    if read_tray_rect && ((TRAY_WIDTH > 0 && TRAY_HEIGHT > 0)); then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.25
  done
  return 1
}

wait_for_window_near_cursor() {
  local cursor_x="$1"
  local cursor_y="$2"
  local tolerance="${3:-80}"
  local tries=0
  while ((tries < 40)); do
    if read_window_bounds && awk \
      -v window_x="$WINDOW_X" \
      -v window_y="$WINDOW_Y" \
      -v window_width="$WINDOW_WIDTH" \
      -v window_height="$WINDOW_HEIGHT" \
      -v cursor_x="$cursor_x" \
      -v cursor_y="$cursor_y" \
      -v tolerance="$tolerance" \
      'BEGIN {
        dx = window_x + window_width / 2 - cursor_x
        dy = window_y + window_height / 2 - cursor_y
        if (dx < 0) dx = -dx
        if (dy < 0) dy = -dy
        exit !(dx <= tolerance && dy <= tolerance)
      }'; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.1
  done
  return 1
}

wait_for_no_window() {
  local tries=0
  while ((tries < 40)); do
    if ! read_window_bounds 2>/dev/null; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.25
  done
  return 1
}

main() {
  require_macos
  ensure_tools

  local script_dir repo_root
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "${script_dir}/../.." && pwd)"
  cd "$repo_root"

  local screenshot_count="${1:-3}"
  local screenshot_interval="${2:-1}"
  local stamp
  stamp="$(date +%Y%m%d-%H%M%S)"
  local out_dir="logs/gui-smoke-$stamp"
  local run_log="$out_dir/run.log"
  local latest_link="logs/latest-gui-smoke"
  local release_dir="$out_dir/release"
  local crop_dir="$out_dir/crops"
  local diff_dir="$out_dir/diffs"
  local crop_top_pixels=32

  if ! [[ "$screenshot_count" =~ ^[0-9]+$ ]] || ((screenshot_count < 2)); then
    die "first arg must be screenshot count (integer >= 2)"
  fi
  if ! [[ "$screenshot_interval" =~ ^([0-9]+|[0-9]*\.[0-9]+)$ ]]; then
    die "second arg must be interval seconds (for example: 1 or 0.5)"
  fi

  mkdir -p "$out_dir" "$crop_dir" "$diff_dir"
  SMOKE_OUT_DIR="$out_dir"
  exec > >(tee "$run_log") 2>&1
  trap on_exit EXIT

  if [[ -n "${DOWNSHIFT_APP_PATH:-}" ]]; then
    prepare_app_bundle
  elif [[ -n "${DOWNSHIFT_RELEASE_DMG_PATH:-}" ]]; then
    DMG_PATH="${DOWNSHIFT_RELEASE_DMG_PATH}"
    [[ -f "$DMG_PATH" ]] || die "provided dmg path does not exist: ${DMG_PATH}"
    RELEASE_TAG="${DOWNSHIFT_RELEASE_TAG:-}"
    log "using provided dmg: ${DMG_PATH}"
  else
    require_github_auth
    RELEASE_TAG="$(resolve_release_tag)"
    [[ -n "$RELEASE_TAG" ]] || die "failed to resolve latest release tag"
    log "resolved release tag: ${RELEASE_TAG}"

    download_release_dmg "$RELEASE_TAG" "$release_dir"
    log "downloaded dmg: ${DMG_PATH}"
  fi

  if [[ -z "${DOWNSHIFT_APP_PATH:-}" ]]; then
    prepare_app_bundle
  fi

  launch_app_bundle
  wait_for_app_process || die "app process did not appear after launch"
  log "app process detected (pid ${APP_PID})"

  wait_for_window || die "no visible ${APP_NAME} window detected after launch"
  log "visible window count: ${WINDOW_COUNT}"

  local warmup_capture="$out_dir/warmup-popup-trigger.png"
  local settle_delay_seconds="10"
  CAPTURE_MODE="fullscreen"
  log "triggering macos capture prompt with warmup screenshot"
  screencapture -x "$warmup_capture"
  log "saved $warmup_capture"
  local allow_attempt=1
  while ((allow_attempt <= 10)); do
    if smoke_input allow-screen-capture 2>/dev/null; then
      log "dismissed screen-capture permission prompt"
      break
    fi
    sleep 0.5
    allow_attempt=$((allow_attempt + 1))
  done
  log "waiting ${settle_delay_seconds}s for animation and prompts to settle"
  sleep "$settle_delay_seconds"
  log "capturing full-screen screenshots"

  local i=1
  while ((i <= screenshot_count)); do
    local file="$out_dir/shot-$i.png"
    screencapture -x "$file"
    log "saved $file"
    magick "$file" -gravity North -chop "0x${crop_top_pixels}" "$crop_dir/shot-$i.png"
    log "saved $crop_dir/shot-$i.png"
    if ((i < screenshot_count)); then
      sleep "$screenshot_interval"
    fi
    i=$((i + 1))
  done

  wait_for_window_bounds || die "could not read the visible Downshift window bounds"
  local fixed_window_x="$WINDOW_X"
  local fixed_window_y="$WINDOW_Y"
  local fixed_window_width="$WINDOW_WIDTH"
  local fixed_window_height="$WINDOW_HEIGHT"
  local context_click_x=$((fixed_window_x + fixed_window_width / 2))
  local context_click_y=$((fixed_window_y + fixed_window_height - 6))

  log "pausing and resetting the widget through its native context menu"
  smoke_input right-click "$context_click_x" "$context_click_y"
  wait_for_native_menu_item "pause" || die "native pause menu did not open"
  screencapture -x "$out_dir/reset-context-menu.png"
  smoke_input menu-click "pause" "$APP_PID"

  smoke_input right-click "$context_click_x" "$context_click_y"
  wait_for_native_menu_item "paused" || die "pause did not update the native menu state"
  smoke_input key escape
  screencapture -x "$out_dir/reset-paused.png"

  smoke_input right-click "$context_click_x" "$context_click_y"
  wait_for_native_menu_item "reset" || die "native reset menu did not open"
  smoke_input menu-click "reset" "$APP_PID"
  wait_for_window_bounds || die "reset did not leave the widget visible"
  screencapture -x "$out_dir/reset.png"
  log "native reset restored the visible active widget"
  read_window_bounds || die "could not read the widget bounds after reset"
  local snooze_click_x=$((WINDOW_X + WINDOW_WIDTH / 2))
  local snooze_click_y=$((WINDOW_Y + WINDOW_HEIGHT - 6))

  log "snoozing the widget for five minutes through its native context menu"
  smoke_input right-click "$snooze_click_x" "$snooze_click_y"
  wait_for_native_menu_item "snooze" || die "native snooze menu did not open"
  screencapture -x "$out_dir/snooze-context-menu.png"
  smoke_input key home
  smoke_input key down
  smoke_input key right
  wait_for_native_menu_item "snooze for 5 minutes" || die "native snooze submenu did not open"
  smoke_input key return
  wait_for_no_window || die "snooze did not hide the widget"
  screencapture -x "$out_dir/snoozed.png"
  log "snooze hid the widget"

  activate_running_instance
  wait_for_window_bounds || die "second launch did not resume the snoozed widget"
  screencapture -x "$out_dir/snooze-resumed.png"
  log "second launch resumed the snoozed widget"
  local fixed_center_x=$((fixed_window_x + fixed_window_width / 2))
  local fixed_center_y=$((fixed_window_y + fixed_window_height / 2))
  local follow_cursor_x1=$((fixed_center_x - 260))
  local follow_cursor_y1=$((fixed_center_y + 160))
  local follow_cursor_x2=$((fixed_center_x - 420))
  local follow_cursor_y2=$((fixed_center_y + 280))

  wait_for_tray_rect || die "could not locate the Downshift status item"
  local tray_center_x=$((TRAY_X + TRAY_WIDTH / 2))
  local tray_center_y=$((TRAY_Y + TRAY_HEIGHT / 2))
  log "right-clicking the status item at ${tray_center_x},${tray_center_y}"
  smoke_input right-click "$tray_center_x" "$tray_center_y"
  sleep 0.5
  screencapture -x "$out_dir/tray-menu-right-click.png"
  smoke_input key escape

  log "left-clicking the status item at ${tray_center_x},${tray_center_y}"
  smoke_input left-click "$tray_center_x" "$tray_center_y"
  sleep 0.5
  screencapture -x "$out_dir/tray-menu-left-click.png"
  smoke_input key escape

  log "opening the status-item menu through Accessibility to enable follow-cursor mode"
  smoke_input tray-click
  sleep 0.5
  screencapture -x "$out_dir/follow-tray-menu.png"
  smoke_input menu-click "follow cursor" "$APP_PID"
  sleep 0.8

  smoke_input move "$follow_cursor_x1" "$follow_cursor_y1"
  wait_for_window_near_cursor "$follow_cursor_x1" "$follow_cursor_y1" 80 || die "follow-cursor mode did not move the widget to the first pointer position"
  local follow_window_x1="$WINDOW_X"
  local follow_window_y1="$WINDOW_Y"
  screencapture -x "$out_dir/follow-cursor.png"
  smoke_input move "$follow_cursor_x2" "$follow_cursor_y2"
  wait_for_window_near_cursor "$follow_cursor_x2" "$follow_cursor_y2" 80 || die "follow-cursor mode did not move the widget to the second pointer position"
  if [[ "$follow_window_x1" == "$WINDOW_X" && "$follow_window_y1" == "$WINDOW_Y" ]]; then
    die "follow-cursor mode did not move the widget after the pointer moved"
  fi
  log "follow-cursor movement passed: ${follow_window_x1},${follow_window_y1} -> ${WINDOW_X},${WINDOW_Y}"

  log "right-clicking the status item while follow-cursor mode is active"
  smoke_input right-click "$tray_center_x" "$tray_center_y"
  sleep 0.5
  screencapture -x "$out_dir/follow-tray-context-menu.png"
  smoke_input key escape

  log "opening the status-item menu through Accessibility to disable follow-cursor mode"
  smoke_input tray-click
  sleep 0.5
  screencapture -x "$out_dir/follow-tray-disable-menu.png"
  smoke_input menu-click "follow cursor" "$APP_PID"
  sleep 0.8
  read_window_bounds || die "could not read the Downshift window after disabling follow-cursor mode"
  if ! awk \
    -v window_x="$WINDOW_X" \
    -v window_y="$WINDOW_Y" \
    -v fixed_window_x="$fixed_window_x" \
    -v fixed_window_y="$fixed_window_y" \
    'BEGIN {
      dx = window_x - fixed_window_x
      dy = window_y - fixed_window_y
      if (dx < 0) dx = -dx
      if (dy < 0) dy = -dy
      exit !(dx <= 20 && dy <= 20)
    }'; then
    die "status-item menu did not return the widget to fixed mode"
  fi
  log "status-item menu and follow-cursor disable passed"

  local diff_nonzero_pairs=0
  local diff_total_pairs=0
  local max_diff_pixels=0
  i=1
  while ((i < screenshot_count)); do
    local next=$((i + 1))
    local metric_file="$diff_dir/shot-${i}-to-${next}.txt"
    local diff_image="$diff_dir/shot-${i}-to-${next}.png"
    local diff_pixels
    diff_pixels="$(
      compare -metric AE \
        "$crop_dir/shot-$i.png" \
        "$crop_dir/shot-$next.png" \
        "$diff_image" \
        2>"$metric_file"
    )" || true
    diff_pixels="$(awk '{ print $1 }' "$metric_file")"
    # ImageMagick may format large pixel counts using scientific notation.
    if ! [[ "$diff_pixels" =~ ^[0-9]+([.][0-9]+)?([eE][+-]?[0-9]+)?$ ]]; then
      die "failed to read image diff metric from $metric_file"
    fi
    if awk "BEGIN { exit !($diff_pixels > 0) }"; then
      diff_nonzero_pairs=$((diff_nonzero_pairs + 1))
    fi
    if awk "BEGIN { exit !($diff_pixels > $max_diff_pixels) }"; then
      max_diff_pixels="$diff_pixels"
    fi
    diff_total_pairs=$((diff_total_pairs + 1))
    i=$((i + 1))
  done

  local motion_observed="no"
  if ((diff_nonzero_pairs > 0)); then
    motion_observed="yes"
  fi

  cat >"$out_dir/result.txt" <<EOF
release_repo=$REPO_SLUG
release_tag=$RELEASE_TAG
dmg_path=$DMG_PATH
mount_point=$MOUNT_POINT
app_path=$APP_PATH
app_pid=$APP_PID
window_count=$WINDOW_COUNT
capture_mode=$CAPTURE_MODE
warmup_capture=$warmup_capture
settle_delay_seconds=$settle_delay_seconds
screenshot_count=$screenshot_count
screenshot_interval_seconds=$screenshot_interval
crop_top_pixels=$crop_top_pixels
diff_total_pairs=$diff_total_pairs
diff_nonzero_pairs=$diff_nonzero_pairs
max_diff_pixels=$max_diff_pixels
motion_observed=$motion_observed
reset=passed
snooze_resume=passed
follow_cursor_movement=passed
tray_menu_right_click=passed
tray_menu_disable_follow=passed
run_log=$run_log
EOF

  ln -sfn "$(basename "$out_dir")" "$latest_link"
  log "result written to $out_dir/result.txt"
}

main "$@"
