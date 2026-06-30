# Phase 4 - Tank Coax Server Runtime

## Phase Status

Status: pending.

## Objective

Implement the server-authoritative Tank coax firing behavior using the refactored weapon profile,
damage, cooldown, and event plumbing. This phase should make Tanks actually fire the coax, but
client art/audio may still use temporary or fallback feedback until Phase 5.

## Scope

- Add a `tank_coax` weapon profile with 6-tile range, 4 damage, 6-tick cooldown, small-arms weapon
  class, direct-fire legality, and overpenetration enabled.
- Add independent Tank coax cooldown ticking and reset through the weapon-aware cooldown interface.
- Implement coax target search for Tanks only. It should use the current authoritative
  `weapon_facing`/turret direction, not hull facing.
- Gate coax shots to targets within 10 degrees on either side of the current turret direction.
  Use a named constant rather than duplicating anonymous tolerances.
- Reuse or share the direct-fire hostile, visibility, smoke, line-of-sight, targetability,
  resource-node exclusion, and friendly hard-blocker safety checks.
- The coax must not call the normal turret-rotation path, set desired weapon facing, request chase
  paths, clear paths, or replace explicit cannon target intent.
- The coax may fire while the cannon is rotating, ready, reloading, or otherwise unavailable, as
  long as the Tank is in a state where it can expose and attack targets.
- Implement infantry-priority and fallback target selection inside the coax arc. If the
  requirements still do not define infantry precisely, stop and get that product decision before
  coding the target group.
- Emit attack events with the `tank_coax` weapon identity.
- Make coax overpenetration apply with small-arms damage and coax weapon identity.
- Preserve all existing Tank cannon behavior and tests, including stationary range ramp,
  main-cannon cooldown, target priority, turret alignment, firing reveal, overpenetration, and
  moving-fire path retention.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/defs.rs` or the new weapon-profile module from Phase 1
- `server/crates/sim/src/game/entity/state.rs`
- `server/crates/sim/src/game/entity/entity.rs`
- `server/crates/sim/src/game/services/combat/mod.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/weapons.rs`
- `server/crates/sim/src/game/services/combat/damage.rs`
- `server/crates/sim/src/game/services/combat/events.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `docs/design/balance.md`
- `docs/design/server-sim.md`

## Edge Cases To Cover

- In-arc eligible infantry takes 4 base small-arms damage from coax.
- Armored fallback targets take reduced small-arms damage, not Tank AP damage.
- Coax overpenetration hits secondary targets with small-arms damage and emits normal
  overpenetration events.
- Coax prioritizes in-arc infantry-priority targets over fallback targets.
- Ekat is not treated as infantry priority. The chosen Golem/support-weapon behavior is covered by
  tests once the product decision is resolved.
- Coax fires at fallback vehicles or buildings only when no infantry-priority target is legal in arc.
- Coax does not fire outside the arc, outside range, through smoke, through blocked LOS, through
  friendly hard blockers, at resources, or at non-hostile entities.
- Coax cooldown and cannon cooldown are independent in both directions.
- Coax does not rotate the turret toward its target and does not change current cannon target id or
  pathing intent.
- A Tank can fire cannon and coax in the same tick only if the final design allows it and tests make
  the event ordering deterministic; otherwise define and test a deterministic ordering.
- Stale targets, dead targets, non-finite facing, missing combat state, and dead Tanks are safe
  no-ops.

## Verification

- Focused Rust combat tests for coax damage, small-arms armor reduction, overpenetration,
  independent cooldown, arc gating, range gating, target priority, fallback targeting, and no turret
  rotation.
- Focused Rust regression tests for existing Tank cannon targeting, cooldown, stationary range ramp,
  moving-fire path retention, overpenetration, and firing reveal.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`.
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`.

## Manual Test Focus

In a local dev scenario, point a Tank turret at a mixed infantry/vehicle group and confirm the coax
fires only through the turret arc while the cannon behavior remains recognizable. Also check a
moving Tank and a reloading/rotating cannon case to confirm coax opportunity fire does not make the
Tank chase or snap the turret.

## Handoff Expectations

State the final infantry-priority definition, the coax weapon id, the cooldown storage behavior, and
whether same-tick cannon/coax events are allowed. Call out any temporary client feedback limitations
that Phase 5 must replace.
