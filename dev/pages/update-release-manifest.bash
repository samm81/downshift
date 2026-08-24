#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

TAG_VALUE="${TAG:-}"
COMMIT=false
PUSH=false

if [[ $# -gt 0 && "$1" != --* ]]; then
  TAG_VALUE="$1"
  shift
fi
while [[ $# -gt 0 ]]; do
  case "$1" in
    --commit)
      COMMIT=true
      ;;
    --push)
      COMMIT=true
      PUSH=true
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      exit 2
      ;;
  esac
  shift
done

if [[ -z "$TAG_VALUE" ]]; then
  echo "error: a stable release tag is required (for example TAG=v0.2.0)" >&2
  exit 2
fi
if [[ ! "$TAG_VALUE" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: release tag must be a stable tag such as v0.2.0" >&2
  exit 2
fi
if [[ "$PUSH" == true && "$(git branch --show-current)" != "main" ]]; then
  echo "error: --push is only allowed from the main branch" >&2
  exit 2
fi

REPOSITORY="${GITHUB_REPOSITORY:-samm81/downshift}"
RELEASE_JSON="$(mktemp)"
trap 'rm -f "$RELEASE_JSON"' EXIT

gh api "repos/${REPOSITORY}/releases/tags/${TAG_VALUE}" >"$RELEASE_JSON"
node dev/pages/release-manifest.mjs generate "$RELEASE_JSON" docs/release.json

if
  git diff --quiet -- docs/release.json &&
    git diff --cached --quiet -- docs/release.json &&
    git ls-files --error-unmatch -- docs/release.json >/dev/null 2>&1
then
  echo "Pages release manifest is already current for ${TAG_VALUE}"
  exit 0
fi

if [[ "$COMMIT" != true ]]; then
  echo "updated docs/release.json for ${TAG_VALUE} (not committed)"
  exit 0
fi

if [[ "$PUSH" == true ]]; then
  git fetch origin main
  git rebase origin/main
fi

git add docs/release.json
if git diff --cached --quiet -- docs/release.json; then
  echo "Pages release manifest is already current for ${TAG_VALUE}"
  exit 0
fi

git commit -m "chore: update Pages release manifest for ${TAG_VALUE}"

if [[ "$PUSH" == true ]]; then
  for attempt in 1 2 3; do
    if git push origin HEAD:main; then
      exit 0
    fi
    if [[ "$attempt" == 3 ]]; then
      echo "error: failed to push Pages release manifest after ${attempt} attempts" >&2
      exit 1
    fi
    git fetch origin main
    git rebase origin/main
  done
fi
