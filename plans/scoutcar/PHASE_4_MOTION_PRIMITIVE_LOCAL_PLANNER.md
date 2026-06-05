# Phase 4 - Motion Primitive Local Planner

## Goal

Replace scout-car waypoint stepping with a short-horizon local planner that chooses legal car-like
motions from the current pose.

## Rationale

The existing controller computes a desired facing, rotates toward it, then steps. That is still a
point-waypoint controller wearing a car costume. A scout car should choose from possible vehicle
motions and only accept motions whose swept body is legal.

## Motion Primitives

Candidate primitives per tick or short horizon:

- forward straight;
- forward left arcs at several curvatures;
- forward right arcs at several curvatures;
- reverse straight;
- optional no-op only when no legal movement exists.

Do not add reverse turning in this phase.

## Candidate Scoring

Score each candidate by:

- progress along the route corridor or toward the final goal;
- final and swept static clearance;
- alignment with the route lookahead;
- steering smoothness;
- reverse penalty;
- traffic penalty;
- blocked-front penalty.

Reject candidates before scoring if the swept capsule clips static blockers.

## Route Corridor

The global A* path remains useful, but the local planner should treat it as a corridor. It may choose
a nearby legal pose that is not exactly on the tile-center polyline if that pose makes better
clearance/progress tradeoffs.

## Code Areas

- new `server/src/game/services/movement/scout_car.rs` or similarly focused module
- `server/src/game/services/movement/waypoints.rs`
- `server/src/game/services/movement/tank_drive.rs` only to remove scout-car-specific logic from it
- `server/src/game/entity/state.rs` if minimal planner state is needed

## Tests

- Scout car turns around a building corner without nose-contact loops.
- Scout car in a wall-parallel lane chooses forward motion that stays off the wall.
- Far-behind goals still start with a broad forward turn, not reverse.
- Nearby behind goals can use straight reverse.
- Candidate selection is deterministic when two candidates have equal score.
- No candidate can accept a statically illegal final pose.

## Done When

- Scout-car movement no longer depends on exact interception of intermediate waypoint centers.
- The car can make continuous progress around tight corners through chosen arcs rather than
  blocked-step recovery.
