# Phase 5 - Target Facts And Direct-Fire Legality

## Phase Status

Status: pending.

## Objective

Create explicit rules-owned target facts and reusable direct-fire legality helpers while preserving
current target acquisition behavior. This phase prepares the codebase for multiple weapon policies
without changing which targets existing units choose.

## Scope

- Add a target classification surface in `rts-rules` or a closely related rules-owned module.
- Represent current facts explicitly: unit, building, resource node, armor class, weapon class,
  anti-armor threat, support weapon, field obstacle, vehicle body, economy unit, and current
  coax-infantry-priority eligibility.
- Define current coax infantry-priority eligibility as Worker, Rifleman, and Machine Gunner. Ekat,
  Golems, Mortar Teams, Artillery, Anti-Tank Guns, vehicles, buildings, resources, and Tank Traps are
  not infantry-priority. Keep the classifier easy to extend later, but do not add future unit kinds
  in this phase.
- Replace ad hoc candidate fields in combat acquisition with a target-facts snapshot while keeping
  the same data available to existing priority code.
- Extract direct-fire legality into a helper reusable by default attacks and future secondary
  weapons. It must cover hostile targetability, fog visibility, smoke, LOS, friendly hard blockers,
  targetability, and resource-node exclusion. Include an intended-target mode for future coax use
  that rejects a target when the shot would resolve to an intervening enemy hard blocker instead of
  the intended target.
- Preserve Tank Trap route-obstruction facts and the current special case where infantry does not
  auto-acquire Tank Traps.
- Add exhaustive classification tests across current `EntityKind` values.

## Out Of Scope

- No change to priority ordering.
- No machine-gun-like policy selection.
- No `tank_coax` firing.
- No protocol or client changes.
- No changes to visibility/projection rules beyond moving existing checks behind helpers.

## Expected Touch Points

- `server/crates/rules/src/combat.rs`
- `server/crates/rules/src/kind.rs`
- `server/crates/rules/src/defs.rs`
- `server/crates/rules/src/balance/entrenchment.rs` if infantry-related helpers are consolidated
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/projection.rs`
- `server/crates/sim/src/game/services/world_query.rs`
- `server/crates/sim/src/game/services/combat/tests*.rs`
- `docs/design/server-sim.md`
- `docs/design/balance.md` if the rules classification contract is documented

## Edge Cases To Cover

- Resource nodes remain invalid direct-fire targets.
- Smoke at attacker or target still blocks direct-fire acquisition.
- Fog visibility and team visibility remain unchanged for ordered and auto-acquired targets.
- Terrain LOS and friendly hard blockers still reject direct shots.
- The new intended-target legality mode rejects an enemy infantry candidate behind an enemy
  Tank/building hard blocker when the direct shot would hit the blocker first.
- Mortar indirect fire keeps its current exception from direct LOS/blocker checks.
- Tank Trap route-obstruction behavior is unchanged for vehicles.
- Infantry-like attackers still do not auto-acquire Tank Traps just because the facts surface names
  them as field obstacles.
- Current target candidate sets remain equivalent before priority ranking.

## Verification

- Focused `rts-rules` classification tests over all current entity kinds.
- Focused `rts-sim` tests for smoke, rock/terrain blocking, friendly hard blockers, Tank/building
  shot interception, TankTrap/PumpJack shot behavior, and resource-node non-targetability.
- Existing `server/crates/sim/src/game/services/combat/tests/target_priority.rs` tests.
- Existing Tank Trap combat tests.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs` if docs are changed.
- `git diff --check`

## Manual Test Focus

No required manual gameplay test if candidate/legality tests are strong. If a smoke test is
performed, verify a Tank, Scout Car, Machine Gunner, and infantry still acquire legal visible
targets and still refuse blocked or smoke-hidden targets.

## Handoff Expectations

Name the target facts type, the direct-fire legality helper, the intended-target legality mode, and
any remaining hard-coded entity kind checks. Confirm whether Phase 6 can implement priority policies
entirely from target facts or needs one additional fact exposed.
