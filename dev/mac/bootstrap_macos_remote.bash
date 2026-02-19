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

if [[ "$(uname -s)" != "Darwin" ]]; then
  die "this script is for macos only"
fi

REPO_SSH_URL="${REPO_SSH_URL:-git@github.com:dwsk/breath-ball.git}"
TARGET_DIR_RAW="${TARGET_DIR:-~/src/breath-ball}"
TARGET_DIR="$(expand_home_prefix "$TARGET_DIR_RAW")"
KEY_SRC="${KEY_SRC:-$TARGET_DIR/dev/linux/id_ed25519_breathball}"
KEY_DEST="$HOME/.ssh/id_ed25519_breathball"
PUB_DEST="$HOME/.ssh/id_ed25519_breathball.pub"
SKIP_REPO_CLONE="${SKIP_REPO_CLONE:-0}"

mkdir -p "$HOME/.ssh"
chmod 700 "$HOME/.ssh"

if [[ -n "${BREATHBALL_PRIVATE_KEY_B64:-}" ]]; then
  log "installing breath-ball ssh key from BREATHBALL_PRIVATE_KEY_B64"
  printf '%s' "$BREATHBALL_PRIVATE_KEY_B64" | base64 --decode >"$KEY_DEST"
  chmod 600 "$KEY_DEST"

  if [[ -n "${BREATHBALL_PUBLIC_KEY_B64:-}" ]]; then
    printf '%s' "$BREATHBALL_PUBLIC_KEY_B64" | base64 --decode >"$PUB_DEST"
    chmod 644 "$PUB_DEST"
  elif command -v ssh-keygen >/dev/null 2>&1; then
    ssh-keygen -y -f "$KEY_DEST" >"$PUB_DEST"
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
    ssh-keygen -y -f "$KEY_DEST" >"$PUB_DEST"
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
  ssh-keyscan github.com >>"$HOME/.ssh/known_hosts" 2>/dev/null || true
fi

if ! grep -q "Host github.com" "$HOME/.ssh/config" 2>/dev/null; then
  log "writing github ssh config"
  cat >>"$HOME/.ssh/config" <<CFG
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

log "bootstrap complete"
log "repo path: $TARGET_DIR"
