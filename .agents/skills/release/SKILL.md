---
name: release
description: Run the repository release workflow from the current release branch with strict safety checks, including version bump, commit, branch synchronization, rebases, push, tag-based release from README instructions, and returning to the release branch. Use when preparing and shipping a new release for this repo.
---

# Release

execute each step in order and stop immediately on any failed precondition.

## workflow

1. verify branch and tracked-file clean state.
2. run `git branch --show-current` and record the current release branch.
3. run `git status --porcelain --untracked-files=no` and require empty output.
4. if tracked changes are present, stop and report to user. untracked files do not block the release flow.
5. run the full repo verification pass required for releases.
6. use `make verify-release` and require success before continuing.
7. bump project version using the repo's normal version location and format.
8. commit the version bump.
9. update local `main` from `origin`.
10. rebase the current release branch on updated `main`.
11. switch to `main`.
12. rebase `main` on the release branch.
13. push branches required by the repo workflow.
14. create and push release tag exactly as documented in `README.md`.
15. set `RELEASE_TAG` to the tag just pushed, find its tag-triggered `release` GitHub Actions run, and wait for it with `gh run watch --exit-status`. Require the run to pass; this gate includes the Pages deployment and Pages smoke checks.
16. verify that the public website has updated to the new release. Poll `https://getdownshift.app/release.json` for up to 10 minutes, allowing for Pages/CDN propagation. Parse the manifest with Node.js and require its `version` to equal `RELEASE_TAG`; if it does not, stop and report the observed version, endpoint, and release workflow URL.
17. switch back to the release branch.

## execution details

1. read `README.md` before tagging to follow the exact release/tag command.
2. if any rebase conflict appears, stop and report conflict files to user.
3. if push or tag push fails, stop and return the exact git error.
4. after pushing the tag, locate the matching release run rather than assuming the newest run is the correct one. If no matching run appears after a bounded retry, stop and report that the release workflow was not found.
   For example:

   ```bash
   RELEASE_RUN_ID=""
   for attempt in {1..30}; do
     RELEASE_RUN_ID="$(
       gh run list \
         --workflow release.yml \
         --event push \
         --branch "$RELEASE_TAG" \
         --limit 1 \
         --json databaseId \
         --jq '.[0].databaseId // empty'
     )"
     [[ -n "$RELEASE_RUN_ID" ]] && break
     sleep 2
   done
   [[ -n "$RELEASE_RUN_ID" ]] || exit 1
   gh run watch "$RELEASE_RUN_ID" --exit-status
   ```

5. for the website check, use a temporary manifest file so the same repository validator can run before comparing the version:

   ```bash
   RELEASE_TAG="v<version>"
   PAGES_URL="https://getdownshift.app"
   MANIFEST_PATH="$(mktemp)"
   trap 'rm -f "$MANIFEST_PATH"' EXIT

   for attempt in {1..30}; do
     if curl --fail --location --silent --show-error \
       --retry 3 --retry-delay 5 --retry-all-errors \
       "$PAGES_URL/release.json" --output "$MANIFEST_PATH" && \
       node dev/pages/release-manifest.mjs validate "$MANIFEST_PATH" && \
       [[ "$(node -e 'const fs = require("node:fs"); console.log(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).version)' "$MANIFEST_PATH")" == "$RELEASE_TAG" ]]; then
       echo "verified $PAGES_URL/release.json for $RELEASE_TAG"
       break
     fi

     if [[ "$attempt" == 30 ]]; then
       echo "error: $PAGES_URL/release.json did not update to $RELEASE_TAG"
       cat "$MANIFEST_PATH"
       exit 1
     fi
     sleep 20
   done
   ```

   If the manifest remains on an older version, treat the release as failed even if the GitHub Actions run passed.
6. after completion, report:
   - new version
   - commit hash for version bump
   - pushed branch names
   - pushed tag name
   - release workflow run URL and result
   - verified public website version
   - final release branch
