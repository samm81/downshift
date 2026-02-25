#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALL_HELPER="$SCRIPT_DIR/helper_install_homebrew_and_tools.bash"
SETUP_HELPER="$SCRIPT_DIR/helper_setup_dev_env.bash"
GUI_HELPER="$SCRIPT_DIR/helper_gui_permissions.bash"

die() {
  printf '[bootstrap-02] error: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '[bootstrap-02] %s\n' "$*"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  die "this script is for macos only"
fi

[[ -x "$INSTALL_HELPER" ]] || die "missing helper: $INSTALL_HELPER"
[[ -x "$SETUP_HELPER" ]] || die "missing helper: $SETUP_HELPER"
[[ -x "$GUI_HELPER" ]] || die "missing helper: $GUI_HELPER"

log "step 1/3: homebrew + core toolchain setup"
"$INSTALL_HELPER"

log "step 2/3: development environment setup"
"$SETUP_HELPER"

if [[ "${SKIP_GUI_PERMISSIONS:-0}" == "1" ]]; then
  log "step 3/3: gui permission bootstrap skipped (SKIP_GUI_PERMISSIONS=1)"
else
  log "step 3/3: gui permission bootstrap"
  if pgrep -x Dock >/dev/null 2>&1; then
    "$GUI_HELPER"
  else
    log "skipping gui permissions because no desktop session is active"
    log "run later from a remote-desktop terminal: ./dev/mac/helper_gui_permissions.bash"
  fi
fi

log "if this is your first bootstrap in this terminal, run: exec zsh"
if [[ "${AUTO_EXEC_ZSH:-0}" == "1" ]]; then
  if command -v zsh >/dev/null 2>&1 && [[ -t 0 && -t 1 ]]; then
    log "AUTO_EXEC_ZSH=1 set; launching a fresh login zsh now"
    exec zsh -l
  else
    log "AUTO_EXEC_ZSH=1 requested, but zsh/tty is unavailable; run 'exec zsh' manually"
  fi
fi

log "done: mac bootstrap complete"
