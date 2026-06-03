# Phase 3 - Tank Body Locomotion

Goal: make tank hull facing stable and rate-limited. Tanks should not instantly point at each path
segment or target. The existing `facing` snapshot field remains body/hull facing.

This phase is tank-first. Infantry can keep instant facing.

## Dependencies

- Phase 2 should be complete so client rendering blends the new gradual body angles.

## Scope

In scope:

- Add bounded tank body rotation on movement paths.
- Add path lookahead for desired tank body direction.
- Slow or pause tank movement when the hull is badly misaligned.
- Prevent combat from instantly snapping tank body facing toward targets.
- Add Rust tests for movement and combat-facing behavior.

Out of scope:

- No new protocol field.
- No independent turret/barrel facing yet.
- No side/rear damage yet.
- No changes to infantry movement.
- No full vehicle physics.

## Files To Touch

- `server/src/game/entity.rs`
- `server/src/game/services/movement.rs`
- `server/src/game/services/combat.rs`
- `server/src/config.rs` only if constants are shared outside one module. Otherwise keep local
  constants in the service.
- `DESIGN.md` if tank-facing semantics are documented.

## Implementation Steps

1. Keep `MovementState::facing` as the authoritative body facing sent through `EntityView.facing`.
   Do not rename it in this phase.

2. Add local helpers, preferably in `movement.rs` and reused by combat if needed:

   ```rust
   fn angle_delta(from: f32, to: f32) -> f32
   fn rotate_toward(current: f32, desired: f32, max_delta: f32) -> f32
   fn tank_speed_scale(abs_angle_error: f32) -> f32
   ```

3. Suggested local constants:

   ```rust
   const TANK_BODY_TURN_RATE_RAD_PER_TICK: f32 = 0.035;
   const TANK_BODY_LOOKAHEAD_PX: f32 = config::TILE_SIZE as f32 * 2.0;
   const TANK_CRAWL_ANGLE_RAD: f32 = 0.55;
   const TANK_PIVOT_ANGLE_RAD: f32 = 1.25;
   const TANK_BODY_FIRE_TOLERANCE_RAD: f32 = 0.30;
   ```

4. In `movement_system`, when the unit is a tank and has a path:

   - Pick a desired point from path lookahead. Walk the reversed path from next waypoint outward
     until a point at least `TANK_BODY_LOOKAHEAD_PX` from current position is found; fallback to the
     next waypoint.
   - Desired angle is `atan2(desired_y - y, desired_x - x)`.
   - Rotate current `facing` toward desired by `TANK_BODY_TURN_RATE_RAD_PER_TICK`.
   - Scale movement budget by angle error:
     - below `TANK_CRAWL_ANGLE_RAD`: full or near-full speed.
     - between crawl and pivot: interpolate down.
     - above `TANK_PIVOT_ANGLE_RAD`: no forward movement this tick.

5. For non-tank units, preserve current `new_facing = atan2(path_segment)` behavior.

6. In `combat_system`, replace the unconditional `e.set_facing(target_angle)` with tank-aware
   behavior:

   - Non-tanks still face instantly for now.
   - Tanks rotate body toward the target at the same bounded turn rate.
   - While Phase 4 is not done and the barrel is still welded to the hull, tanks should only fire
     when body angle is within `TANK_BODY_FIRE_TOLERANCE_RAD` of the target direction.

7. Ensure a stationary tank with an explicit target can keep rotating even when it has no path.

8. Keep `facing` finite. If a computed angle is not finite, skip the update for that tick.

## Tests

Add Rust tests:

- `tank_body_facing_turns_gradually_along_path`: after one movement tick toward a perpendicular
  waypoint, tank `facing` changes by no more than the turn-rate constant.
- `tank_pauses_when_body_badly_misaligned`: tank with a 180-degree desired heading rotates but does
  not move materially in the same tick.
- `rifleman_facing_remains_instant_for_path_segment`: infantry behavior is unchanged.
- `tank_combat_does_not_snap_body_to_target`: in-range target changes desired body angle gradually.
- `tank_cannot_fire_until_body_aligned_before_turrets_exist`: cooldown or target HP proves firing is
  gated by hull alignment.

Run:

```bash
cd server && cargo fmt && cargo test movement::tests combat::tests
cd server && cargo test
```

## Acceptance Criteria

- Tank hulls turn smoothly across path corners.
- Tank hulls do not snap toward combat targets.
- Tanks do not visibly slide sideways at full speed while facing far away from travel direction.
- Existing `facing` protocol remains unchanged and still means body facing.
- No client or protocol files change in this phase.
