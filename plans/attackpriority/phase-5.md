# Phase 5 - Tank Trap Obstruction Context

## Phase Status

- [ ] Not started.

## Objective

Make vehicle Tank Trap targeting depend on movement obstruction instead of raw proximity. Vehicles
should focus enemy Tank Traps when those traps are actually blocking, pinching, or immediately
obstructing their movement, while irrelevant nearby traps should not outrank real combat threats.

## Work

- Add a small sim-owned obstruction context that combat acquisition can ask for without owning
  pathfinding internals.
- Prefer a narrow helper near movement/pathing/occupancy that can answer questions such as:
  - is this enemy Tank Trap on or adjacent to the attacker's current path segment;
  - is this trap between the attacker and current movement/path goal within a short forward window;
  - is this trap part of a vehicle-body pinch or closed gap that prevents progress;
  - is this trap the reason a recent vehicle-body path/chase request failed or stopped.
- Keep the helper deterministic and bounded. Do not run expensive full A-star for every candidate
  during combat acquisition.
- Feed obstruction facts into the Phase 2/3 ranking boundary as a high context tier for vehicle-body
  attackers only.
- Preserve infantry behavior:
  - infantry-like auto-acquisition still ignores Tank Traps;
  - explicit attacks against visible enemy Tank Traps remain legal;
  - Tank Traps remain shot-transparent where current combat rules say they are.
- Preserve non-obstructing vehicle behavior:
  - a nearby Tank Trap that is not in front, on the route, pinching movement, or associated with a
    blocked path should not outrank an Anti-Tank Gun, Tank, or other meaningful combat target;
  - vehicles still fall back to attacking a Tank Trap when it is the best legal relevant target.
- Add tests and dev-scenario coverage:
  - Tank prioritizes an Anti-Tank Gun over an irrelevant nearby Tank Trap;
  - Tank prioritizes an obstructing Tank Trap over a lower-threat soft target when breaching;
  - Tank or Scout Car attacks a trap in a blocked/pinched path and progresses after destruction;
  - Rifleman passes through or ignores enemy Tank Traps without auto-attacking;
  - explicit Rifleman attack on a visible enemy Tank Trap still works.
- Update `docs/design/server-sim.md`, `docs/design/balance.md`, or Tank Trap-related docs if the
  movement/combat contract changes.

## Expected Touch Points

- `server/crates/sim/src/game/services/combat/priority.rs`
- `server/crates/sim/src/game/services/combat/acquisition.rs`
- `server/crates/sim/src/game/services/combat/tests.rs`
- `server/crates/sim/src/game/services/occupancy.rs`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/services/move_coordinator.rs`
- `server/crates/sim/src/game/setup/dev_scenarios/layouts/tank_traps.rs` if scenario coverage is
  extended
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Implementation Checklist

- [ ] Add a bounded obstruction-context helper.
- [ ] Feed obstruction context into target ranking for vehicle-body attackers.
- [ ] Keep infantry Tank Trap auto-acquisition filtering intact.
- [ ] Preserve explicit Tank Trap attack commands.
- [ ] Add obstructing versus irrelevant Tank Trap priority tests.
- [ ] Add or update dev scenarios if useful for manual inspection.
- [ ] Update design docs for the movement/combat contract.
- [ ] Run focused verification and record exact commands.
- [ ] Mark this phase as done in this file.

## Verification

```bash
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::combat
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::movement
cargo test --manifest-path server/Cargo.toml -p rts-sim game::services::pathing
cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture
node scripts/check-docs-health.mjs
git diff --check
```

If dev scenarios or client-visible references change, also run the relevant scenario/client contract
checks named by the implementation.

## Manual Test Focus

Open the Tank Trap pathing/dev scenario matrix or an equivalent local match setup. Confirm vehicles
breach traps that are actually in their way, vehicles do not ignore an Anti-Tank Gun just because a
non-obstructing Tank Trap is nearby, infantry does not stop to shoot traps on attack-move, and direct
trap attacks still work.

## Handoff Expectations

Report the exact obstruction definition, where the helper lives, and how expensive it is per combat
tick. Include manual scenario notes for vehicle breaching, irrelevant nearby traps, infantry
pass-through, and explicit trap attacks.
