# Phase 2 - Car-Aware Path Following

## Goal

Change scout cars from exact waypoint interception to car-aware route following. This should solve
the "beside the waypoint" jiggle without making scout cars pivot like tanks.

## Approach

Introduce scout-car-specific waypoint consumption and lookahead inside the existing movement seam.
The coarse A* route remains the source of intent, but the car should steer toward a drivable point on
that route instead of always chasing the immediate waypoint center.

## Steps

1. Add helper functions near the existing vehicle movement helpers:
   - `scout_car_accepts_waypoint`;
   - `scout_car_desired_path_point`;
   - `scout_car_final_goal_tolerance`;
   - small vector helpers for along-track and lateral error.
2. For intermediate waypoints, consume the waypoint when any of these are true:
   - current position is within the scout-car acceptance radius;
   - the car has passed the waypoint along the route segment;
   - the next route segment is statically reachable from the current body position and facing.
3. For final goals, avoid exact-arrival-only behavior for scout cars:
   - if the car is within final tolerance and lateral error dominates along-track error, mark arrived;
   - keep exact arrival when the goal is directly reachable along the car's current travel direction.
4. Rework scout car drive intent to use lookahead:
   - choose a point ahead on the current statically legal route segment;
   - bound lookahead so it does not aim through buildings or terrain;
   - keep the existing reverse behavior for nearby behind goals.
5. Keep scout cars out of generic infantry sidestep and tank pivot logic.
6. Preserve deterministic ordering and existing per-tick movement budgets.

## Code Areas

- `server/src/game/services/movement/waypoints.rs`
- `server/src/game/services/movement/tank_drive.rs`
- `server/src/game/services/movement/tests.rs`
- `server/src/config.rs`
- `DESIGN.md`

## Edge Cases

- A final goal next to a building should not let the scout car settle inside the building clearance.
- An attack-move scout car that settles at its movement destination should still keep the expected
  combat semantics after arrival.
- Tolerant arrival should not accidentally clear scout cars that are far from the ordered point.
- A simplified one-waypoint path should still work on open ground.
- Repathing should still happen when the next route is statically blocked by a newly built structure.

## Tests

- Open-ground far move remains smooth and reaches the destination tolerance.
- Far behind goal still causes a broad turn, not an in-place pivot.
- Nearby behind goal still reverses and settles.
- Lateral final-goal miss settles instead of oscillating.
- Lateral intermediate-waypoint miss advances to the next segment.
- Static segment checks prevent lookahead from aiming through a building footprint.

## Done When

- Scout cars no longer need to exactly hit every waypoint to make progress.
- They still visibly behave like cars: bounded curvature, no normal in-place rotation, and reverse
  only when appropriate.
- Existing tank movement behavior is unchanged except for shared helper refactors.
