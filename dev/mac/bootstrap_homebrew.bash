#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[brew-bootstrap] %s\n' "$*"
}

die() {
  printf '[brew-bootstrap] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

require_macos() {
  if [[ "$(uname -s)" != "Darwin" ]]; then
    die "this script is for macos only"
  fi
}

require_sudo() {
  log "checking sudo access"
  sudo -v || die "sudo access is required for one-time mac setup"

  # keep sudo alive while the script runs
  while true; do
    sudo -n true
    sleep 60
    kill -0 "$$" || exit
  done 2>/dev/null &
  SUDO_KEEPALIVE_PID="$!"
  trap 'kill "$SUDO_KEEPALIVE_PID" >/dev/null 2>&1 || true' EXIT
}

ensure_xcode_clt() {
  if xcode-select -p >/dev/null 2>&1; then
    return
  fi

  log "xcode command line tools are missing; requesting install"
  xcode-select --install || true
  die "complete xcode command line tools install, then rerun this script"
}

ensure_homebrew() {
  if have brew; then
    return
  fi

  if ! have curl; then
    die "curl is required to install homebrew"
  fi

  log "installing homebrew"
  NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
}

load_brew_env() {
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
    BREW_BIN="/opt/homebrew/bin/brew"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
    BREW_BIN="/usr/local/bin/brew"
  else
    die "brew executable not found after installation"
  fi
}

persist_brew_env() {
  local profile
  for profile in "$HOME/.zprofile" "$HOME/.zshrc"; do
    touch "$profile"
    if ! grep -q 'brew shellenv' "$profile"; then
      # shellcheck disable=SC2016
      printf 'eval "$(%s shellenv)"\n' "$BREW_BIN" >>"$profile"
    fi
  done
}

install_formula() {
  local formula="$1"
  if brew list "$formula" >/dev/null 2>&1; then
    return
  fi
  log "installing $formula"
  brew install "$formula"
}

ensure_node_and_npm() {
  if have node && have npm; then
    return
  fi
  install_formula node
}

ensure_codex() {
  if have codex; then
    return
  fi

  if ! have npm; then
    die "npm is required to install codex"
  fi

  log "installing codex with npm"
  npm install -g @openai/codex
}

verify() {
  for tool in brew shellcheck shfmt pre-commit node npm codex cliclick; do
    have "$tool" || die "missing expected tool after setup: $tool"
  done

  log "brew: $(brew --version | head -n 1)"
  log "shellcheck: $(shellcheck --version | head -n 1)"
  log "shfmt: $(shfmt --version)"
  log "pre-commit: $(pre-commit --version)"
  log "node: $(node --version)"
  log "npm: $(npm --version)"
  log "codex: $(codex --version)"
}

require_macos
require_sudo
ensure_xcode_clt
ensure_homebrew
load_brew_env
persist_brew_env
install_formula shellcheck
install_formula shfmt
install_formula pre-commit
install_formula cliclick
ensure_node_and_npm
ensure_codex
verify

log "bootstrap complete"
log "next: run ./dev/mac/setup_dev_env.bash"
