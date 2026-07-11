# Phase 5 - Match Capture and History Integration

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Capture ended matches into the new checkpoint-backed replay artifact. The replay's start checkpoint
should be the actual initial state used by the match, and the first timeline should preserve normal
player commands. Match history should store the new artifact shape and avoid offering replay launch
for old incompatible rows. This phase closes the accepted beta dead zone from the schema break by
making newly captured matches launchable again.

## Expected Touch Points

- Match lifecycle capture in `server/src/lobby/room_task/**`
- `server/src/db.rs`
- `server/src/main.rs`
- Replay session tests

## Verification

- Run focused match-history and replay capture tests.
- Run one live Node suite if touched server endpoints affect match history or replay launch.

## Manual Testing Focus

Play or script a short two-player match, open its match-history replay, seek, and inspect both
player perspectives.

## Handoff

The handoff must identify the generated replay artifact location or endpoint future agents should
use for Stage 4 work.
