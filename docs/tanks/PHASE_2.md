# Phase 2 - Path Following, Reverse, Pivot, and Stuck Behavior

## Goal

Teach the tank controller how to follow coarse A* routes without chasing every tile center like a
hovercraft. This phase keeps tile A* but changes how tanks consume and interpret waypoints.

## Steps

1. Keep the existing route lookahead idea, but feed it into the tank controller as desired driving
   intent rather than direct movement direction.
2. Add close-goal behavior:
   - reverse when the goal is behind and nearby;
   - pivot when the goal is behind and farther away;
   - avoid turning a full loop for tiny corrections.
3. Add corner behavior:
   - slow before sharp turns;
   - avoid overshooting simplified path corners;
   - do not pop a waypoint just because the circular center passed it if the hull cannot make the
     next segment cleanly.
4. Rework stuck handling for tanks:
   - distinguish "blocked by static obstacle" from "waiting to rotate";
   - avoid sidestep injection for tanks unless it becomes a controlled reverse/pivot maneuver;
   - repath only when the coarse route is truly stale.
5. Audit `MoveCoordinator` path smoothing for tank orders. Keep A* and simplification if useful, but
   stop relying on turn-cost A* as the main source of believable turning.
6. Add command-level tests for long movement, close retargeting, attack-move while firing, and
   blocked/static-repath cases.

## Plain-Language Explanation

A* can say "go around that building," but it should not tell a tank to wiggle through every tile
center. This phase makes the tank look farther along the route, slow for corners, reverse when that
is the natural move, and avoid treating ordinary rotation time as being stuck.

## Expected Code Touches

- `server/src/game/services/movement.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/pathing.rs`
- `server/src/game/pathfinding.rs` only if route-shaping options are simplified or removed
- `DESIGN.md` for any changed tank path-following contract

## Refactor Depth

Medium. This is still inside existing movement/pathing seams. It becomes deep only if the phase
tries to replace A* or implement body-aware route clearance before Phase 3.

## Done When

- Tanks follow long paths with fewer twitchy heading changes.
- Tanks handle close behind-target orders without absurd spins.
- Stuck/repath logic does not fight normal pivoting or braking.
- The tank still reaches legal goals through known corridor and corner fixtures.

