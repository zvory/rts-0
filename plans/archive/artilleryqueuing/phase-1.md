# Phase 1 - Authoritative Queue Semantics

## Phase Status

Status: pending.

## Objective

Make the server able to represent and execute queued artillery `setup -> pointFire` plans without
trusting the client preview. This phase should change command admission and queue promotion only
where needed for Artillery Point Fire, while preserving current immediate packed-artillery no-op
behavior and terminal queued Point Fire behavior.

## Scope

- Allow queued `useAbility(pointFire)` to append for an owned Artillery when either:
  - current Point Fire command acceptance is already legal, or
  - the unit's current/future order plan has a setup stage that is the direct predecessor for this
    Point Fire stage.
- Keep unqueued Point Fire validation unchanged. Packed artillery without a prior or active
  Point Fire/setup path must still not auto-setup, spend ammo, or emit target markers.
- Keep final Point Fire execution validation at promotion/fire time: live artillery id, ownership,
  kind, deployment state, path state, map-clamped target, min/max range, field-of-fire, faction
  ability eligibility, and ammo affordability.
- Teach queued promotion to distinguish "Point Fire is waiting for this artillery to finish its
  setup" from "Point Fire is stale/invalid." Waiting stages must stay in the queue until deployment
  completes or the queue is explicitly cleared.
- Preserve terminal queue behavior. Once a Point Fire stage is accepted for an artillery unit, later
  queued unit stages must not append behind it for that unit.
- Preserve stop, hold, immediate move/attack/build/deconstruct/ability replacement, death, and queue
  clear semantics.
- Update `docs/design/server-sim.md` and `docs/design/protocol.md` where they currently describe
  Point Fire as requiring a deployed gun at issue time and queued abilities as issue-time-only.

## Expected Touch Points

- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/services/order_execution.rs`
- `server/crates/sim/src/game/entity/order.rs` only if a helper is needed, not for a new wire shape
- `server/crates/sim/src/game/tests/artillery_tests.rs`
- `server/crates/sim/src/game/services/commands/tests/support_weapons.rs`
- `server/crates/sim/src/game/services/order_queue.rs` tests
- `docs/design/server-sim.md`
- `docs/design/protocol.md`

## Edge Cases To Cover

- Queued `setup -> pointFire` from a stationary packed artillery accepts and fires after deployment.
- Queued `move -> setup -> pointFire` accepts even when the Point Fire target is invalid from the
  current position but valid from the arrived position.
- Queued Point Fire target that is still out of range after movement/setup is skipped safely at
  promotion without spending ammo.
- Point Fire remains terminal: a later queued move does not append behind accepted Point Fire.
- Stop/hold/immediate replacement clears the staged setup and Point Fire.
- Mixed Artillery/Rifleman or Artillery/Anti-Tank Gun selections do not give non-artillery units
  Point Fire stages.
- Dead artillery, stale ids, under-construction artillery, invalid coordinates, and empty queues are
  safe no-ops.

## Verification

- Focused Rust tests covering the edge cases above.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if the
  implementation changes sim service boundaries.
- `node tests/protocol_parity.mjs` if docs or projection wording changes alongside protocol
  metadata or order-plan vocabulary.
- `git diff --check`.

## Manual Test Focus

In a local match or dev scenario, queue an artillery move, Shift-queue setup, then Shift-queue
Point Fire. Confirm the gun moves, deploys, and starts firing only after setup completes, and that
Stop cancels the queued plan.

## Handoff Expectations

Name the helper or rule that decides when queued Point Fire may follow setup. Describe how queued
promotion waits through setup and how it avoids waiting forever on stale invalid stages. List any
server behavior intentionally left for the client phase, especially preview, command-card, and
frozen cone affordances.
