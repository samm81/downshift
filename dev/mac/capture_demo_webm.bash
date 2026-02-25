#!/usr/bin/env bash
set -euo pipefail
QUIET=0

log() {
  if [[ "$QUIET" == "1" ]]; then
    return
  fi
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
  for tool in cargo ffmpeg; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
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

  local quiet=0
  local demo=0
  local duration=""
  while (($# > 0)); do
    case "$1" in
      --quiet)
        quiet=1
        ;;
      --demo)
        demo=1
        quiet=1
        ;;
      *)
        if [[ -z "$duration" ]]; then
          duration="$1"
        else
          die "unexpected argument: $1"
        fi
        ;;
    esac
    shift
  done
  QUIET="$quiet"

  if [[ -z "$duration" ]]; then
    duration="11.5"
  fi
  if ! [[ "$duration" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
    die "duration must be a number >= 3 seconds"
  fi
  if ! awk "BEGIN { exit !($duration >= 3) }"; then
    die "duration must be a number >= 3 seconds"
  fi
  local setup_delay="3"
  local start_delay="0.3"

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

  log "waiting ${setup_delay}s before launching app for manual setup"
  sleep "$setup_delay"

  log "starting app with cargo run --quiet"
  cargo run --quiet >"$run_log" 2>&1 &
  APP_PID=$!
  trap cleanup EXIT

  local screen_idx
  screen_idx="$(capture_device_index)" || die "failed to detect avfoundation screen capture device"

  log "waiting ${start_delay}s before recording to align loop start"
  sleep "$start_delay"

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
    -vf "fps=30" \
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
  if [[ "$demo" == "1" ]]; then
    printf '%s\n' "$out_webm"
  fi
}

main "$@"
