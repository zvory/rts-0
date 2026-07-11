# Phase 3 - Game Construction From Checkpoint

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Route normal and lab game construction through a narrow validated-checkpoint import API. Producers
can still differ, but the resulting live `Game` should come from checkpoint state. Retire or
isolate construction paths that duplicate checkpoint import behavior.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `server/src/lobby/**`
- Architecture docs for the `Game` API seam

## Verification

- Run focused sim and lobby tests for normal match and lab start.
- Run the architecture check if `Game` public APIs changed.

## Manual Testing Focus

Start one normal match and one lab, then verify first snapshots and basic commands work.

## Handoff

The handoff must name any remaining direct construction path and why it still exists.
