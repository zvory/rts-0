# Tank Path Smoothing - Multi-Phase Plan

This plan fixes tank visual sliding and tile-path jiggle without replacing the existing server
authoritative movement model. The current movement system uses 8-direction tile A*, converts every
tile to a waypoint center, and then lets tanks rotate their hull toward a short lookahead point.
That keeps pathing correct, but it makes tanks visibly correct their heading for tile-center noise,
local steering, sidesteps, and collision pushes.

The target design is:

- Tile A* remains the reachability solver.
- Static terrain/building legality remains authoritative on the server.
- Tanks and other units follow smoother, longer route segments when the swept body can legally
  travel straight.
- Tank hull facing follows the same smoothed segment the tank is trying to move along.
- Client rendering may polish interpolation, but it must not hide a different authoritative armor
  facing.

## Read First

Before implementing any phase:

1. Read `CLAUDE.md`.
2. Read `DESIGN.md`, especially movement/pathing, tank facing, fog, and hardening sections.
3. Read all files in this directory.
4. Inspect the actual current code before editing:
   - `server/src/game/pathfinding.rs`
   - `server/src/game/services/pathing.rs`
   - `server/src/game/services/move_coordinator.rs`
   - `server/src/game/services/movement.rs`
   - `server/src/game/services/standability.rs`
   - `server/src/game/entity.rs`
   - `client/src/state.js`
   - `client/src/renderer.js`

## Problem Statement

The visible bug is "tanks slide." The root cause is a mismatch between path semantics and tank
presentation:

- A* returns a tile-center route.
- Movement consumes short waypoints, including diagonal grid artifacts.
- Tanks have bounded hull rotation and speed scaling based on hull alignment.
- The tank hull is visually large enough that small course corrections are obvious.
- Local steering and collision may move the body without changing the route-level intent.

Do not solve this by making the hull instantly face the velocity vector. Tank hull facing is
gameplay-facing because armor uses it. The authoritative facing should become less noisy because
the movement intent is less noisy.

## Phases

- [Phase 0 - Investigation and fixtures](PHASE_0_INVESTIGATION.md)
- [Phase 1 - Static line-of-sight and swept-body checks](PHASE_1_STATIC_SEGMENTS.md)
- [Phase 2 - Path simplification after A*](PHASE_2_PATH_SIMPLIFICATION.md)
- [Phase 3 - Tank route follower and long lookahead](PHASE_3_TANK_ROUTE_FOLLOWER.md)
- [Phase 4 - Optional tank turn-cost pathing](PHASE_4_TURN_COST_PATHING.md)
- [Phase 5 - Client visual polish only](PHASE_5_CLIENT_POLISH.md)
- [Phase 6 - Integration, regression tests, and docs](PHASE_6_INTEGRATION.md)

## Non-Negotiable Invariants

1. The server remains authoritative for position, path progression, and tank hull facing.
2. Tank armor must use the same `facing` value the client sees for owned/visible tanks.
3. No path smoothing may allow a tank to clip through terrain, water, rocks, buildings, scaffolds,
   or map bounds.
4. Unit-unit collision remains a movement-system concern. Smoothing checks only static blockers
   unless a phase explicitly says otherwise.
5. Movement and replay behavior must stay deterministic. Sort ids and candidates wherever order can
   affect outcomes.
6. `Game::tick()` must remain panic-free. No `unwrap`, `expect`, or unchecked indexing on tick
   paths.
7. If a phase changes a contract described in `DESIGN.md`, update `DESIGN.md` in the same change.
8. Do not edit generated or Bazel files. This repo uses Rust/JS commands from `CLAUDE.md`.

## Implementation Strategy

Implement each phase as a separate change if possible. Stop after each phase with targeted tests
green. Do not jump directly to client-side hiding; that can make the armor-facing model misleading.

The critical implementation sequence is:

1. Prove segment legality.
2. Simplify tile waypoint lists into legal straight segments.
3. Make tanks face and move according to those segments.
4. Only then tune path preferences or visual interpolation.

## Suggested Test Commands

Use targeted Rust tests first, then broader tests after behavior changes:

```bash
cd server && cargo fmt && cargo test
node tests/server_integration.mjs
node tests/regression.mjs
node tests/ai_integration.mjs
cd tests && npm install && node client_smoke.mjs
```

For documentation-only changes, no runtime test is required beyond checking the files are present.

