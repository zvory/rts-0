# Phase 3 - Lab Operator Action Timeline

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Add replay action variants for authoritative lab mutations and `issueCommandAs`. These actions
should apply through public lab or game APIs and should validate operator permissions, player ids,
entity ids, and action payload bounds. Client-only lab view controls should not become replay
actions unless they affect authoritative playback.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`
- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task/lab.rs`
- Replay action validation tests

## Verification

- Add tests that record lab actions, replay them from the baseline checkpoint, and compare final
  game state.
- Include imported setup or deleted-entity cases to prove entity ids remain stable.

## Manual Testing Focus

In a lab, create or edit units, issue commands as a player, and confirm the same sequence replays
through the shared replay path.

## Handoff

The handoff must list lab operations that are supported, rejected, or still not represented.
