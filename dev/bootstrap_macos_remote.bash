#!/usr/bin/env bash
set -euo pipefail

# remote helper script
# this script is intended to run on the mac host (stdin via ssh is fine).

log() {
  printf '[bootstrap] %s\n' "$*"
}

die() {
  printf '[bootstrap] error: %s\n' "$*" >&2
  exit 1
}

expand_home_prefix() {
  local p="$1"
  p="${p/#\$HOME/$HOME}"
  p="${p/#\~/$HOME}"
  printf '%s' "$p"
}

ensure_nvm_loaded() {
  export NVM_DIR="$HOME/.nvm"
  if [[ -s "$NVM_DIR/nvm.sh" ]]; then
    # shellcheck disable=SC1090
    . "$NVM_DIR/nvm.sh"
  fi
}

ensure_nvm() {
  ensure_nvm_loaded
  if command -v nvm >/dev/null 2>&1; then
    return
  fi

  if ! command -v curl >/dev/null 2>&1; then
    die "curl is required to install nvm"
  fi

  log "installing nvm"
  curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash
  ensure_nvm_loaded

  if ! command -v nvm >/dev/null 2>&1; then
    die "nvm installation failed"
  fi
}

ensure_shell_profile_for_nvm() {
  local profile="$HOME/.zshrc"
  if [[ ! -f "$profile" ]]; then
    touch "$profile"
  fi

  if ! grep -q 'export NVM_DIR="$HOME/.nvm"' "$profile"; then
    cat >> "$profile" <<'CFG'
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
CFG
  fi
}

ensure_npm() {
  if command -v npm >/dev/null 2>&1; then
    return
  fi

  ensure_nvm
  ensure_shell_profile_for_nvm
  log "installing node lts (includes npm) via nvm"
  nvm install --lts
  nvm alias default 'lts/*' >/dev/null 2>&1 || true
  nvm use --lts >/dev/null

  if ! command -v npm >/dev/null 2>&1; then
    die "npm installation failed"
  fi
}

ensure_codex() {
  ensure_nvm_loaded

  if command -v codex >/dev/null 2>&1; then
    log "codex already installed: $(codex --version 2>/dev/null || echo 'version unavailable')"
    return
  fi

  ensure_npm
  log "installing codex via npm"
  npm install -g @openai/codex

  if ! command -v codex >/dev/null 2>&1; then
    die "codex installation failed"
  fi

  log "codex installed: $(codex --version 2>/dev/null || echo 'version unavailable')"
}

if [[ "$(uname -s)" != "Darwin" ]]; then
  die "this script is for macos only"
fi

REPO_SSH_URL="${REPO_SSH_URL:-git@github.com:dwsk/breath-ball.git}"
TARGET_DIR_RAW="${TARGET_DIR:-~/src/breath-ball}"
TARGET_DIR="$(expand_home_prefix "$TARGET_DIR_RAW")"
KEY_SRC="${KEY_SRC:-$HOME/dev/id_ed25519_breathball}"
KEY_DEST="$HOME/.ssh/id_ed25519_breathball"
PUB_DEST="$HOME/.ssh/id_ed25519_breathball.pub"
SKIP_REPO_CLONE="${SKIP_REPO_CLONE:-0}"

mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"

if [[ -n "${BREATHBALL_PRIVATE_KEY_B64:-}" ]]; then
  log "installing breath-ball ssh key from BREATHBALL_PRIVATE_KEY_B64"
  printf '%s' "$BREATHBALL_PRIVATE_KEY_B64" | base64 --decode > "$KEY_DEST"
  chmod 600 "$KEY_DEST"

  if [[ -n "${BREATHBALL_PUBLIC_KEY_B64:-}" ]]; then
    printf '%s' "$BREATHBALL_PUBLIC_KEY_B64" | base64 --decode > "$PUB_DEST"
    chmod 644 "$PUB_DEST"
  elif command -v ssh-keygen >/dev/null 2>&1; then
    ssh-keygen -y -f "$KEY_DEST" > "$PUB_DEST"
    chmod 644 "$PUB_DEST"
  fi
elif [[ -f "$KEY_DEST" ]]; then
  log "using existing key at $KEY_DEST"
elif [[ -f "$KEY_SRC" ]]; then
  log "copying key from $KEY_SRC to $KEY_DEST"
  cp "$KEY_SRC" "$KEY_DEST"
  chmod 600 "$KEY_DEST"

  if [[ -f "${KEY_SRC}.pub" ]]; then
    cp "${KEY_SRC}.pub" "$PUB_DEST"
    chmod 644 "$PUB_DEST"
  elif command -v ssh-keygen >/dev/null 2>&1; then
    ssh-keygen -y -f "$KEY_DEST" > "$PUB_DEST"
    chmod 644 "$PUB_DEST"
  fi
else
  die "no key found. provide BREATHBALL_PRIVATE_KEY_B64, or place key at $KEY_DEST or $KEY_SRC"
fi

if ! command -v git >/dev/null 2>&1; then
  die "git is required but not installed"
fi

if ! grep -q "^github.com " "$HOME/.ssh/known_hosts" 2>/dev/null; then
  log "adding github.com to known_hosts"
  ssh-keyscan github.com >> "$HOME/.ssh/known_hosts" 2>/dev/null || true
fi

if ! grep -q "Host github.com" "$HOME/.ssh/config" 2>/dev/null; then
  log "writing github ssh config"
  cat >> "$HOME/.ssh/config" <<CFG
Host github.com
  HostName github.com
  User git
  IdentityFile $KEY_DEST
  IdentitiesOnly yes
CFG
  chmod 600 "$HOME/.ssh/config"
fi

if [[ "$SKIP_REPO_CLONE" == "1" ]]; then
  log "skipping remote git clone/fetch (local sync mode)"
  mkdir -p "$TARGET_DIR"
else
  if [[ -d "$TARGET_DIR/.git" ]]; then
    log "repo already exists at $TARGET_DIR; fetching latest"
    GIT_SSH_COMMAND="ssh -i $KEY_DEST -o IdentitiesOnly=yes" git -C "$TARGET_DIR" fetch --all --prune
  else
    log "cloning $REPO_SSH_URL into $TARGET_DIR"
    mkdir -p "$(dirname "$TARGET_DIR")"
    GIT_SSH_COMMAND="ssh -i $KEY_DEST -o IdentitiesOnly=yes" git clone "$REPO_SSH_URL" "$TARGET_DIR"
  fi
fi

ensure_codex

log "bootstrap complete"
log "repo path: $TARGET_DIR"
