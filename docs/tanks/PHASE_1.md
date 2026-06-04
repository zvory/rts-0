# Phase 1 - Tank Locomotion Controller Inside the Current Circle-Body Model

## Goal

Replace the current tank "rotate facing, scale speed, then step toward waypoint" behavior with a
small tracked-vehicle controller while keeping existing circular standability and collision. This
phase should improve tank feel without a geometry refactor.

## Model

Use differential-track kinematics as the conceptual model:

```text
left_track_power/right_track_power -> track speeds
forward_speed = (left_track_speed + right_track_speed) / 2
turn_rate = (right_track_speed - left_track_speed) / track_width
position += hull_forward * forward_speed
facing += turn_rate
```

The implementation does not need to expose track power on the wire. It can be an internal helper
that converts route-following intent into a bounded forward/reverse speed and turn rate.

## Steps

1. Add tank locomotion fields to `MovementState` only if the existing fields are insufficient:
   current forward speed, desired forward speed, and optionally current angular velocity.
2. Add a tank controller helper in or near `services::movement` that takes current hull facing,
   current speed, route lookahead point, and terrain/occupancy constraints.
3. Limit acceleration, braking, reverse speed, and angular acceleration so tank motion changes over
   time instead of instantly.
4. Move tanks only along their hull forward axis. The controller may command forward, reverse, or
   pivot, but it must not choose an arbitrary sideways landing vector.
5. Keep oil burn based on actual distance moved so economy behavior remains tied to motion.
6. Preserve turret behavior: tank combat still uses independent `weaponFacing` and should not clear
   movement paths when firing.
7. Add unit tests for:
   - gradual acceleration;
   - gradual braking;
   - pivot while badly misaligned;
   - no lateral self-motion;
   - finite recovery from bad facing/speed values.

## Plain-Language Explanation

This is the first real gameplay-feel phase. A tank should stop behaving like a circle that can move
in any direction. It should turn its hull, build up speed, slow down, reverse, or pivot like a heavy
tracked vehicle.

## Expected Code Touches

- `server/src/game/entity.rs`
- `server/src/game/services/movement.rs`
- `server/src/config.rs` or `server/src/rules/defs.rs` for tank turn/acceleration constants if they
  become balance data
- `DESIGN.md` if the `MovementState` contract or documented tank movement behavior changes

## Refactor Depth

Medium. This phase can stay inside the current movement service, but it likely needs new
locomotion state and a clearer tank-specific helper. It should not touch protocol, fog, lobby,
commands, or client rendering beyond existing interpolation.

## Done When

- Tank self-motion follows hull direction only.
- Tanks visibly accelerate, brake, pivot, and reverse.
- Existing server tests pass or are intentionally updated for the new behavior.
- Patch notes list changed tank movement constants and expected gameplay impact.

