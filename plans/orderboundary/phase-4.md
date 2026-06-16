# Phase 4 - Planned Action Executor

## Phase Status

- [ ] Not implemented.

## Objective

Separate planned order mutation from command validation and decoding.

## Work

- Extract the mutation side of `order_planner` results into a narrow planned-action executor.
- Keep `commands.rs` responsible for ownership, visibility, command-budget, issue-time cost checks,
  and planner fact construction.
- Keep `order_planner` pure and avoid adding stateful service imports to it.
- Update archcheck service classifications or allowlists only if a new executor module is created,
  and explain why the edge is narrower than the current adapter.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_planner.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- `server/crates/archcheck/src/lib.rs` if a new service module is introduced

## Implementation Checklist

- [ ] Name the planned effect types that currently cause direct mutation.
- [ ] Create a narrow executor or private helper for planned effects.
- [ ] Route immediate command application through the executor.
- [ ] Preserve ability launch and preserve-movement semantics.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim order_planner`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim command`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Test Focus

Smoke-test mixed selections, repeated queued smoke allocation, immediate reactive smoke, attack
orders, move orders, and command-budget edge cases.

## Handoff Expectations

Include a before/after responsibility map for command validation, pure planning, and mutation.
