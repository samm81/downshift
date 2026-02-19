#!/usr/bin/env bash
set -euo pipefail

# local launcher script
# usage:
#   ./dev/boostrap_macos.bash user@MAC_HOST
#
# optional env overrides:
#   REPO_SSH_URL            (default: git@github.com:dwsk/breath-ball.git)
#   TARGET_DIR              (default: ~/src/breath-ball, expanded on remote)
#   MACOS_REMOTE_SSH_IDENTITY (path to ssh key used to connect to the remote host)
#   SSH_OPTS                (extra ssh flags; if set, takes precedence over auto ssh flags)
#   REPO_SOURCE_MODE        (local-sync|remote-git, default: local-sync)
#
# this wrapper reads local key files and passes them to the remote helper:
#   ./dev/id_ed25519_breathball
#   ./dev/id_ed25519_breathball.pub

if [[ $# -ne 1 ]]; then
  echo "usage: $0 user@MAC_HOST" >&2
  exit 1
fi

REMOTE="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
HELPER="$SCRIPT_DIR/bootstrap_macos_remote.bash"
LOCAL_KEY="$SCRIPT_DIR/id_ed25519_breathball"
LOCAL_PUB="$SCRIPT_DIR/id_ed25519_breathball.pub"

if [[ ! -f "$HELPER" ]]; then
  echo "helper script not found: $HELPER" >&2
  exit 1
fi

if [[ ! -f "$LOCAL_KEY" ]]; then
  echo "private key not found: $LOCAL_KEY" >&2
  exit 1
fi

PRIVATE_B64="$(base64 < "$LOCAL_KEY" | tr -d '\n')"
PUBLIC_B64=""
if [[ -f "$LOCAL_PUB" ]]; then
  PUBLIC_B64="$(base64 < "$LOCAL_PUB" | tr -d '\n')"
fi

REPO_SSH_URL="${REPO_SSH_URL:-git@github.com:dwsk/breath-ball.git}"
TARGET_DIR="${TARGET_DIR:-~/src/breath-ball}"
REPO_SOURCE_MODE="${REPO_SOURCE_MODE:-local-sync}"

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
  "BREATHBALL_PRIVATE_KEY_B64='$PRIVATE_B64' BREATHBALL_PUBLIC_KEY_B64='$PUBLIC_B64' REPO_SSH_URL='$REPO_SSH_URL' TARGET_DIR='$TARGET_DIR' SKIP_REPO_CLONE='$SKIP_REPO_CLONE' bash -s" \
  < "$HELPER"

if [[ "$REPO_SOURCE_MODE" == "local-sync" ]]; then
  echo "[bootstrap] syncing local checkout to remote target"

  tar \
    --exclude='.git/index.lock' \
    --exclude='Session.vim' \
    -C "$REPO_ROOT" -cf - . \
    | "${SSH_CMD[@]}" "$REMOTE" \
      "TARGET_DIR='$TARGET_DIR' /bin/bash -lc 'TARGET_DIR_RESOLVED=\"\${TARGET_DIR/#\\\$HOME/\$HOME}\"; TARGET_DIR_RESOLVED=\"\${TARGET_DIR_RESOLVED/#\~/\$HOME}\"; mkdir -p \"\$TARGET_DIR_RESOLVED\"; tar -xf - -C \"\$TARGET_DIR_RESOLVED\"'"

  echo "[bootstrap] local sync complete"
fi

echo "[bootstrap] complete: remote machine is ready for remote-desktop development"
