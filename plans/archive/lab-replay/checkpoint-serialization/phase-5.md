# Phase 5 - Deterministic Resume Harness

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Build a reusable test harness that exports checkpoints from representative games, restores them,
runs both original and restored games forward, and compares semantic results. The harness should
cover movement, combat, fog, production, buildings, smoke or mortar, and longer match duration.
Generated artifacts should live under `target/`.

## Expected Touch Points

- Sim test helpers
- Replay or self-play helper scripts only if they are the best source of representative scenarios
- `docs/context/testing.md` if a new targeted command is added

## Verification

- Run the new harness on a small representative set.
- Ensure generated goldens or artifacts are ignored and not staged.

## Manual Testing Focus

No manual gameplay testing is required beyond opening one failure artifact if the harness produces
one.

## Handoff

The handoff must include the exact command for future checkpoint/replay agents to run.
