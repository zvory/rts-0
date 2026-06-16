# Phase 5 - PR-first phase runner

## Status

Pending.

## Goal

Convert `scripts/phase-runner.sh` from local merge/push sequencing to PR-first sequencing that can
either stop after arming auto-merge or wait for a definite merge before continuing.

## Scope

- Add explicit PR mode flags, for example:
  - `--pr` to push the phase branch, open/update a PR, arm auto-merge, and stop;
  - `--pr --wait` to wait for each phase PR to merge before starting the next phase.
- Keep one branch and one worktree per phase.
- Update the inner executor prompt:
  - run focused local verification;
  - commit phase work;
  - do not rely on the full local commit hook;
  - do not merge or push `main`;
  - return enough handoff metadata for the outer runner to open the PR.
- Update the result schema to include PR lifecycle fields such as `pr_number`, `pr_url`,
  `head_sha`, `auto_merge_state`, and `merge_wait_state`.
- For serial `--wait` runs, after each PR merge:
  - fetch `origin/main`;
  - verify the phase head SHA is an ancestor of `origin/main`;
  - only then discover or start the next phase.
- On CI failure, conflict, closed PR, or missing auto-merge, stop with a blocked handoff and leave
  the phase worktree available for repair.
- Keep sparse polling guidance. Sleeping inside shell should not consume model tokens, but output
  should stay bounded.

## Expected touch points

- `scripts/phase-runner.sh`
- `scripts/phase-runner-result.schema.json`
- `scripts/wait-pr.sh` from Phase 4
- `scripts/pr-sweep.sh` from Phase 4
- `plans/README.md`
- `docs/context/planning.md`
- `AGENTS.md`
- `CLAUDE.md`

## Verification

- `scripts/phase-runner.sh --dry-run --pr` prints branch, PR, and auto-merge actions without
  merging local `main`.
- A docs-only test phase can open and arm a PR without running the old full local gate.
- `--pr --wait` does not start a second test phase until the first PR is merged and the first head
  SHA is reachable from `origin/main`.
- A simulated or real failing PR leaves a clear blocked handoff with PR URL and failing check
  names.
- Existing phase-id discovery still supports numeric, decimal, and suffixed phase ids.

## Manual testing focus

Run the phase runner on a tiny throwaway docs-only phase. Confirm an operator can see whether the
runner stopped because the PR is safely armed, because it merged, or because a specific check
failed.

## Handoff expectations

Include examples for single-phase, phase range with non-blocking PRs, and phase range with blocking
serial waits. Note any branch naming or PR title conventions that follow-up agents must preserve.

