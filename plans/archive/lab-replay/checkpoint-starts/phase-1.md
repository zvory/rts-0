# Phase 1 - Normal Match Start Checkpoints

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Generate a tick-zero `GameCheckpoint` for normal matches using today's map, player, loadout, spawn,
and starting-resource rules. The live match should behave the same, but the checkpoint should be
available as the durable representation of the initial state. Do not change replay artifacts yet.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- Match creation call sites in `server/src/lobby/**`
- Focused sim tests for starting entities/resources

## Verification

- Run focused start-state tests.
- Add a test that normal match start exports a valid checkpoint with exact starting entity ids.

## Manual Testing Focus

Start a local normal match and confirm starting bases, workers, resources, and teams are unchanged.

## Handoff

The handoff must state whether live matches are still constructed directly or already imported from
the generated checkpoint.
