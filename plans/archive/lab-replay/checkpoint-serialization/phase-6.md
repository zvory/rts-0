# Phase 6 - Public Checkpoint API and Docs

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Stabilize the public API that room, replay, lab, and tests should use for checkpoint export/import.
Keep validation and import logic inside the simulation boundary. Update docs so future feature work
knows how to add state without breaking checkpoint round trips.

## Expected Touch Points

- `server/crates/sim/src/game/mod.rs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md` if artifact schema references checkpoints
- `docs/context/server-sim.md`
- `docs/context/protocol.md`

## Verification

- Run focused checkpoint tests and the architecture check if public seams changed.
- Run formatting for touched Rust and Markdown if applicable.

## Manual Testing Focus

Start a normal local match and a lab room to confirm no construction path was accidentally changed.

## Handoff

The handoff must describe the stable checkpoint API and the next stage's expected entry point.
