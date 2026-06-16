# Phase 2 - Entity Order Mutation Helpers

## Phase Status

- [x] Done.

## Objective

Move repeated order mutation sequences behind narrow, named entity APIs without changing behavior.

## Work

- Add helper methods for replacing the active order, clearing the active order, clearing all orders,
  appending queued intents, popping a promoted stage, and updating path/goal state where current
  call sites duplicate fragile sequences.
- Update command, queue, ability, and movement call sites only where the helper directly replaces an
  existing mutation sequence.
- Keep helper names semantic enough that future command code does not need to know the field-level
  order state layout.

## Expected Touch Points

- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/entity/`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- Movement coordinator files only if they share the same mutation sequences.

## Implementation Checklist

- [x] Identify repeated direct order-field mutation patterns.
- [x] Add the smallest helper set needed to name those patterns.
- [x] Convert command/order/ability call sites incrementally.
- [x] Confirm archcheck field-write ratchets do not expand outside entity-owned modules.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim command`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim order_queue`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Test Focus

Smoke-test stop, move, attack-move, immediate ability use, queued ability use, support-weapon setup,
and cancellation paths that should clear or preserve orders.

## Handoff Expectations

Document every new helper and the semantic difference between replace, clear active, clear all,
append queued, and promote queued.
