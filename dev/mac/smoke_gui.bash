#!/usr/bin/env bash
set -euo pipefail

APP_NAME="Downshift"
APP_BUNDLE_NAME="${APP_NAME}.app"
REPO_SLUG="${DOWNSHIFT_RELEASE_REPO:-samm81/downshift}"

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
  for tool in compare gh hdiutil open pgrep screencapture swift; do
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

require_github_auth() {
  gh auth status >/dev/null 2>&1 || die "gh is not authenticated"
}

resolve_release_tag() {
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

launch_app_bundle() {
  log "launching ${APP_PATH}"
  open -a "$APP_PATH"
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

main() {
  require_macos
  ensure_tools
  require_github_auth

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
  local diff_dir="$out_dir/diffs"

  if ! [[ "$screenshot_count" =~ ^[0-9]+$ ]] || ((screenshot_count < 2)); then
    die "first arg must be screenshot count (integer >= 2)"
  fi
  if ! [[ "$screenshot_interval" =~ ^([0-9]+|[0-9]*\.[0-9]+)$ ]]; then
    die "second arg must be interval seconds (for example: 1 or 0.5)"
  fi

  mkdir -p "$out_dir" "$diff_dir"
  exec > >(tee "$run_log") 2>&1
  trap cleanup EXIT

  RELEASE_TAG="$(resolve_release_tag)"
  [[ -n "$RELEASE_TAG" ]] || die "failed to resolve latest release tag"
  log "resolved latest release tag: ${RELEASE_TAG}"

  download_release_dmg "$RELEASE_TAG" "$release_dir"
  log "downloaded dmg: ${DMG_PATH}"

  mount_release_dmg
  log "mounted dmg at ${MOUNT_POINT}"

  launch_app_bundle
  wait_for_app_process || die "app process did not appear after launch"
  log "app process detected (pid ${APP_PID})"

  wait_for_window || die "no visible ${APP_NAME} window detected after launch"
  log "visible window count: ${WINDOW_COUNT}"

  local warmup_capture="$out_dir/warmup-popup-trigger.png"
  local settle_delay_seconds="2"
  CAPTURE_MODE="fullscreen"
  log "triggering macos capture prompt with warmup screenshot"
  screencapture -x "$warmup_capture"
  log "saved $warmup_capture"
  log "waiting ${settle_delay_seconds}s for animation and prompts to settle"
  sleep "$settle_delay_seconds"
  log "capturing full-screen screenshots"

  local i=1
  while ((i <= screenshot_count)); do
    local file="$out_dir/shot-$i.png"
    screencapture -x "$file"
    log "saved $file"
    if ((i < screenshot_count)); then
      sleep "$screenshot_interval"
    fi
    i=$((i + 1))
  done

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
        "$out_dir/shot-$i.png" \
        "$out_dir/shot-$next.png" \
        "$diff_image" \
        2>"$metric_file"
    )" || true
    diff_pixels="$(awk '{ print $1 }' "$metric_file")"
    if ! [[ "$diff_pixels" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
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
diff_total_pairs=$diff_total_pairs
diff_nonzero_pairs=$diff_nonzero_pairs
max_diff_pixels=$max_diff_pixels
motion_observed=$motion_observed
run_log=$run_log
EOF

  ln -sfn "$(basename "$out_dir")" "$latest_link"
  log "result written to $out_dir/result.txt"
}

main "$@"
