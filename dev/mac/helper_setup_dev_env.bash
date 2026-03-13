#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[setup] %s\n' "$*"
}

die() {
  printf '[setup] error: %s\n' "$*" >&2
  exit 1
}

have() {
  command -v "$1" >/dev/null 2>&1
}

ensure_rust() {
  if have cargo && have rustc; then
    return
  fi

  have curl || die "curl is required to install rust toolchain"

  log "installing rust toolchain with rustup"
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
}

ensure_rust_components() {
  have rustup || die "rustup is required to install rust components"
  log "ensuring rustfmt and clippy components are installed"
  rustup component add rustfmt clippy
}

load_rust_env_if_present() {
  if have cargo && have rustc; then
    return
  fi

  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1091
    source "$HOME/.cargo/env"
  fi
}

load_brew_env_if_present() {
  if have brew; then
    return
  fi

  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  fi
}

install_with_brew() {
  local formula="$1"
  if brew list "$formula" >/dev/null 2>&1; then
    return
  fi
  log "installing $formula with homebrew"
  brew install "$formula"
}

install_prereqs() {
  [[ "$(uname -s)" == "Darwin" ]] || die "this script is for macos only"
  load_brew_env_if_present
  have brew || die "homebrew is required on macos. run ./dev/mac/helper_install_homebrew_and_tools.bash first."
  install_with_brew shellcheck
  install_with_brew shfmt
  install_with_brew pre-commit
  install_with_brew cliclick
  ensure_rust
  load_rust_env_if_present
  ensure_rust_components
}

verify_tools() {
  local missing=()
  for tool in shellcheck shfmt pre-commit node npm codex cargo rustc rustfmt cargo-clippy cliclick; do
    if ! have "$tool"; then
      missing+=("$tool")
    fi
  done

  if ((${#missing[@]} > 0)); then
    die "missing required tools: ${missing[*]}"
  fi
}

setup_hooks() {
  if ! pre-commit install >/dev/null 2>&1; then
    die "pre-commit install failed"
  fi
  log "installed git pre-commit hook"
}

show_versions() {
  log "shellcheck: $(shellcheck --version | head -n 1)"
  log "shfmt: $(shfmt --version)"
  log "pre-commit: $(pre-commit --version)"
  log "node: $(node --version)"
  log "npm: $(npm --version)"
  log "codex: $(codex --version)"
  log "cargo: $(cargo --version)"
  log "rustc: $(rustc --version)"
  log "rustfmt: $(rustfmt --version)"
  log "clippy: $(cargo clippy --version)"
}

install_prereqs
verify_tools
setup_hooks
show_versions

log "development environment setup complete"
