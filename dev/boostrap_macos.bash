#!/usr/bin/env bash
set -euo pipefail

# local launcher script
# usage:
#   ./dev/boostrap_macos.bash user@MAC_HOST
#
# optional env overrides:
#   REPO_SSH_URL (default: git@github.com:dwsk/breath-ball.git)
#   TARGET_DIR   (default: $HOME/src/breath-ball)
#   SSH_OPTS     (extra ssh flags, e.g. "-i ~/.ssh/scw -p 22")
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
TARGET_DIR="${TARGET_DIR:-\$HOME/src/breath-ball}"

ssh ${SSH_OPTS:-} -t "$REMOTE" \
  "BREATHBALL_PRIVATE_KEY_B64='$PRIVATE_B64' BREATHBALL_PUBLIC_KEY_B64='$PUBLIC_B64' REPO_SSH_URL='$REPO_SSH_URL' TARGET_DIR='$TARGET_DIR' bash -s" \
  < "$HELPER"
