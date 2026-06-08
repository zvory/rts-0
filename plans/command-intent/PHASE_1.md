# Phase 1 - Server Adapter and Existing Command Parity

Status: Planned.

Goal: adapt `services::commands` to use the planner for command families that already exist, without
changing player-visible behavior.

## Scope

- Build a thin adapter in `services::commands` that converts authoritative simulation state into
  `order_planner::UnitFacts`.
- Route existing order families through the planner where their current behavior should be preserved:
  - `move`
  - `attackMove`
  - direct `attack`
  - `gather`
  - `build`
- Keep existing validation strength or improve it only where the design doc already requires it.
- Keep all existing command log and replay shapes unchanged.
- Translate planner actions back into current command-service mutations:
  - `ReplaceActive` clears queued orders and calls existing coordinator/order helpers.
  - `AppendQueued` calls existing `append_queued_order` helpers.
  - notices are emitted only for queue-full cases introduced by this phase.

## Non-Goals

- Do not add new queued order kinds yet.
- Do not change client input behavior.
- Do not change ability semantics except as needed to preserve current behavior through the planner
  adapter.

## Tests

- Existing command-service tests remain green.
- Add direct parity tests showing planner-backed `move`, `attackMove`, `attack`, `gather`, and
  `build` produce the same active order and queued state as before.
- Add queue-full notice tests for valid queued commands that fail only because a unit queue is full.
- Replay determinism tests still pass for existing queued command logs.

## Done

- `services::commands` has one narrow path for building planner facts and applying planner actions.
- Existing gameplay behavior and command logs are unchanged.
- `cargo test -p rts-sim` passes.
