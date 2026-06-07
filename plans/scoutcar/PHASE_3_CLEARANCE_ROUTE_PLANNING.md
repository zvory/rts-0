# Phase 3 - Clearance-Aware Route Planning

## Goal

Make scout-car global route planning prefer safer corridors instead of shortest legal tile paths
that skim walls and building corners.

## Rationale

The current coarse A* route can tell the car to drive along obstacle boundaries. The local movement
layer then inherits a bad corridor and spends its time suppressing illegal poses or recovering after
contact. A better global route reduces wall ramming before local planning starts.

## Scope

- Add a scout-car route shape distinct from `Normal` and `PreferFewerTurns`.
- Include clearance cost, turn cost, and corner/pinch penalties in scout-car A*.
- Preserve exact goal snapping where appropriate, but keep final goal legality checked by the
  scout-car body.
- Keep infantry and tank route behavior unchanged unless explicitly opted in.

## Route Cost Inputs

- base movement cost;
- direction-change cost;
- static clearance penalty;
- diagonal pinch / near-corner penalty;
- optional small penalty for paths whose next segment points directly into a nearby blocker.

## Narrow Passage Rule

Clearance cost should prefer the center of open space but must not reject intended chokepoints when
there is no wider route. The route can accept lower clearance when all nearby alternatives are also
low-clearance, similar in spirit to Voronoi-style clearance fields used in vehicle planning.

## Code Areas

- `server/src/game/services/pathing.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/movement/tests.rs`
- `docs/design/server-sim.md` after behavior is implemented

## Tests

- In a wide route around a building, scout-car A* selects a path with larger minimum clearance than
  shortest-path A*.
- In a narrow legal lane, scout-car A* still finds a route.
- The selected path avoids known corner-graze tiles when alternatives exist.
- Cached path keys distinguish scout-car clearance routing from existing route shapes.
- Existing tank and infantry path tests remain unchanged.

## Done When

- Scout-car routes are visibly less wall-hugging in static fixtures.
- Route choice reduces blocked static step attempts before any local recovery logic is involved.
