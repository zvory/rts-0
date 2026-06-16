# Phase 1 - Characterization Tests

## Phase Status

- [ ] Not implemented.

## Objective

Lock down current command and queued-order behavior before refactoring production code.

## Work

- Add focused tests for command budget rejection, queue-full notices, mixed-selection order
  handling, stale queued stage skipping, and promotion-time affordability.
- Add focused tests for ability issue versus queued promotion behavior, including cooldown, charges,
  costs, and movement preservation.
- Add focused tests for worker build/gather queue promotion, support-weapon setup transitions, and
  artillery point fire where coverage is thin.
- Keep production changes limited to tiny test helpers if existing APIs make behavior impossible to
  assert.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/ability_orders.rs`
- Existing test modules under `server/crates/sim/src/game/`

## Implementation Checklist

- [ ] Inventory existing command/order tests and name missing high-risk behaviors.
- [ ] Add characterization tests for immediate command admission and rejection.
- [ ] Add characterization tests for queued promotion and stale-stage behavior.
- [ ] Add characterization tests for ability issue-time versus promotion-time semantics.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim command`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim order_queue`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim ability`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`

## Manual Test Focus

In a local match, smoke-test move, attack-move, Shift queued movement, queued worker build,
queued smoke or point ability, support-weapon setup after movement, and artillery point fire.

## Handoff Expectations

List every behavior now covered, any ambiguity discovered, and which tests later phases must keep
green before changing command or queue internals.
