# Phase 1 - ReplayAction Contract and Tick Semantics

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

## Scope

Define the `ReplayAction` schema and timing model. The contract should specify sequence ordering,
tick meaning, actor/operator identity, player id validation, entity-reference validation, and how
future action variants extend the schema. This phase should not yet change playback behavior.

## Expected Touch Points

- `server/crates/sim/src/game/replay.rs` or the new replay DTO module
- `server/crates/protocol/src/lib.rs` if shared artifact DTOs live there
- `docs/design/protocol.md`
- Focused serde and validation tests

## Verification

- Run focused replay DTO tests.
- Add tests for invalid tick order, duplicate sequence, invalid player ids, and oversized actions.

## Manual Testing Focus

No manual gameplay testing is expected.

## Handoff

The handoff must state the chosen tick convention in plain language.
