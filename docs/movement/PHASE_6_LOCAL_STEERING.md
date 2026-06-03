# Phase 6 - Local Steering

Goal: add a short-range steering layer so moving units try to avoid solid nearby units before hard
collision overlap resolves them.

Only start this phase after observing remaining avoidable movement stupidity from the earlier
phases. Do not use this phase to replace A* or create polished crowd flow.

## Dependencies

- Phase 0 must be complete. Steering needs footing profiles or equivalent resistance data.
- Phase 1 should be complete so final goals are less clumpy.
- `docs/collision` Phases 0-3 should be complete, or this phase must integrate their shared
  standability helpers as part of the same change.

## Scope

In scope:

- Add local steering for units with active paths.
- Steer away from nearby non-ghost units, weighted by footing/resistance.
- Keep steering bounded by spatial-index neighbor queries.
- Keep shared standability checks authoritative for static terrain/building body legality.
- Add tests for avoidance, choke behavior, and determinism.

Out of scope:

- No flow fields.
- No ORCA/RVO.
- No global dynamic obstacle cost maps.
- No pathfinding replacement.
- No steering for workers latched to resources or construction.
- No formation marching.

## Files To Touch

- `server/src/game/services/movement.rs`
- Possibly `server/src/game/services/mod.rs` if extracting a small `locomotion` helper module.
- `DESIGN.md` if the hardening/movement description changes.

## Implementation Model

For each moving unit:

```text
path_dir = normalized vector toward lookahead/next waypoint
separation = sum over nearby solid units of away_dir * weight
desired_dir = normalize(path_dir + separation * steer_strength)
step = desired_dir * speed_budget
if step landing passes unit_static_standable: use it
else: fall back to existing path step / wall-slide behavior
```

This is steering, not path planning. It should help with local avoidance but still allow chokes,
jams, and imperfect motion.

## Implementation Steps

1. Expose or reuse Phase 0 footing helpers. If they are still private inside collision code, either
   move them to a small local section in `movement.rs` or extract a `services::locomotion` module.
   Keep the helper deterministic and pure.
   The implemented stack keeps the footing helpers in `movement.rs`, shared by steering and
   collision, and uses `services::standability::unit_static_standable` for every steering
   candidate landing check.

2. Add steering constants in `movement.rs`:

   ```rust
   const STEERING_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 1.5;
   const STEERING_RADIUS_PX: f32 = config::TILE_SIZE as f32 * 2.5;
   const STEERING_STRENGTH: f32 = 0.65;
   const STEERING_MAX_NEIGHBORS: usize = 16;
   ```

3. Add a helper:

   ```rust
   fn local_steering_dir(
       entities: &EntityStore,
       spatial: &SpatialIndex,
       id: u32,
       x: f32,
       y: f32,
       path_dir: (f32, f32),
   ) -> (f32, f32)
   ```

4. Query neighbors from the spatial index. Sort or otherwise ensure deterministic order before
   applying a neighbor cap.

5. Ignore:

   - self
   - non-units
   - ghost footing
   - dead entities
   - neighbors outside `STEERING_RADIUS_PX`

6. Weight separation by:

   - stronger when overlap or near-overlap is high,
   - stronger for `Firm`, `Braced`, and `Heavy` neighbors,
   - weaker for `Soft` idle units.

7. Integrate steering into `movement_system` only for normal path-following movement. Keep current
   static-obstacle repath, tolerant arrival, sidestep, and final waypoint behavior.

8. If the steered landing fails `unit_static_standable`, fall back to the current path step /
   wall-slide logic. Do not let steering move through buildings or impassable terrain, including
   cases where only a large unit's body, not its center tile, would clip static geometry.

9. Keep collision resolution after movement. Steering reduces bad dynamic unit overlaps; shared
   standability remains the static legality authority, and collision remains the deterministic
   unit-unit cleanup pass.

## Tests

Add Rust tests:

- `moving_unit_steers_around_braced_unit_when_space_exists`: pathing target is beyond a deployed MG;
  moving rifleman gains lateral displacement instead of driving straight into it.
- `choke_still_clogs_when_no_space_exists`: steering does not tunnel through buildings or terrain.
- `steering_ignores_ghost_harvester`: harvesting worker does not create avoidance.
- `steering_candidate_rejected_when_body_would_clip_building`: body-aware standability blocks a
  steered candidate even if the center tile is passable.
- `steering_neighbor_cap_is_deterministic`: same setup produces same position after repeated runs.
- Existing stuck, sidestep, and collision tests still pass.

Run:

```bash
cd server && cargo fmt && cargo test movement::tests
cd server && cargo test
```

## Acceptance Criteria

- Moving units make fewer direct overlaps with braced/heavy units when open space exists.
- Chokes and traffic jams still happen.
- Steering is bounded, deterministic, and panic-free.
- Steering never bypasses static standability.
- Collision resolution remains active and tested.
- No protocol or client files change in this phase.
