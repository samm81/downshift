---
name: release
description: Run the repository release workflow from the mac branch with strict safety checks, including version bump, commit, branch synchronization, rebases, push, tag-based release from README instructions, and returning to mac. Use when preparing and shipping a new release for this repo.
---

# Release

execute each step in order and stop immediately on any failed precondition.

## workflow

1. verify branch and tracked-file clean state.
2. run `git branch --show-current` and require `mac`.
3. run `git status --porcelain --untracked-files=no` and require empty output.
4. if branch is not `mac` or tracked changes are present, stop and report to user. untracked files do not block the release flow.
5. bump project version using the repo's normal version location and format.
6. commit the version bump.
7. update local `main` from `origin`.
8. rebase current `mac` branch on updated `main`.
9. switch to `main`.
10. rebase `main` on `mac`.
11. push branches required by the repo workflow.
12. create and push release tag exactly as documented in `README.md`.
13. switch back to `mac`.

## execution details

1. read `README.md` before tagging to follow the exact release/tag command.
2. if any rebase conflict appears, stop and report conflict files to user.
3. if push or tag push fails, stop and return the exact git error.
4. after completion, report:
   - new version
   - commit hash for version bump
   - pushed branch names
   - pushed tag name
   - final branch (`mac`)
