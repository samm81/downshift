#!/usr/bin/env bash
set -euo pipefail

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
  for tool in cargo screencapture shasum; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
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

wait_for_app_start() {
  local tries=0
  while ((tries < 40)); do
    if kill -0 "$APP_PID" >/dev/null 2>&1; then
      return 0
    fi
    tries=$((tries + 1))
    sleep 0.5
  done
  return 1
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
  ensure_tools

  local fail_if_static=0
  local screenshot_count="${1:-3}"
  local screenshot_interval="${2:-1}"
  while (($# > 0)); do
    case "$1" in
      --fail-if-static)
        fail_if_static=1
        shift
        ;;
      *)
        break
        ;;
    esac
  done
  screenshot_count="${1:-3}"
  screenshot_interval="${2:-1}"

  local script_dir repo_root
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "${script_dir}/../.." && pwd)"
  cd "$repo_root"

  local stamp
  stamp="$(date +%Y%m%d-%H%M%S)"
  local out_dir="logs/gui-smoke-$stamp"
  local run_log="$out_dir/run.log"
  local latest_link="logs/latest-gui-smoke"

  if ! [[ "$screenshot_count" =~ ^[0-9]+$ ]] || ((screenshot_count < 2)); then
    die "first arg must be screenshot count (integer >= 2)"
  fi
  if ! [[ "$screenshot_interval" =~ ^([0-9]+|[0-9]*\.[0-9]+)$ ]]; then
    die "second arg must be interval seconds (for example: 1 or 0.5)"
  fi

  mkdir -p "$out_dir"

  log "starting app with cargo run --quiet"
  cargo run --quiet >"$run_log" 2>&1 &
  APP_PID=$!
  trap cleanup EXIT

  wait_for_app_start || die "app process did not stay up long enough to verify"

  CAPTURE_MODE="fullscreen"
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

  local unique_hashes
  unique_hashes="$(shasum "$out_dir"/shot-*.png | awk '{print $1}' | sort -u | wc -l | tr -d ' ')"
  local motion_observed="no"
  if ((unique_hashes > 1)); then
    motion_observed="yes"
  fi

  cat >"$out_dir/result.txt" <<EOF
capture_mode=$CAPTURE_MODE
screenshot_count=$screenshot_count
screenshot_interval_seconds=$screenshot_interval
unique_image_hashes=$unique_hashes
motion_observed=$motion_observed
run_log=$run_log
EOF

  ln -sfn "$(basename "$out_dir")" "$latest_link"
  log "result written to $out_dir/result.txt"
  log "motion_observed=$motion_observed (unique_image_hashes=$unique_hashes)"

  if [[ "$fail_if_static" == "1" && "$motion_observed" != "yes" ]]; then
    die "no visual motion detected in captured screenshots"
  fi
}

main "$@"
