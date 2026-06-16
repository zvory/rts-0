# Phase 5 - Queue Promotion Executor

## Phase Status

- [ ] Not implemented.

## Objective

Apply the planned-action boundary to queued promotion while preserving tick order and queue feel.

## Work

- Keep `systems.rs` promotion timing unchanged.
- Make `order_queue.rs` identify readiness, stale stages, and promotion-time facts, then delegate
  narrow mutations to shared execution helpers where safe.
- Preserve deterministic point-move batching and the semantic differences between issue-time and
  promotion-time validation.
- Cover queued move, attack, gather, build, world ability, self ability, setup, and artillery point
  fire paths.

## Expected Touch Points

- `server/crates/sim/src/game/services/order_queue.rs`
- Planned executor/helper from Phase 4
- `server/crates/sim/src/game/services/ability_orders.rs`
- Entity order helpers from Phase 2
- `server/crates/archcheck/src/lib.rs` if service edges change

## Implementation Checklist

- [ ] Inventory promotion paths and mark which can share command-time execution helpers.
- [ ] Move shared mutation into the executor without changing readiness checks.
- [ ] Add regressions for stale stages, batching order, unaffordable build notice, and ability
  rejection.
- [ ] Confirm tick ordering in `systems.rs` is unchanged.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim order_queue`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim ability`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Test Focus

Smoke-test long Shift queues with movement, attacks, worker build, smoke, support-weapon setup, and
artillery point fire. Confirm queues advance instead of stalling.

## Handoff Expectations

State exactly which helpers are shared between command-time execution and promotion-time execution,
and which behaviors intentionally remain different.
