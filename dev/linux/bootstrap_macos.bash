#!/usr/bin/env bash
set -euo pipefail

# linux-side launcher script (provisioning/orchestration only)
# usage:
#   ./dev/linux/bootstrap_macos.bash user@MAC_HOST
#
# optional env overrides:
#   REPO_SSH_URL            (default: git@github.com:dwsk/downshift.git)
#   TARGET_DIR              (default: ~/src/downshift, expanded on remote)
#   MACOS_REMOTE_SSH_IDENTITY (path to ssh key used to connect to the remote host)
#   SSH_OPTS                (extra ssh flags; if set, takes precedence over auto ssh flags)
#   REPO_SOURCE_MODE        (local-sync|remote-git, default: local-sync)
#   LINK_REPO_CODEX_CONFIG  (1|0, default: 1; link ~/.codex/config.toml -> repo dev/codex/config.toml)
#   SYNC_GIT_CONFIG         (1|0, default: 1; sync ~/.config/git/config to remote ~/.gitconfig)
#   LOCAL_GIT_CONFIG        (default: ~/.config/git/config)
#
# this wrapper reads local key files and passes them to the remote helper:
#   ./dev/linux/id_ed25519_downshift
#   ./dev/linux/id_ed25519_downshift.pub

if [[ $# -ne 1 ]]; then
  echo "usage: $0 user@MAC_HOST" >&2
  exit 1
fi

REMOTE="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HELPER="$REPO_ROOT/dev/mac/bootstrap_macos_remote.bash"
LOCAL_KEY="$SCRIPT_DIR/id_ed25519_downshift"
LOCAL_PUB="$SCRIPT_DIR/id_ed25519_downshift.pub"

if [[ ! -f "$HELPER" ]]; then
  echo "helper script not found: $HELPER" >&2
  exit 1
fi

if [[ ! -f "$LOCAL_KEY" ]]; then
  echo "private key not found: $LOCAL_KEY" >&2
  exit 1
fi

PRIVATE_B64="$(base64 <"$LOCAL_KEY" | tr -d '\n')"
PUBLIC_B64=""
if [[ -f "$LOCAL_PUB" ]]; then
  PUBLIC_B64="$(base64 <"$LOCAL_PUB" | tr -d '\n')"
fi

REPO_SSH_URL="${REPO_SSH_URL:-git@github.com:dwsk/downshift.git}"
TARGET_DIR="${TARGET_DIR:-~/src/downshift}"
REPO_SOURCE_MODE="${REPO_SOURCE_MODE:-local-sync}"
LINK_REPO_CODEX_CONFIG="${LINK_REPO_CODEX_CONFIG:-1}"
SYNC_GIT_CONFIG="${SYNC_GIT_CONFIG:-1}"
LOCAL_GIT_CONFIG="${LOCAL_GIT_CONFIG:-$HOME/.config/git/config}"

if [[ -n "${SSH_OPTS:-}" ]]; then
  # shellcheck disable=SC2206
  SSH_CMD=(ssh ${SSH_OPTS})
else
  SSH_CMD=(ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new)
  if [[ -n "${MACOS_REMOTE_SSH_IDENTITY:-}" ]]; then
    SSH_CMD+=(-i "$MACOS_REMOTE_SSH_IDENTITY" -o IdentitiesOnly=yes)
  fi
fi

SKIP_REPO_CLONE=0
if [[ "$REPO_SOURCE_MODE" == "local-sync" ]]; then
  SKIP_REPO_CLONE=1
fi

"${SSH_CMD[@]}" -t "$REMOTE" \
  "DOWNSHIFT_PRIVATE_KEY_B64='$PRIVATE_B64' DOWNSHIFT_PUBLIC_KEY_B64='$PUBLIC_B64' REPO_SSH_URL='$REPO_SSH_URL' TARGET_DIR='$TARGET_DIR' SKIP_REPO_CLONE='$SKIP_REPO_CLONE' bash -s" \
  <"$HELPER"

if [[ "$REPO_SOURCE_MODE" == "local-sync" ]]; then
  echo "[bootstrap] syncing local checkout to remote target"

  tar \
    --exclude='.git/index.lock' \
    --exclude='Session.vim' \
    -C "$REPO_ROOT" -cf - . |
    "${SSH_CMD[@]}" "$REMOTE" \
      "TARGET_DIR='$TARGET_DIR' /bin/bash -lc 'TARGET_DIR_RESOLVED=\"\${TARGET_DIR/#\\\$HOME/\$HOME}\"; TARGET_DIR_RESOLVED=\"\${TARGET_DIR_RESOLVED/#\~/\$HOME}\"; mkdir -p \"\$TARGET_DIR_RESOLVED\"; tar -xf - -C \"\$TARGET_DIR_RESOLVED\"'"

  echo "[bootstrap] local sync complete"
fi

if [[ "$LINK_REPO_CODEX_CONFIG" == "1" ]]; then
  echo "[bootstrap] linking remote codex config to repo config.toml"
  "${SSH_CMD[@]}" "$REMOTE" \
    "TARGET_DIR='$TARGET_DIR' /bin/bash -lc 'TARGET_DIR_RESOLVED=\"\${TARGET_DIR/#\\\$HOME/\$HOME}\"; TARGET_DIR_RESOLVED=\"\${TARGET_DIR_RESOLVED/#\~/\$HOME}\"; mkdir -p \"\$HOME/.codex\"; ln -sfn \"\$TARGET_DIR_RESOLVED/dev/codex/config.toml\" \"\$HOME/.codex/config.toml\"; echo \"[bootstrap] codex config link: \$HOME/.codex/config.toml -> \$TARGET_DIR_RESOLVED/dev/codex/config.toml\"'"
fi

if [[ "$SYNC_GIT_CONFIG" == "1" ]]; then
  echo "[bootstrap] syncing local git config to remote ~/.gitconfig"
  if [[ ! -f "$LOCAL_GIT_CONFIG" ]]; then
    echo "local git config not found: $LOCAL_GIT_CONFIG" >&2
    exit 1
  fi

  cat "$LOCAL_GIT_CONFIG" |
    "${SSH_CMD[@]}" "$REMOTE" \
      "/bin/bash -lc 'cat > \"\$HOME/.gitconfig\"'"
  "${SSH_CMD[@]}" "$REMOTE" \
    "/bin/bash -lc 'git config --global commit.gpgsign false; git config --global --unset-all user.signingkey || true; git config --global tag.gpgsign false'"
  echo "[bootstrap] git config sync complete"
fi

echo "[bootstrap] complete: remote machine is ready for remote-desktop development"
echo "[bootstrap] next (run on remote mac):"
echo "[bootstrap]   ssh $REMOTE"
echo "[bootstrap]   cd $TARGET_DIR"
echo "[bootstrap]   ./dev/mac/bootstrap_homebrew.bash"
echo "[bootstrap]   ./dev/mac/setup_dev_env.bash"
