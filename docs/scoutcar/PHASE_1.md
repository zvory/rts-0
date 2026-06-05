# Phase 1 - Scout Car Movement Contract

## Goal

Define scout car movement semantics before changing code. The immediate problem is not only
front-on building stalls. Scout cars currently inherit point-waypoint expectations from infantry,
but their own locomotion cannot move sideways or pivot in place. This phase turns that observation
into explicit rules and fixtures.

## Problem Statement

Scout cars can get stuck in two related ways:

- A traffic push or route shape puts the front of the oriented body against a building. The current
  movement code then suppresses facing changes when the car makes no progress, so it cannot turn out.
- A scout car ends up beside a waypoint or final goal. The desired vector points mostly sideways, but
  the car can only translate along its current facing with bounded curvature, so it oscillates instead
  of satisfying exact point arrival.

Both are symptoms of asking a nonholonomic vehicle to hit exact points.

## Contract

1. Scout cars should follow a route corridor, not exact intermediate waypoint points.
2. Intermediate waypoints may be consumed when the car passes them, enters a scout-car-specific
   acceptance radius, or reaches a position from which the next segment is statically reachable.
3. Final move goals may use a small scout-car-specific tolerance when exact arrival would require
   lateral motion the car cannot perform.
4. Scout cars must not rotate in place as normal locomotion.
5. Scout cars may reverse as normal locomotion only for nearby behind goals or as an explicit recovery
   maneuver after repeated lack of progress.
6. Scout cars must never rotate or move into a statically illegal oriented body position.
7. The behavior must remain server-authoritative, deterministic, and replay-stable.

## Implementation Notes

- Keep the wire protocol unchanged.
- Keep the existing `facing` field as the body orientation.
- Prefer constants near existing movement constants:
  - scout car waypoint acceptance radius;
  - scout car final goal tolerance;
  - scout car stuck recovery trigger;
  - scout car reverse recovery distance;
  - scout car recovery cooldown.
- Document the final semantics in `DESIGN.md` once the implementation is selected.

## Tests To Add First

- Scout car beside an intermediate waypoint consumes or bypasses it instead of oscillating.
- Scout car beside a final goal settles within the accepted final tolerance and clears the order.
- Scout car front-blocked by a building does not rotate illegally into the footprint.
- Scout car front-blocked by a building remains able to recover without clearing the player's move
  order.
- Existing test `scout_car_locomotion_suppresses_illegal_rotation_when_blocked` remains meaningful:
  it should prevent illegal in-place rotation, not prevent legal recovery movement.

## Done When

- The movement contract is written clearly enough that implementation choices can be evaluated
  against it.
- Regression fixtures cover both broad failure modes: nose-on-static-obstacle and lateral waypoint
  miss.
- Any expected player-facing impact is captured as patch-note bullets for the final change.
