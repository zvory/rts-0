# Phase 3 - Recovery, Traffic, and Verification

## Goal

Add bounded recovery behavior for scout cars that are blocked by static geometry or traffic after
Phase 2's path-following changes. Recovery should be rare, deterministic, and visibly car-like.

## Recovery Behavior

When a scout car on `Move` or `AttackMove` repeatedly makes little progress while far from its
`path_goal`, run a recovery check before giving up or repathing:

1. Confirm the car is genuinely stuck:
   - `stuck_ticks` is above a scout-car-specific threshold;
   - the car is outside final goal tolerance;
   - the current order is still movement-oriented;
   - the car is in a statically legal body position.
2. Search backward along the current hull direction for a legal recovery point:
   - try increasing distances from about one body length up to a few tiles;
   - require `unit_static_standable` at the candidate;
   - require the segment from current position to candidate to be statically standable;
   - reject non-finite coordinates and out-of-bounds candidates.
3. Push the recovery point as the next waypoint with the existing reverse-ordered path storage.
4. Reset or cool down the stuck counter so the car does not inject recovery points every tick.
5. After the reverse waypoint is consumed, resume the original path.

## Traffic Interaction

Recovery is not a replacement for traffic avoidance. Keep the existing traffic throttle and turn bias,
but use recovery only when the car cannot make progress through normal steering.

Watch for these cases:

- car nose pressed into a building by other units;
- car boxed between infantry and a building edge;
- multiple scout cars trying to pass through a narrow opening;
- car collision displacement leaving the car beside the route.

## Implementation Notes

- Avoid adding new network fields.
- Avoid generic infantry sidestep injection for scout cars.
- Avoid tank-style pivot behavior.
- Keep recovery purely local to the movement system so player commands and replays remain clean.
- Prefer a small helper such as `inject_scout_car_reverse_recovery` over spreading recovery decisions
  through the waypoint loop.

## Tests

- Building nose-stuck fixture:
  - scout car starts legal with its front near a factory footprint;
  - normal forward step is blocked;
  - after the trigger threshold, a reverse recovery waypoint is inserted;
  - the car backs away without rotating into the building.
- Traffic shove fixture:
  - another unit blocks the scout car's front near a building;
  - the car eventually backs out instead of jiggling indefinitely.
- Recovery cooldown fixture:
  - stuck recovery does not add unbounded duplicate waypoints.
- Resume fixture:
  - after reversing out, the scout car continues toward the original far goal.
- Determinism fixture:
  - repeated runs with the same setup produce the same final position and path state.

## Verification

- Run `cd server && cargo test` for movement and simulation coverage.
- If client-visible behavior changes are substantial, run a local server and inspect a small scenario
  with scout cars ordered around factories and narrow base traffic.
- Update `DESIGN.md` with the final scout-car path-following and recovery semantics.

## Done When

- Scout cars recover from front-blocked static geometry without gaining tank pivot movement.
- Scout cars beside waypoints or final goals do not jiggle indefinitely.
- Recovery remains bounded, deterministic, and legal under oriented body standability.
- Patch notes explain the player-facing movement change without overstating tactical impact.
