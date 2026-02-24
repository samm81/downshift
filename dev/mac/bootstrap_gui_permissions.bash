#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[gui-perms] %s\n' "$*"
}

die() {
  printf '[gui-perms] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

require_macos() {
  [[ "$(uname -s)" == "Darwin" ]] || die "this script is for macos only"
}

require_gui_session() {
  pgrep -x Dock >/dev/null 2>&1 || die "no active macos gui session detected. run this from a non-headless desktop session."
}

ensure_tools() {
  local missing=()
  for tool in osascript screencapture open; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done
  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
}

trigger_automation_prompt() {
  log "triggering apple events automation prompt (terminal -> system events)"
  osascript <<'APPLESCRIPT' >/dev/null 2>&1 || true
tell application "System Events"
  return name of every process
end tell
APPLESCRIPT
}

trigger_accessibility_prompt() {
  log "triggering accessibility/ui scripting prompt (system events)"
  osascript <<'APPLESCRIPT' >/dev/null 2>&1 || true
tell application "System Events"
  if exists process "Finder" then
    tell process "Finder"
      return count of windows
    end tell
  end if
end tell
APPLESCRIPT
}

trigger_screen_recording_prompt() {
  local shot="/tmp/downshift-permission-shot-$$.png"
  log "triggering screen capture prompt"
  screencapture -x "$shot" >/dev/null 2>&1 || true
  rm -f "$shot"
}

show_settings_shortcuts() {
  log "if prompts were denied or not shown, open privacy settings and verify terminal permissions"
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation" >/dev/null 2>&1 || true
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility" >/dev/null 2>&1 || true
  open "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture" >/dev/null 2>&1 || true
}

main() {
  require_macos
  require_gui_session
  ensure_tools

  log "this script must be run in a visible desktop session so macos can show prompts"
  trigger_automation_prompt
  trigger_accessibility_prompt
  trigger_screen_recording_prompt
  show_settings_shortcuts
  log "permission bootstrap complete"
}

main "$@"
