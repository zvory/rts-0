# Phase 3 - Tank Route Follower and Long Lookahead

Goal: make tank hull facing and movement intent use the same smoothed route segment, with a longer
lookahead than the tank body.

Phase 2 removes most tile-center noise. This phase makes tanks take advantage of that cleaner route.

## Current Behavior to Inspect

In `server/src/game/services/movement.rs`:

- `TANK_BODY_LOOKAHEAD_PX` is currently a short constant.
- `tank_desired_path_point` scans waypoints and returns a point at least that far away.
- The tank rotates hull facing toward that desired point.
- `tank_speed_scale` slows or stops the tank based on the hull error.
- Actual movement still steps toward the next waypoint in the path loop.

## Desired Behavior

For tanks:

- Use a longer route lookahead, for example 4-6 tiles, after Phase 2 smoothing is in place.
- Choose a desired point only along the current legal route segment.
- Do not let local steering or collision pushes redefine hull intent every tick.
- If the next segment requires a sharp turn, slow or pivot before entering the turn instead of
  visually sliding sideways through it.

## Implementation Options

Prefer the smallest option that works with the current path representation.

### Option A - Longer Lookahead Over Smoothed Waypoints

Increase tank lookahead only after Phase 2 smoothing exists. Keep `tank_desired_path_point` scanning
the smoothed path, but tune the distance upward.

Pros:

- Small change.
- Keeps current movement loop.

Cons:

- If a path still contains a sharp corner, the tank may face too far around the bend.

### Option B - Segment-Bounded Lookahead

Track or infer the current segment from current position to the next smoothed waypoint. The desired
facing point must lie on that segment unless a farther waypoint is reachable by a legal static
segment.

Pros:

- Prevents facing through corners.
- Better match between movement and hull intent.

Cons:

- More code than Option A.

### Option C - Consume Reachable Waypoints During Movement

Before the movement loop steps toward `next_waypoint`, pop intermediate waypoints that are still
reachable by a static segment from the current tank position. This makes actual movement follow the
same route simplification dynamically.

Pros:

- Robust when collision pushes a tank slightly off the planned line.

Cons:

- Needs careful tests to avoid popping required corners after a push.

## Recommended First Implementation

Implement Option B if the Phase 2 simplifier is reliable. Use Option A only as a small tuning patch
after verifying that smoothed paths already have few corners.

## Tests

Add tests for:

- A tank on a long open segment faces the long-route direction, not each old tile-center step.
- A tank approaching an obstacle corner does not face through the blocked corner.
- A badly misaligned tank still pivots/slows rather than sliding at full speed.
- Tank `facing` remains finite after movement.
- Non-tank unit facing behavior is unchanged.

## Acceptance Criteria

- Tank hull facing is less noisy because route intent is smoother, not because the client hides it.
- Tank armor-facing semantics remain authoritative and visible.
- Tanks do not drive through static blockers.
- Infantry movement/facing is not regressed.
- `DESIGN.md` documents the new tank route-following rule if behavior changes materially.

## Common Mistakes

- Increasing lookahead before path simplification. That can make tanks face across corners while
  still moving tile-by-tile.
- Deriving hull facing from collision displacement. Collision can be sideways and should not become
  strategic tank intent.
- Making turret facing or firing rules depend on movement lookahead.

