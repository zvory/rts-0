# Pathing Plan 1 — Loosen Intermediate-Waypoint Arrival

## Problem

`movement_system` (server/src/game/services/movement.rs:35) only consumes a
waypoint when one of two predicates fires:

1. `dist <= ARRIVE_EPS` (2 px) — "I am on the center."
2. `dist <= budget` — "I can reach the center this tick."

Waypoints emitted by `pathfinding::to_world_waypoints`
(server/src/game/pathfinding.rs:188) are **tile centers**. When two units' paths
share an intermediate tile, both lock onto the same point. Collision resolution
(`resolve_collisions`) keeps them apart by a unit diameter, so neither can satisfy
predicate (1). Predicate (2) also fails because the collision push consumes the
budget each tick. The pair is wedged at the tile boundary; the path never advances;
a third unit on the same path joins the wedge; cascade.

Sidestepping (movement.rs:217) is a patch over the symptom but doesn't fire until
`SIDESTEP_TRIGGER_TICKS` (15) and frequently sidesteps *back into the same
contested center* because the predicate has not changed.

The root cause is the arrival predicate, not the path geometry. Intermediate
waypoints are routing hints, not destinations — being "near" one or "past" one
is just as good as being on it.

## Fix

Treat intermediate waypoints as fly-by, the final waypoint as the destination.

### Predicate change

In the waypoint-consume loop in `movement_system`:

- **Intermediate waypoint** (path length > 1 after pop, i.e. there is a next-next
  waypoint): pop when **either**
  - `dist <= ARRIVE_RADIUS_INTERMEDIATE_PX`, OR
  - the unit has *passed* it — i.e. the projection of `(pos - waypoint)` onto
    `(next_next_waypoint - waypoint)` is positive. (The pass-by check handles
    the case where a unit is shoved sideways but is geometrically beyond the
    waypoint relative to the next leg.)
- **Final waypoint** (only one left in path): keep current predicate
  (`dist <= ARRIVE_EPS` or reachable this tick). The existing tolerant-arrival
  fallback at movement.rs:171 already handles the "close to goal but stuck"
  case for the final point.

### Constants

Add to `server/src/config.rs`:

```rust
/// Radius within which an *intermediate* waypoint is considered reached. Tile
/// centers are routing hints, not destinations; brushing past one satisfies the
/// route. Must be ≥ largest unit radius + a small slack so two units cannot both
/// be simultaneously locked onto the same waypoint center.
pub const ARRIVE_RADIUS_INTERMEDIATE_PX: f32 = TILE_SIZE as f32 * 0.5; // 16 px
```

16 px is half a tile and is comfortably larger than the largest unit radius
(tank = 26 px diameter, radius 13). A unit that enters the inner-half of a
waypoint's tile counts as having visited it. We keep this strictly less than
`TILE_SIZE` so the predicate cannot fire for a waypoint a full tile away.

### Position update when popping by radius

Currently when `dist <= ARRIVE_EPS` the code snaps `(x, y)` to the waypoint and
continues with the remaining budget. With a 16 px radius we must NOT snap —
snapping would teleport the unit up to 16 px sideways. Instead:

- Do not modify `(x, y)` when popping an intermediate waypoint by the radius/pass
  predicate. Just pop and re-enter the loop; the next iteration will steer toward
  the new next waypoint from the unit's current position using its remaining budget.
- Keep the existing snap behaviour for the `dist <= budget` branch (we genuinely
  reach the waypoint exactly that tick) and for the final waypoint's `ARRIVE_EPS`
  case.

### Sidestep interaction

With this change the sidestep injector (movement.rs:217) will fire much less often
because intermediate-waypoint wedges resolve themselves. Leave the injector in
place — it still earns its keep when a unit is stuck mid-corridor against terrain
or another anchored unit. No constant changes needed.

### Stuck-tracking interaction

The `stuck_ticks` / `last_progress_pos` accounting at movement.rs:152 measures
*displacement*, not waypoint progress, so it remains correct under this change.
A unit that smoothly transits a contested center now actually moves, so
`stuck_ticks` correctly stays low.

## Edge cases to verify

- **Two waypoints both within radius simultaneously.** With 16 px radius and
  ~45 px tile diagonals, this can happen at corners. The loop already iterates
  while waypoints are consumable; it will pop both in one tick. Correct
  behaviour — the unit is past both.
- **Final waypoint within radius but not within `ARRIVE_EPS`.** Falls through to
  the existing tolerant-arrival check or the partial-step branch. No regression
  vs. today.
- **Single-waypoint path.** `path.len() == 1` from the start; treated as final;
  current behaviour preserved.
- **Path replanned mid-traversal.** Replanning replaces the whole path
  (entity.rs `set_path`); predicate sees a fresh path and behaves correctly.

## Tests

Add to `services/movement.rs` `#[cfg(test)] mod tests`:

1. **`intermediate_waypoint_consumed_by_radius`** — single unit, two-waypoint
   path with intermediate at tile center; place the unit 10 px off the
   intermediate center on its inbound side; tick once; assert the intermediate
   was popped and the unit is now steering at the final waypoint.
2. **`two_units_sharing_waypoint_do_not_wedge`** — spawn two units a few tiles
   apart, give both the same multi-tile path through a shared intermediate
   tile, run 60 ticks, assert both have reached the goal (no stuck-ticks
   saturation, no sidestep injected). This is the core regression the change
   is designed to fix.
3. **`final_waypoint_still_requires_close_arrival`** — single unit, one-waypoint
   path; tick until `path_is_empty`; assert the final position is within
   `ARRIVE_EPS` of the waypoint OR tolerant-arrival fired (i.e. behaviour
   matches today).
4. **`pass_by_waypoint_pops_when_overshooting_sideways`** — unit positioned just
   past the intermediate waypoint relative to the next leg's direction but
   `> ARRIVE_RADIUS_INTERMEDIATE_PX` away (e.g. shoved sideways by collision);
   tick once; assert the intermediate was popped.

Also re-run the existing `clustered_units_make_progress_to_distant_goal` and
`group_move_to_one_point_settles_without_overlap` — they should pass with
fewer ticks needed.

## Files touched

- `server/src/config.rs` — add `ARRIVE_RADIUS_INTERMEDIATE_PX`.
- `server/src/game/services/movement.rs` — change waypoint-consume loop in
  `movement_system`; new tests.
- `DESIGN.md` — note the arrival predicate in the movement-system section if
  it is documented there. Verify before editing.

## Out of scope

- Path geometry / smoothing / any-angle (Plan 2).
- Goal-slot assignment for terminal pile-ups (different problem; user reports
  no symptom).
- Removing the sidestep injector (still useful as a fallback).
