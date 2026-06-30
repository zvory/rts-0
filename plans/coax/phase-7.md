# Phase 7 - Tank Coax Server Runtime

## Phase Status

Status: pending.

## Objective

Implement the server-authoritative Tank coax firing behavior using the refactored weapon profile,
damage, cooldown, event, target facts, and priority-policy surfaces. This phase makes Tanks
actually fire the coax, while detailed client art/audio polish remains in Phase 8.

## Scope

- Add the live `tank_coax` weapon profile with 6-tile range, 4 damage, 6-tick cooldown, small-arms
  weapon class, direct-fire legality, and overpenetration enabled.
- Give Tanks an additional secondary weapon without changing their default `tank_cannon` profile.
- Tick and reset `tank_coax` cooldown independently from `tank_cannon`.
- Implement a Tank-only secondary firing pass that evaluates legal targets inside the current
  authoritative turret/weapon facing arc.
- Gate coax shots to targets within 10 degrees on either side of current turret facing. Use a named
  constant.
- Reuse direct-fire hostile, visibility, smoke, line-of-sight, targetability, resource-node
  exclusion, and friendly-hard-blocker safety checks.
- Use the machine-gun-like priority policy from Phase 6: infantry-priority first, fallback legal
  targets second, distance/id ties.
- Emit `Event::Attack` with `weaponKind: "tank_coax"`.
- Apply coax overpenetration with small-arms damage and coax overpenetration policy.
- Preserve Tank cannon target selection, turret rotation, stationary range ramp, cooldown, firing
  reveal, overpenetration, movement/path retention, and event behavior.

## Out Of Scope

- No client Tank rig coax barrel.
- No final coax-specific audio/visual polish beyond the Phase 4 fallback behavior.
- No command-card UI, toggle, upgrade, research, range display, cost/supply/sight/trainability
  change, or balance tuning outside the approved coax profile.
- No change to explicit Tank cannon intent or pathing.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/defs.rs` or the weapon-profile module from Phase 1
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

- In-arc Worker, Rifleman, or Machine Gunner takes 4 base small-arms damage from coax.
- Ekat, Golem, Mortar Team, Artillery, and Anti-Tank Gun are not infantry-priority for coax.
- Armored fallback targets take reduced small-arms damage, not Tank AP damage.
- Coax overpenetration hits secondary targets with small-arms damage and does not use Tank cannon
  facing multipliers.
- Coax prioritizes in-arc infantry-priority targets over fallback targets.
- Coax fires at fallback vehicles or buildings only when no infantry-priority target is legal in
  arc.
- Coax does not fire outside the 10-degree arc, outside 6-tile range, through smoke, through
  blocked LOS, through friendly hard blockers, at resources, at non-hostile entities, or at hidden
  targets.
- Coax cooldown and cannon cooldown are independent in both directions.
- Coax does not rotate the turret, set desired weapon facing, change `target_id` used by the cannon,
  clear paths, request chase paths, or alter current movement intent.
- A Tank can fire cannon and coax in the same tick only if tests define deterministic event ordering;
  otherwise enforce and test a deterministic priority.
- Stale targets, dead targets, non-finite facing, missing combat state, and dead Tanks are safe
  no-ops.

## Verification

- Focused Rust combat tests for coax damage, small-arms armor reduction, overpenetration,
  independent cooldown, arc gating, range gating, target priority, fallback targeting, and no turret
  rotation/pathing changes.
- Focused Rust regression tests for existing Tank cannon targeting, cooldown, stationary range
  ramp, moving-fire path retention, overpenetration, and firing reveal.
- `cargo test --manifest-path server/Cargo.toml -p rts-sim coax`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim tank_combat`
- `node tests/protocol_parity.mjs` if weapon ids or event fields are touched.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

In a local dev scenario, point a Tank turret at a mixed infantry/vehicle/building group and confirm
the coax fires only through the turret arc while cannon behavior remains recognizable. Also check a
moving Tank and a reloading/rotating cannon case to confirm coax opportunity fire does not make the
Tank chase, snap the turret, or drop its path.

## Handoff Expectations

State the final coax weapon id, profile values, infantry-priority definition, cooldown behavior,
event ordering decision, and any temporary client feedback limitations that Phase 8 must replace.
