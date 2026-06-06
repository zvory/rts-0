# Phase 5 - Swept Collision and Wall Response

## Goal

Prevent scout cars from ramming through or repeatedly into static geometry by checking motion over
the whole step and applying a bounded fallback response when no clean motion is available.

## Rationale

Final-pose legality is not enough for car movement. A step can have a legal start and end while the
body clips a corner during the turn. The planner should reject those steps. If all useful steps are
blocked, a conservative wall response can prevent hard wedging.

## Swept Collision

- Sample the capsule along each candidate primitive at fixed deterministic intervals.
- Reject candidates if any sampled pose is statically illegal.
- Treat a wall-adjacent turn-away as legal only when every sampled capsule pose remains clear; the
  capsule shape may save the old rectangular rear corner, but it must not hide real swept overlap.
- Keep sample count tied to body length/speed so faster movement does not skip collision.
- Prefer exact analytical checks only if they stay simple; robust sampling is acceptable for v1.

## Wall Response Fallback

Only after no clean candidate is available:

- identify the attempted motion and nearest static blocker direction;
- remove the component of motion into the blocker;
- allow only a small tangent displacement if that swept displacement is legal;
- never rotate the car into an illegal pose;
- never use fallback to bypass a blocked chokepoint.

This approximates "frictionless walls" without turning walls into normal navigation.

## Code Areas

- `server/src/game/services/geometry.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/services/movement/scout_car.rs`
- `server/src/game/services/movement/waypoints.rs`

## Tests

- Arc around a building corner samples intermediate poses and rejects corner clipping.
- Legal tangent fallback along a wall is accepted when it reduces overlap risk.
- Side-by-side building contact can resolve into a legal tangent/outward turn-away instead of
  reverse-recovery loops, while rejecting any sampled rear-end overlap.
- Fallback does not move through a building corner.
- A scout car touching no blocker does not use wall response.
- Repeated wall-response ticks remain bounded and eventually trigger repath/recovery if no route is
  possible.

## Done When

- Accepted scout-car motion is legal across the whole step, not just at the endpoint.
- Wall response reduces hard wedging without becoming the primary steering system.
