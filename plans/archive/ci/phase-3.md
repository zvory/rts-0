# Phase 3 - Local hooks and agent contract

## Status

Done.

## Goal

Stop using the developer laptop as the authoritative full gate and make the agent instructions
match the PR-first repository contract.

## Scope

- Change local hooks so ordinary commits no longer run `./tests/run-all.sh` by default.
- Keep cheap checks that catch obvious mistakes, such as `git diff --check`, if they are low-risk.
- Preserve focused local verification guidance for touched areas.
- Rewrite direct-merge instructions in agent-facing docs.
- Replace "merge branch to `main` and push" with "push branch, open owned PR, arm auto-merge, and
  either stop with PR evidence or wait for a definite merge when serial work requires it."
- Define completion states for normal agents:
  - merged to `main`;
  - PR opened, owned, and auto-merge armed;
  - blocked with PR link and exact failing check or human decision needed.
- Update docs that currently say every local commit must pass `tests/run-all.sh`.

## Expected touch points

- `hooks/_gate-main.sh`
- `hooks/pre-commit`
- `hooks/pre-merge-commit`
- `scripts/install-hooks.sh`
- `AGENTS.md`
- `CLAUDE.md`
- `README.md`
- `docs/context/testing.md`
- `docs/design/testing.md`
- `docs/context/planning.md`
- `plans/README.md`

## Verification

- In a disposable worktree, a docs-only commit does not invoke the full local gate.
- The new hook still fails on the intentionally retained cheap checks, if any.
- `rg` no longer finds stale default instructions telling agents to avoid PRs or direct-merge
  branches into `main`.
- Existing focused test commands remain documented.

## Manual testing focus

Read the updated instructions as an agent would. Confirm there is exactly one normal lifecycle and
that it does not depend on a human remembering to watch an unowned PR.

## Handoff expectations

Call out any remaining archived plans that still mention direct merges. Archived plan text does not
need to be rewritten unless it can confuse active automation.
