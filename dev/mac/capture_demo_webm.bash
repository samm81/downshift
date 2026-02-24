#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[capture-webm] %s\n' "$*"
}

die() {
  printf '[capture-webm] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

require_macos() {
  [[ "$(uname -s)" == "Darwin" ]] || die "this script is for macos only"
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

require_tools() {
  local missing=()
  for tool in cargo ffmpeg osascript; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
}

get_window_frame() {
  osascript <<'APPLESCRIPT'
try
  tell application "System Events"
    if not (exists process "downshift") then error "process not found"
    tell process "downshift"
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
  local frame=""
  while ((tries < 80)); do
    frame="$(get_window_frame)"
    if [[ "$frame" != ERROR:* ]]; then
      printf '%s\n' "$frame"
      return 0
    fi
    sleep 0.25
    tries=$((tries + 1))
  done
  return 1
}

capture_device_index() {
  local ffmpeg_devices
  ffmpeg_devices="$(ffmpeg -f avfoundation -list_devices true -i "" 2>&1 || true)"
  local line
  line="$(printf '%s\n' "$ffmpeg_devices" | awk '/Capture screen/ { print; exit }')"
  [[ -n "$line" ]] || return 1
  printf '%s\n' "$line" | sed -E 's/.*\[(.*)\].*/\1/'
}

cleanup() {
  if [[ -n "${APP_PID:-}" ]] && kill -0 "$APP_PID" >/dev/null 2>&1; then
    kill "$APP_PID" >/dev/null 2>&1 || true
    wait "$APP_PID" >/dev/null 2>&1 || true
  fi
}

main() {
  require_macos
  load_rust_env_if_present
  require_tools

  local duration="${1:-8}"
  if ! [[ "$duration" =~ ^[0-9]+$ ]] || ((duration < 3)); then
    die "duration must be an integer >= 3 seconds"
  fi

  local script_dir repo_root
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "${script_dir}/../.." && pwd)"
  cd "$repo_root"

  local stamp out_dir run_log raw_video out_webm
  stamp="$(date +%Y%m%d-%H%M%S)"
  out_dir="logs/demo-capture-$stamp"
  run_log="$out_dir/run.log"
  raw_video="$out_dir/raw.mp4"
  out_webm="$out_dir/downshift-demo.webm"
  mkdir -p "$out_dir"

  log "starting app with cargo run --quiet"
  cargo run --quiet >"$run_log" 2>&1 &
  APP_PID=$!
  trap cleanup EXIT

  local frame
  frame="$(wait_for_window)" || die "app window did not become available"

  local frame_x frame_y frame_w frame_h
  IFS=',' read -r frame_x frame_y frame_w frame_h <<<"$frame"

  # keep even dimensions for codec safety.
  frame_w=$((frame_w - (frame_w % 2)))
  frame_h=$((frame_h - (frame_h % 2)))

  local screen_idx
  screen_idx="$(capture_device_index)" || die "failed to detect avfoundation screen capture device"

  log "recording raw screen video for ${duration}s (device index: $screen_idx)"
  ffmpeg -y \
    -f avfoundation \
    -framerate 30 \
    -i "${screen_idx}:none" \
    -t "$duration" \
    -pix_fmt yuv420p \
    "$raw_video" >/dev/null 2>&1

  log "encoding webm"
  ffmpeg -y \
    -i "$raw_video" \
    -vf "crop=${frame_w}:${frame_h}:${frame_x}:${frame_y},fps=12,scale=640:-2:flags=lanczos" \
    -an \
    -c:v libvpx-vp9 \
    -b:v 0 \
    -crf 34 \
    -row-mt 1 \
    "$out_webm" >/dev/null 2>&1

  cat >"$out_dir/result.txt" <<EOF
duration_seconds=$duration
raw_video=$raw_video
webm=$out_webm
run_log=$run_log
EOF

  log "done: $out_webm"
}

main "$@"
