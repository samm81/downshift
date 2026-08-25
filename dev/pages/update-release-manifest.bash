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
node dev/pages/release-manifest.mjs embed docs/release.json docs/index.html
node dev/pages/release-manifest.mjs validate-embedded docs/release.json docs/index.html

if
  git diff --quiet -- docs/release.json docs/index.html &&
    git diff --cached --quiet -- docs/release.json docs/index.html &&
    git ls-files --error-unmatch -- docs/release.json docs/index.html >/dev/null 2>&1
then
  echo "Pages release metadata is already current for ${TAG_VALUE}"
  exit 0
fi

if [[ "$COMMIT" != true ]]; then
  echo "updated docs/release.json and embedded it in docs/index.html for ${TAG_VALUE} (not committed)"
  exit 0
fi

if [[ "$PUSH" == true ]]; then
  # Rebase before generating the files so the working tree stays clean during
  # synchronization. The generated changes can then be committed on top of
  # the current main branch without asking git to rebase unstaged work.
  git fetch origin main
  git rebase origin/main
fi

git add docs/release.json docs/index.html
if git diff --cached --quiet -- docs/release.json docs/index.html; then
  echo "Pages release metadata is already current for ${TAG_VALUE}"
  exit 0
fi

git commit -m "chore: update Pages release metadata for ${TAG_VALUE}"

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
