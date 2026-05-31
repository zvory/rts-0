# Pathing Plan 2 — Line-of-Sight Waypoint Skipping

**Assumes Plan 1 (`PATHING-ARRIVAL-PLAN.md`) is implemented and merged.**

## Problem

Even with loosened intermediate arrival, units still *aim at* tile centers
because that is what `pathfinding::to_world_waypoints` emits. Consequences:

- Paths bend at 45° increments through the centers of every tile the A* visits,
  even when a straight line would be unobstructed.
- Two units whose A* paths share a long straight corridor of tile centers will
  still converge on the same line of points; Plan 1 keeps them from wedging,
  but they still funnel through identical interior centers.
- Tile-center steering produces visibly staircase movement in open terrain,
  which is the cosmetic complaint behind the original request.

The fix is execution-time path smoothing: at each tick, look ahead along the
remaining waypoints and steer at the **furthest waypoint reachable by a
straight line on passable terrain**. Intermediate waypoints between the unit
and that target are dropped.

This is funnel/string-pulling applied at the steering layer, not the planning
layer. The A* output is unchanged; the smoother decides which of those
waypoints actually need to be visited.

## Design

### Per-tick lookahead

At the top of the waypoint-consume loop in `movement_system`
(server/src/game/services/movement.rs:60), before deciding the next waypoint:

1. Let `path` be the unit's remaining waypoints in visit order
   (reverse of `entity.movement.path`).
2. Walk `k` from `min(path.len() - 1, MAX_LOS_LOOKAHEAD)` down to `1`.
3. For each candidate `k`, run `segment_passable(pos, path[k], class)`.
4. On the first `k` that passes, drop waypoints `path[0..k]` (i.e. pop them off
   the entity's reverse-ordered `path` vec) and proceed with `path[k]` as the
   next waypoint.
5. If no `k > 0` passes, leave the path alone.

The final waypoint (path[len-1]) is always a candidate, so a clear line of
sight to the goal collapses the entire remaining path in one step.

### `segment_passable`

A new helper in `server/src/game/pathfinding.rs` or `services/movement.rs`:

```rust
/// Whether the open segment from (x0, y0) to (x1, y1) crosses only tiles that
/// are passable for `class` (terrain + building footprint). Used by the
/// any-angle smoother to decide whether a waypoint can be skipped.
fn segment_passable(
    map: &Map,
    occ: &Occupancy,
    class: MobilityClass,
    x0: f32, y0: f32,
    x1: f32, y1: f32,
) -> bool;
```

Implementation: amanatides-woo voxel traversal (a.k.a. tile-DDA) over the tile
grid. Visit every tile the segment enters, return false on the first impassable
one. Cost is `O(|dx_tiles| + |dy_tiles|)`, which for `MAX_LOS_LOOKAHEAD = 8`
waypoints over an 8-tile span is ~16 tile checks per unit per tick — cheap.

Implementation notes:

- Use the same passability rule as the pathfinder: `map.is_passable_for(class, tx, ty) && occ.passable(tx, ty)`. Static obstacles only — do NOT consult unit
  positions. We are not trying to predict where other units will be; we are
  asking "is this geometric route clear of terrain and buildings."
- No diagonal corner-cutting check. The pathfinder forbids cutting between two
  blocked tiles (pathfinding.rs:148); the smoother must apply the same rule. If
  a tile-DDA step transitions diagonally (both x and y advance in the same
  step), require both orthogonal neighbors to also be passable.
- Use `f32`; no need for sub-pixel precision.
- All arithmetic must be panic-free (no division by zero when `x0 == x1` and
  `y0 == y1`; treat as trivially passable).

### Constants

Add to `server/src/config.rs`:

```rust
/// Maximum number of remaining waypoints the steering smoother looks ahead for
/// a clear straight-line shortcut. Bounds the per-unit per-tick cost of the
/// line-of-sight check; 8 covers ~half a screen on the current map scale.
pub const MAX_LOS_LOOKAHEAD: usize = 8;
```

### Interaction with Plan 1

The Plan-1 arrival predicate still governs *when* the current next waypoint is
consumed. Plan 2 only changes *which* waypoint is "current." Both together:

- Smoother picks the furthest LOS-reachable waypoint as `next`.
- Arrival predicate pops it when the unit is within the intermediate radius
  *or* has passed it.

This stacks correctly: smoothing reduces the number of waypoints the arrival
predicate has to chew through, and arrival reduces the number of ticks each
remaining waypoint takes.

### Interaction with sidestep

Sidestep injects a perpendicular waypoint at the *front* of the path
(`push_waypoint` at movement.rs:281). The smoother must not skip past a freshly
injected sidestep — it is intentionally short-range and off the original line.

Two acceptable approaches:

- **(A) Skip-budget reset.** When a sidestep waypoint is injected, mark it
  (new bool field on `MovementState` or a sentinel `f32::NAN` flag — pick
  whichever fits the struct cleanly). The smoother starts lookahead from index
  1, not 0, when the head is a sidestep marker. Pop the marker when the
  sidestep waypoint is popped.
- **(B) Lookahead from index 0 but treat the first waypoint as required.** I.e.
  the smoother considers `k` in `1..=lookahead_max`, not `0..=lookahead_max`,
  meaning it always commits to the very next waypoint and only looks for
  shortcuts among waypoints 2+. Simpler; loses one tile of smoothing at the
  head of the path.

Pick (B) unless profiling shows the lost smoothing matters. Document the
choice in the implementation.

### Determinism

Replay correctness requires identical paths from identical inputs.
`segment_passable` is a pure function of map/occupancy and the two endpoints,
and the lookahead loop iterates in fixed order. Determinism is preserved.

## Edge cases to verify

- **Unit on a diagonal A* path through open terrain.** Expect: collapses to a
  single straight segment from start to goal.
- **Unit pathing around a corner of impassable terrain.** Expect: collapses
  to a two-segment path bending at the corner tile, not staircase.
- **Sidestep injected mid-traverse.** Expect (option B): sidestep waypoint is
  reached before the smoother considers shortcuts again.
- **Path becomes empty after smoothing.** Cannot happen — the final waypoint is
  always retained.
- **Goal sits on the edge of a passable tile next to an impassable one.** The
  smoother's straight line must not clip through the impassable neighbor. The
  diagonal-corner rule above handles this; verify with a test.
- **Very long paths (50+ waypoints).** Bounded by `MAX_LOS_LOOKAHEAD`; per-tick
  cost stays flat. Long paths get smoothed incrementally, not all at once.

## Tests

In `services/movement.rs` `#[cfg(test)] mod tests`:

1. **`los_collapses_straight_path_in_open_terrain`** — flat map, 10-tile
   diagonal path; after one tick of smoothing the path has 1 remaining
   waypoint (the goal).
2. **`los_preserves_path_around_obstacle`** — map with a wall between start
   and goal; after smoothing the path has ≥ 2 remaining waypoints and every
   remaining segment is `segment_passable`.
3. **`los_does_not_cut_diagonal_through_two_blocked_tiles`** — explicit
   regression for the corner-cutting rule.
4. **`group_move_open_terrain_does_not_converge_to_centerline`** — two units
   given paths whose A* outputs share a long stretch of tile centers; after
   smoothing, their actual steering targets diverge (the furthest LOS-reachable
   waypoint differs because their starting positions differ).
5. **`sidestep_waypoint_not_smoothed_away`** — inject a sidestep; verify the
   smoother does not skip past it (option B behaviour).

In `server/src/game/pathfinding.rs` (or wherever `segment_passable` lives),
unit tests for the DDA traversal itself:

6. **`segment_passable_horizontal_clear`**, **`segment_passable_vertical_clear`**,
   **`segment_passable_diagonal_clear`** — sanity.
7. **`segment_passable_rejects_traversed_blocker`** — segment that clips a
   blocked tile returns false.
8. **`segment_passable_zero_length`** — start == end returns true.

Re-run regression and self-play tests; movement quality should improve, no
regressions in arrival behaviour.

## Performance

Worst case: every unit, every tick, runs up to `MAX_LOS_LOOKAHEAD` (8)
`segment_passable` calls. Each call traverses up to ~`8 * sqrt(2) ≈ 11` tiles
of DDA. At 30 Hz with, say, 200 units, that is `200 * 30 * 8 * 11 ≈ 530k`
tile-passability lookups per second. Each lookup is a couple of array reads.
Negligible relative to the existing A* and collision-resolution budgets.

If profiling later shows it matters, add a cheap early-exit: only re-run the
smoother every K ticks per unit, or when the unit has actually moved more than
N pixels since the last smooth.

## Files touched

- `server/src/config.rs` — add `MAX_LOS_LOOKAHEAD`.
- `server/src/game/pathfinding.rs` — add `segment_passable` + unit tests.
- `server/src/game/services/movement.rs` — integrate smoother in
  `movement_system`; new tests.
- `DESIGN.md` — document the smoothing pass in the movement section.

## Out of scope

- Plan-time smoothing (rewriting A* to Theta*). Execution-time smoothing gives
  most of the win at a fraction of the complexity and replaces no existing code.
- Cooperative pathfinding / reservation tables.
- Goal-slot formation assignment.
