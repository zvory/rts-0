# Phase 2 - Authoritative Target Locking And Point Fire Setup

## Phase Status

Status: done.

## Objective

Make Point Fire use the new artillery targeting semantics from the requirements. The server should
store a per-gun effective target point locked into that artillery piece's valid range band, and the
Point Fire order should own any needed in-place setup or redeploy before the first shot.

## Scope

- Add a pure helper for artillery fire target locking that accepts map bounds, the artillery origin,
  current or planned setup facing, body facing fallback, min/max range, and raw click position.
- Lock raw clicks to the artillery range band along the origin-to-click ray:
  - inside minimum range locks out to minimum range,
  - outside maximum range locks back to maximum range,
  - already valid range uses the clicked point,
  - zero-length rays use the current/planned setup facing or body facing fallback,
  - final points must be clamped to the playable map along the same ray or the unit ignores the
    command.
- Raise artillery minimum range from 15 tiles to 25 tiles and update Rust rules, client mirrors,
  catalog parity expectations, wiki/generated stats checks, and `docs/design/balance.md`.
- Update unqueued Point Fire command admission so packed artillery can accept a targeted order,
  set up in place toward the locked point, and begin firing after deployment.
- Update deployed Point Fire so a target outside the current cone causes in-place teardown,
  redeploy, and later fire instead of requiring the player to issue setup separately.
- Update queued Point Fire admission and promotion so the stored target is locked from the best
  authoritative future queued position when available, otherwise from the current position.
- Keep Point Fire terminal per artillery unit. Later queued orders for the same gun must not append
  behind accepted Point Fire.
- Preserve no-walking behavior: the fire order may rotate, setup, redeploy, or skip, but it must not
  path toward a raw click to make it valid.
- Preserve final execution checks for liveness, ownership, kind, construction state, deployment,
  path state, faction ability eligibility, range/cone against the stored effective point, ammo, and
  map validity.
- Update `docs/design/server-sim.md` and `docs/design/protocol.md` for Point Fire auto-setup,
  redeploy, target locking, terminal queueing, and final execution validation.

## Expected Touch Points

- `server/crates/sim/src/game/services/order_execution.rs`
- `server/crates/sim/src/game/services/commands.rs`
- `server/crates/sim/src/game/services/order_planner.rs`
- `server/crates/sim/src/game/services/order_queue.rs`
- `server/crates/sim/src/game/entity/order.rs`
- `server/crates/sim/src/game/services/commands/artillery_scatter.rs` if error interpolation needs
  the new 25-to-55 range band
- `server/crates/sim/src/game/tests/artillery_tests.rs`
- `server/crates/sim/src/game/services/commands/tests/abilities.rs`
- `server/crates/sim/src/game/services/commands/tests/support_weapons.rs`
- `server/crates/rules/src/balance*.rs`
- `client/src/config*.js`
- `scripts/check-faction-catalog-parity.mjs`
- `docs/design/server-sim.md`
- `docs/design/protocol.md`
- `docs/design/balance.md`

## Edge Cases To Cover

- Packed artillery accepts immediate Point Fire, stores a locked effective point, sets up in place,
  and fires only after deployment completes.
- Deployed artillery whose locked target is inside the current cone fires from the current setup.
- Deployed artillery whose locked target is outside the current cone tears down, redeploys in place,
  and fires after setup completes.
- Clicks inside 25 tiles lock to the 25-tile floor and do not reject solely because of range.
- Clicks beyond 55 tiles lock to the 55-tile ceiling and do not cause walking.
- Zero-length clicks use the setup/body facing fallback without producing NaN or panic behavior.
- Map-edge rays that cannot produce a valid in-map locked point are safe no-ops for that artillery.
- Multiple selected artillery pieces may store different locked points from the same raw click.
- Queued `move -> pointFire` and `move -> setup -> pointFire` lock from the future position when the
  server can infer it.
- Stop, Hold Position, death, under-construction state, stale ids, invalid coordinates, queue clear,
  and immediate replacement orders safely clear or skip the Point Fire order.
- Ballistic Tables accuracy interpolation uses the 25-to-55 range band without changing the intended
  minimum and maximum error values.

## Verification

- Focused Rust tests for target locking helper behavior, immediate Point Fire setup/redeploy,
  queued Point Fire from future positions, stale target skipping, terminal queue behavior, and
  25-tile minimum range.
- `node scripts/check-faction-catalog-parity.mjs`
- `node scripts/check-wiki.mjs`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if sim
  service boundaries change.
- `node tests/protocol_parity.mjs` if order-plan docs or vocabulary metadata are touched.
- `git diff --check`

## Manual Test Focus

In a local match or dev scenario, select packed artillery and issue Point Fire at close, valid, and
far targets. Confirm the gun does not move, the close and far clicks lock to valid range, the gun
sets up or redeploys in place as needed, and Stop cancels before the first shot.

## Handoff Expectations

Name the target-locking helper and explain how it chooses origin, fallback facing, range clamp, and
map clamp. Summarize which Point Fire paths now auto-setup or redeploy, and list any client preview
work intentionally left to later phases.
