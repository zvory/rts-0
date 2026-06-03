# Phase 1 - Distance-Sensitive Formation Goals

Goal: make group move destinations depend on order distance. Close clicks should compact the group;
far clicks should roughly preserve the selected units' starting shape on arrival.

This phase changes where each unit paths to. It does not change pathfinding, movement physics, or
wire commands.

## Scope

In scope:

- Update `MoveCoordinator::order_group_move`.
- Replace or extend `spread_goals` with formation-aware goal assignment.
- Keep one goal per valid selected unit.
- Keep deterministic assignment in the input id order after command validation.
- Add Rust tests for near, medium, and far orders.

Out of scope:

- No marching formation while moving.
- No rotated formation offsets.
- No persistent slot reservations.
- No protocol or client changes.
- No changes to attack, gather, build, or spawn-point pathing.

## Files To Touch

- `server/src/game/services/move_coordinator.rs`
- `DESIGN.md` if the movement hardening or command semantics text needs a short update.

## Algorithm

Use this model:

```text
centroid = average selected unit position
offset_i = unit_i.position - centroid
move_distance = distance(centroid, clicked_point)
formation_scale = smoothstep(near_threshold, far_threshold, move_distance)
desired_goal_i = clicked_point + offset_i * formation_scale
```

Use world orientation for offsets. Do not rotate them toward movement direction in this phase.

Suggested constants in `move_coordinator.rs`:

```rust
const FORMATION_NEAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 4.0;
const FORMATION_FAR_DISTANCE_PX: f32 = config::TILE_SIZE as f32 * 18.0;
const FORMATION_MAX_OFFSET_PX: f32 = config::TILE_SIZE as f32 * 10.0;
```

The max offset prevents huge scattered selections from requesting unreachable or absurdly wide
arrival layouts.

## Implementation Steps

1. In `order_group_move`, build a local list of valid movable units with their current positions.
   The command service already filters ownership, but keep the local checks because direct tests
   call `order_group_move`.

2. If the valid list is empty, return.

3. For one valid unit, keep current behavior: path to the clicked point, snapped through the normal
   passable-goal fallback.

4. For multiple units, compute centroid from current positions.

5. Compute `formation_scale`:

   ```text
   t = clamp((distance - near) / (far - near), 0, 1)
   smooth = t * t * (3 - 2 * t)
   ```

6. For each valid unit in deterministic order:

   - Compute offset from centroid.
   - Clamp offset length to `FORMATION_MAX_OFFSET_PX`.
   - Compute desired goal.
   - Clamp desired goal to map world bounds.
   - Convert desired goal to a tile.
   - Find a unique nearby passable tile using the existing ring search idea.
   - Store that tile as taken.
   - Use the selected tile center as the path goal.

7. Keep the current fallback behavior: if no unique passable tile is found in the search radius,
   use the desired anchor tile even if occupied. Do not panic or drop the unit.

8. Preserve the current order side effects:

   - `entities.release_miner(id)`
   - set `Order::Move` or `Order::AttackMove`
   - clear target id
   - clear path
   - set `path_goal`
   - mark `MovePhase::AwaitingPath`
   - reset gather state
   - begin machine-gunner teardown
   - reset stuck state

9. Keep `spread_goals` testable. Either update its signature to accept positions, or introduce a
   new pure helper such as `formation_goals(...)` and leave `find_unique_tile_near` intact.

## Tests

Add Rust tests in `server/src/game/services/move_coordinator.rs`:

- `near_group_move_compacts_goals_near_click`: four spaced units ordered near their centroid get
  goals clustered around the clicked point.
- `far_group_move_preserves_world_offsets`: four spaced units ordered far away get goals whose
  relative offsets roughly match their starting offsets.
- `medium_group_move_blends_offsets`: medium-distance order produces offsets between compact and
  full preservation.
- `formation_goals_are_unique_when_tiles_are_free`: no duplicate goal tiles for a normal group.
- `blocked_formation_slot_falls_back_to_nearby_passable_tile`: a blocked desired tile searches
  outward deterministically.
- Existing `goal_spreading_assigns_unique_tiles_deterministically` should be updated or replaced,
  not deleted without equivalent coverage.

Run:

```bash
cd server && cargo fmt && cargo test move_coordinator::tests
cd server && cargo test
```

## Acceptance Criteria

- Single-unit move behavior is unchanged.
- Close group move orders bunch around the clicked point.
- Far group move orders preserve rough starting shape on arrival goals.
- Attack-move uses the same formation goal assignment as move.
- Group goal assignment is deterministic for the same ids, positions, map, and click.
- Path request budgeting is unchanged.
- No protocol or client files change in this phase.
