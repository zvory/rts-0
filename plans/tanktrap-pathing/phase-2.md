# Phase 2 - Owner-Aware Pathing and Acquisition Policy

Status: planned.

## Goal

Implement the Tank Trap rule changes and prove them with the Phase 1 scenario matrix. Vehicles
should route around friendly/allied Tank Traps, breach enemy Tank Trap walls by naturally attacking
them, and infantry-like units should pass through Tank Traps without auto-attacking unless directly
ordered.

## Scope

- Make vehicle-body static path blocking owner-aware for Tank Traps.
- Keep own/allied Tank Traps in the vehicle-body blocker layer.
- Exclude enemy Tank Traps from the vehicle-body static path blocker layer so vehicle paths can
  route into breachable enemy obstacle lines.
- Preserve ordinary all-ground blockers for normal buildings and terrain.
- Preserve infantry pass-through for all Tank Traps.
- Add an auto-acquisition predicate that filters Tank Traps out for infantry-like attackers only.
- Keep Tank Traps eligible as normal auto-acquisition targets for vehicle-body attackers.
- Preserve explicit attack orders for all combat-capable units against visible enemy Tank Traps.
- Update docs if the owner-aware pathing contract becomes part of a design source of truth.

## Implementation Direction

Prefer one shared relation helper instead of scattered branches. A useful shape is:

```rust
enum StaticObstacleRelation {
    PassThrough,
    Avoid,
    Breach,
}
```

or an equivalent policy function that can answer:

- whether a static obstacle blocks path planning for this moving unit and team relation;
- whether a static obstacle is relevant for auto-acquisition by this attacker;
- whether explicit attack orders remain valid.

The helper should use `MovementBodyClass`, `StaticBlockerClass`, and team relation data. Avoid raw
kind lists except where existing definitions already map entity kinds into those classes.

## Expected Touch Points

- `server/crates/rules/src/kind.rs` for any shared rules-level relation helper that does not need
  team data.
- `server/crates/sim/src/game/services/occupancy.rs` for owner-aware Tank Trap blocker layers or
  path-planning views.
- `server/crates/sim/src/game/services/pathing.rs` for path requests, fingerprints, and cache keys
  if owner/team relation affects passability.
- `server/crates/sim/src/game/services/standability.rs` only if standability must distinguish
  own/allied/enemy vehicle blockers. Be careful: final physical placement should still prevent
  vehicle bodies from occupying live Tank Trap tiles.
- `server/crates/sim/src/game/services/movement/` if movement needs a local collision/breach
  handoff after pathing routes into enemy traps.
- `server/crates/sim/src/game/services/combat/acquisition.rs` for auto-acquisition filtering.
- `server/crates/sim/src/game/services/world_query.rs` if the target filtering API should take
  attacker kind/policy instead of repeating closures in acquisition.
- `server/crates/sim/src/game/services/order_queue.rs` only if queued explicit attack promotion
  accidentally applies auto-acquisition filtering. Direct attacks must stay legal.
- Phase 1 scenario and regression tests.

## Acceptance Criteria

- Friendly/allied Tank Trap walls still route vehicle-body units around the wall.
- Enemy Tank Trap walls no longer cause vehicle-body A-star paths to avoid the whole wall as if it
  were friendly terrain.
- Tanks and Scout Cars that hit an enemy Tank Trap wall emit attack events against traps, destroy
  enough traps to open a route, and proceed toward their movement goal.
- Vehicle-body units still prefer enemy units over buildings through the existing combat priority
  logic. When no enemy unit is selected, enemy Tank Traps remain eligible nearest targetable
  buildings.
- Riflemen, Machine Gunners, and Workers moving or attack-moving through enemy Tank Traps do not
  auto-acquire them, stop for them, set up because of them, or emit attack events against them.
- Direct attack orders from Riflemen, Machine Gunners, and Workers against visible enemy Tank Traps
  still work when the unit can attack.
- Charged Riflemen preserve existing moving-fire behavior for direct attacks and still do not
  auto-acquire traps on ordinary move or attack-move orders.
- Own/allied Tank Traps cannot be attacked by own/allied units under normal hostile-target
  validation.
- Existing Tank Trap placement, line spacing, construction, fog, elimination, and damage behavior
  do not regress.

## Verification

Run the focused Phase 1 scenario/test commands plus any direct tests added for the new policy. Likely
commands:

```bash
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -E 'package(rts-sim) & test(tank_trap)'
cargo nextest run --config-file .config/nextest.toml --manifest-path server/Cargo.toml --profile default -E 'package(rts-rules) & test(tank_trap)'
```

If path-cache fingerprints or crate boundaries change, also run:

```bash
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
```

Use the PR `./tests/run-all.sh` gate as the final merge authority.

## Manual Testing Focus

Open the Phase 1 dev scenarios locally. Check one friendly vehicle reroute, one enemy vehicle
breach, one Rifleman pass-through, one Machine Gunner pass-through, and one explicit charged
Rifleman attack against an enemy Tank Trap.

## Handoff Expectations

Mark this phase done in the implementation commit. The handoff must summarize the final obstacle
relation policy, the exact tests run, manual scenario observations, any path-cache or architecture
implications, and whether docs were updated or intentionally left unchanged.
