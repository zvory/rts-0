# Movement System - Multi-Phase Implementation Plan

This plan turns `docs/movement-options-research.md` into handoff-sized implementation phases.
The goal is not polished global crowd flow. Keep tile A*, `MoveCoordinator`, and the rough RTS
traffic feel. Improve the worst jank in layers: local collision, final group goals, rendered
angles, vehicle body state, weapon state, and only then short-range steering.

Every phase should leave the repo in a working state. Do not combine phases unless the user
explicitly asks for a larger change.

## Read First

Before coding any phase:

1. Read `DESIGN.md`.
2. Read `docs/movement-options-research.md`.
3. Read `docs/collision/PLAN.md` if the phase touches unit positions, collision, steering,
   production spawn exits, building placement, or static passability.
4. Read this file.
5. Read the specific phase file.
6. Verify the checkout, branch, and worktree per `CLAUDE.md`.

## Phases

- [Phase 0 - Weighted collision and footing](PHASE_0_WEIGHTED_COLLISION.md)
- [Phase 1 - Distance-sensitive formation goals](PHASE_1_FORMATION_GOALS.md)
- [Phase 2 - Client angle interpolation](PHASE_2_ANGLE_INTERPOLATION.md)
- [Phase 3 - Tank body locomotion](PHASE_3_TANK_BODY_LOCOMOTION.md)
- [Phase 4 - Weapon facing and aim gates](PHASE_4_WEAPON_FACING.md)
- [Phase 5 - Facing-aware tank damage](PHASE_5_FACING_DAMAGE.md)
- [Phase 6 - Local steering](PHASE_6_LOCAL_STEERING.md)
- [Phase 7 - Integration audit and hardening pass](PHASE_7_INTEGRATION_AUDIT.md)

## Current Code Anchors

- `server/src/game/services/movement.rs`
  - `movement_system` consumes waypoint paths and moves units in world pixels.
  - Tank body locomotion rate-limits tank hull facing and scales movement by body-angle error.
  - `resolve_collisions` performs resistance-weighted iterative unit overlap resolution.
  - `is_collision_anchored` currently means pass-through ghost behavior for harvesting and
    constructing workers.
- `server/src/game/services/move_coordinator.rs`
  - `order_group_move` assigns distance-sensitive formation goals.
  - `formation_goals` preserves or compacts rough group shape based on order distance.
  - `find_spawn_point` handles production exits today; `docs/collision` replaces its local
    clearance checks with shared standability.
  - `process_awaiting_paths` budgets fresh A* requests.
- `server/src/game/entity.rs`
  - `MovementState` owns `facing`, path state, repath state, and stuck/sidestep counters.
  - `CombatState` owns cooldown, target id, and machine-gunner setup state.
- `server/src/game/services/combat.rs`
  - Combat keeps non-tanks facing targets instantly.
  - Tanks rotate independent `weaponFacing` toward targets and gate firing on turret alignment.
    Hull/body `facing` remains movement locomotion state.
  - Machine gunners already have packed, setting-up, deployed, and tearing-down setup state.
- `server/src/protocol.rs` and `client/src/protocol.js`
  - Snapshot entity optional fields are mirrored and compact-encoded.
  - Protocol changes must update both files and `DESIGN.md` together.
- `client/src/state.js`
  - `entitiesInterpolated` interpolates `x`, `y`, and shortest-arc `facing` / `weaponFacing`.
- `client/src/renderer.js`
  - Tanks render hulls from body `facing` and barrels/muzzle flashes from
    `weaponFacing ?? facing`.

## Non-Negotiable Invariants

1. The server remains authoritative. Clients send commands only.
2. Fog remains cheat-proof. Do not expose hidden target ids, hidden positions, or hidden
   target-derived angles in snapshots or events.
3. `Game::tick()` stays panic-free. Treat stale ids, missing targets, invalid paths, and blocked
   movement as no-ops or recoverable states.
4. Keep pathfinding bounded. Do not add per-unit unbounded search, dynamic global obstacle maps, or
   all-pairs work outside the existing spatial-index pattern.
5. Keep determinism. Iteration order must be stable; sort ids when needed; no unseeded randomness
   in simulation.
6. Do not replace tile A* or `MoveCoordinator` in this plan.
7. If a phase changes a wire field, update `server/src/protocol.rs`, `client/src/protocol.js`, and
   `DESIGN.md` in the same change.
8. If a phase changes mirrored balance/config values, update `server/src/config.rs`,
   `client/src/config.js`, and `DESIGN.md` together.
9. Each phase must add or update tests for the behavior it changes.
10. Movement may remain a soft dynamic-traffic model, but static body legality belongs to
    `docs/collision`: systems that accept unit positions or static movement goals use the shared
    standability predicates.

## Default Product Choices

Use these defaults unless the user overrides them:

- Harvesting and actively constructing workers stay `Ghost` for movement and unit-unit collision:
  pass-through, not solid. This does not make them ignorable for production spawn or building
  placement; those policies are defined in `docs/collision`.
- Idle and moving infantry are `Soft`.
- Firing riflemen and explicit hold-like idle combatants are `Firm` once a hold/deploy command
  exists. Before that command exists, firing riflemen are the main `Firm` infantry case.
- Deployed or setting-up crew weapons are `Braced`.
- Tanks are `Heavy`.
- Long-order formation offsets stay in world orientation. Do not rotate offsets toward movement
  direction in Phase 1. Formation goal tiles are still filtered through body-aware static
  standability for the specific unit kind.
- `facing` remains the tank body/hull facing. `weaponFacing` is turret/barrel facing.
- Local steering is short-range only. It proposes movement directions; `docs/collision`
  standability remains the authority for static body legality. Do not build flow fields, ORCA/RVO,
  continuum crowds, or a dynamic global cost field in this plan.

## Deferred Options

These are intentionally not implementation phases yet:

- Persistent standing slots and deployment reservations.
- Flow fields, continuum crowds, ORCA/RVO, or shared dynamic cost maps.
- Rotated marching formations.
- Shared arrival-time movement.
- Full crew-weapon firing arcs beyond the `weaponFacing` seam.

Create a new plan before implementing any of these. They add enough state and invalidation behavior
that they need their own design pass.

## Testing Guidance

Use targeted tests while developing a phase, then broaden before handing off:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

The Node integration tests need a running server. Follow the commands in `CLAUDE.md` for that flow.
