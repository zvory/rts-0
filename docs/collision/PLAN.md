# Collision and Standability - Long-Term Plan

This plan addresses the recurring class of bugs where tanks overlap, units are born on occupied
exits, or large unit bodies appear to clip into buildings.

The goal is not a more elaborate crowd simulator. The goal is one authoritative legality layer for
where bodies may exist, then small integrations in production, construction, movement, collision,
and future local steering.

## Read First

Before implementing any phase:

1. Read `DESIGN.md`.
2. Read `docs/movement/PLAN.md`.
3. Read `docs/movement/PHASE_0_WEIGHTED_COLLISION.md`.
4. Read `docs/movement/PHASE_3_TANK_BODY_LOCOMOTION.md`.
5. Read `docs/movement/PHASE_6_LOCAL_STEERING.md` if touching steering or neighbor avoidance.
6. Read this file and the specific collision phase file.

## Diagnosis

The current system mixes several different meanings of "clear":

- Pathfinding uses tile passability plus coarse radius tiles.
- Movement and collision landing checks only ask whether the unit center lands on a passable tile.
- Spawn search checks radius clearance against terrain/buildings, but ignores other units.
- Production forces a spawn even when no clear point exists.
- Building placement checks whether a unit center tile is inside the footprint, not whether a unit
  body intersects the footprint.
- Invariants check unit-unit overlap, but not unit-body vs building-footprint overlap.

Those are individually understandable shortcuts, but together they mean no module can answer the
basic question: "May this entity body legally exist here?"

## Target Model

Introduce a shared standability and geometry boundary, likely under `server/src/game/services/`.
Suggested module names:

- `geometry.rs`: pure body and intersection helpers.
- `standability.rs`: authoritative predicates for unit standing, building placement, spawn exits,
  and collision push targets.

Keep the model simple and deterministic:

- Unit body: circle centered at `(x, y)` with `config::unit_stats(kind).radius`.
- Building body: axis-aligned rectangle derived from footprint tiles and `TILE_SIZE`.
- Resource node body: blocks building placement only unless a later phase explicitly makes nodes
  solid for unit movement.
- Terrain: tile grid, with exact circle-vs-blocked-tile checks for unit body legality.
- Dynamic units: policy-dependent. Movement may ignore them; spawn and construction must not.

The important design rule:

```text
Every state-creating or state-moving system must call the same standability predicates.
```

`EntityStore::spawn_unit` and `spawn_building` can remain low-level constructors for tests, but
gameplay systems that create or move entities must validate first.

## Policies

Use explicit policy names so callers cannot accidentally inherit the wrong behavior:

- `StaticOnly`: terrain plus building footprints. Used by path following and collision push
  targets. Dynamic unit overlap is handled separately by steering/collision.
- `Spawn`: terrain plus building footprints plus all living unit bodies. Used before production
  creates a unit. Ghost units still block birth; pass-through does not mean "spawn on top of me."
- `BuildIntent`: terrain, existing building footprints, resource nodes, and living unit bodies
  except the chosen builder's own body. Used for command-time validation and client preview so a
  worker can conveniently place a building over itself and then walk to an outside staging point.
  This policy does not create a scaffold.
- `BuildingPlacement`: terrain, existing building footprints, resource nodes, and all living unit
  bodies intersecting the candidate footprint. Used immediately before construction creates a
  scaffold.
- `EjectionFallback`: defensive only. Used for legacy cleanup if an existing bad state is found.
  New gameplay should avoid creating states that need ejection.

## Movement Integration

This collision plan should live below the movement plan, not replace it.

- Movement Phase 0 weighted collision stays useful. Its `FootingProfile`/ghost concept should move
  or be exposed through a shared helper once steering needs it.
- Movement Phase 1 formation goals reduce arrival clumping, but final goal assignment is not a
  legality guarantee. Movement still needs standability at landing time.
- Movement Phase 3 tank body locomotion changes how tanks choose direction and speed. It must still
  validate the final candidate body position through `unit_static_standable`. A tank that cannot
  legally advance should rotate or wait, not slide its body through a building edge.
- Movement Phase 4 weapon facing and Phase 5 facing damage are mostly orthogonal. Turret/barrel
  direction should not affect unit standability.
- Movement Phase 6 local steering should produce candidate directions only. Standability remains
  the authority, and collision resolution remains the hard cleanup after steering.

If Phase 3 exists on a feature branch while collision work starts from `main`, integrate against
the Phase 3 branch before changing movement internals. The Phase 3 hooks are `rotate_toward`,
`angle_delta`, tank speed scaling, and the tank-specific movement budget.

## Non-Negotiable Invariants

After the plan lands:

1. Production never creates a unit whose body intersects terrain, a building footprint, or another
   living unit body.
2. Construction never creates a scaffold whose footprint intersects terrain, another building,
   a resource node, or any living unit body. Build intent may still target the chosen builder's
   current body as a convenience, provided the worker can path to an outside staging point before
   scaffold creation.
3. A non-ghost unit's body never intersects a building footprint at end of tick.
4. Non-ghost unit-unit overlap is reduced by collision resolution in the same tick and remains
   within a documented tolerance only for cases where both legal exits are physically blocked.
5. Movement, collision pushes, spawn search, and construction placement use the same geometry
   helpers.
6. The tick path stays bounded, deterministic, and panic-free.
7. Pathfinding can remain coarse, but final movement legality is exact enough for unit radii.

## Phases

- [Phase 0 - Standability Core](PHASE_0_STANDABILITY_CORE.md)
- [Phase 1 - Production Spawn Correctness](PHASE_1_PRODUCTION_SPAWN.md)
- [Phase 2 - Construction and Building Placement Correctness](PHASE_2_CONSTRUCTION_PLACEMENT.md)
- [Phase 3 - Movement and Collision Integration](PHASE_3_MOVEMENT_COLLISION.md)
- [Phase 4 - Local Steering Alignment](PHASE_4_LOCAL_STEERING_ALIGNMENT.md)
- [Phase 5 - Audit and Removal of Legacy Hacks](PHASE_5_AUDIT.md)

Do not combine phases unless the user explicitly asks for a larger change. Each phase should leave
the repo in a playable state.

## Testing Guidance

Use focused Rust tests for each phase, then broaden:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

The Node integration tests need a running server. Follow `CLAUDE.md` for that flow.
