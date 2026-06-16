# Phase 6 - Cleanup, recovery, and rollout validation

## Status

Done.

## Goal

Close the reliability gaps left after switching to PR-first CI: cleanup must understand
auto-merged PR branches, stale PRs must be discoverable, and the team must have evidence that the
new contract works before relying on it for broad work.

## Scope

- Update worktree cleanup for PR-first branches:
  - remove clean worktrees whose branch was merged through a PR;
  - tolerate remote branch auto-deletion;
  - never remove a worktree with uncommitted changes or an unmerged head.
- Add recovery docs for common states:
  - CI failed;
  - branch stale or conflicted;
  - auto-merge not armed;
  - PR closed unmerged;
  - GitHub API unavailable;
  - emergency admin/direct push.
- Add a final workflow canary:
  - one docs-only PR through auto-merge;
  - one representative implementation PR through auto-merge;
  - one serial phase-runner `--pr --wait` exercise on throwaway docs-only phases.
- Audit active docs and plan templates for old direct-merge language.
- Document how to move the same gate to another runner if GitHub Actions pricing or public-repo
  terms change.

## Expected touch points

- `scripts/cleanup-worktrees.sh`
- `README.md`
- `AGENTS.md`
- `CLAUDE.md`
- `docs/context/planning.md`
- `docs/context/testing.md`
- `plans/ci/plan.md`
- Any active plan files whose instructions would conflict with the new lifecycle

## Verification

- Cleanup removes a clean merged PR worktree and keeps an unmerged clean PR worktree.
- Cleanup handles a remote branch that GitHub auto-deleted after merge.
- `scripts/pr-sweep.sh` reports no unowned open `zvorygin/*` PRs after the canary work.
- The canary docs-only PR and representative implementation PR both merge through auto-merge.
- The serial canary confirms the runner waits for definite merge before continuing.

## Manual testing focus

Review the canary PRs and sweep output. Confirm the workflow answers these questions without extra
context: what is waiting, who owns it, whether CI failed, whether auto-merge is armed, and whether a
serial runner can safely proceed.

## Handoff expectations

Summarize the final lifecycle and any known exceptions. If a fallback remains manual, name the exact
command and decision point.
