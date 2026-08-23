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
15. switch back to the release branch.

## execution details

1. read `README.md` before tagging to follow the exact release/tag command.
2. if any rebase conflict appears, stop and report conflict files to user.
3. if push or tag push fails, stop and return the exact git error.
4. after completion, report:
   - new version
   - commit hash for version bump
   - pushed branch names
   - pushed tag name
   - final release branch
