# Movement System Research

This is a research note for a future movement redesign. It does not change the
live protocol or simulation contracts. Any implementation that changes wire
fields, entity state, balance, or module boundaries must update `DESIGN.md` in
the same change.

## Current Model

The server is authoritative at 30 Hz. Movement is split into these pieces:

- `PathingService` runs 8-way A* over tiles and returns world-pixel tile-center
  waypoints. Terrain and building footprints block paths. Units do not block
  paths.
- `MoveCoordinator` owns path request budgeting, goal spreading, spawn-point
  search, repath throttling, and `AwaitingPath` processing.
- `movement_system` walks each unit along its waypoint list at its per-kind
  speed. Intermediate waypoints are soft radius/fly-by hints. The final waypoint
  is precise.
- `resolve_collisions` runs after movement/combat/production/construction/death
  and resolves unit overlap with symmetric 50/50 circle pushes, except anchored
  harvesting and constructing workers are skipped entirely.
- `facing` is a single angle on `MovementState`. It is updated instantly to the
  current movement vector or current target vector.
- Machine gunners have `WeaponSetup`, but setup/deployed state only affects
  movement and firing. It does not make them physically firm in collision.
- The client renders tank hull and barrel from the same `facing`. Client
  interpolation blends `x/y` only, so angle changes snap between snapshots.

The result is a useful simple system, but stationary combat units are still just
ordinary movable circles in collision, and vehicle direction is only "where the
last path segment or target was."

## External Research Takeaways

This pass looked at RTS/coordinated movement writing and crowd movement papers,
then filtered it through this game's desire for bounded, deterministic, somewhat
rough movement.

- Dave Pottinger's 1999 Game Developer articles, [Coordinated Unit Movement][pottinger-1]
  and [Implementing Coordinated Movement][pottinger-2], are the closest fit for
  this problem. The main takeaway is that pathfinding is only half the system:
  executing the path, resolving collision, and coordinating groups are equally
  important. Pottinger also draws a useful distinction between a loose `Group`
  and a `Formation`: a group mostly wants to stay together, while a formation has
  an orientation and per-unit relative positions.
- Pottinger describes several cohesion levels: same speed, same path, and same
  arrival time. This game probably wants a lighter version: preserve arrival
  shape on long orders, but do not force same-speed/same-arrival marching unless
  a future explicit formation command needs it.
- Craig Reynolds' [Steering Behaviors for Autonomous Characters][reynolds]
  separates action selection, steering, and locomotion. That maps well to this
  codebase: `Order`/`MoveCoordinator` choose intent, a local locomotion layer
  should steer/collide, and body/turret turn-rate state should handle physical
  execution.
- Elijah Emerson's [Crowd Pathfinding and Steering Using Flow Field Tiles][emerson]
  explains the Supreme Commander 2-style route: flow fields solve the cost of
  moving hundreds or thousands of agents toward goals, support dynamic terrain,
  and combine global fields with local steering. This is valuable reference, but
  it is intentionally more polished and more global than we want right now.
- Treuille, Cooper, and Popovic's [Continuum Crowds][continuum] shows the high
  end of the field-based approach: dynamic potential fields integrate global
  navigation with moving obstacles and produce smooth crowd behavior. That is a
  good warning as much as a tool: it removes exactly the traffic friction and
  imperfect local behavior we may want as gameplay texture.

[pottinger-1]: https://www.gamedeveloper.com/programming/coordinated-unit-movement
[pottinger-2]: https://www.gamedeveloper.com/programming/implementing-coordinated-movement
[reynolds]: https://www.red3d.com/cwr/papers/1999/gdc99steer.html
[emerson]: https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf
[continuum]: https://grail.cs.washington.edu/projects/crowd-flows/continuum-crowds.pdf

## Design Goals

- Stationary combat units should hold ground better than moving units.
- Deployed or setting-up crew weapons should feel planted, not easy to bump.
- Riflemen that are stopped and firing should have some footing, but should not
  become full buildings.
- Tanks need meaningful body direction for front/side/rear damage.
- Tanks need independent turret direction, and turret traverse must matter for
  firing.
- Tanks should not jiggle their body at every tile-center direction change.
- Far group-move orders should roughly preserve the selected units' starting
  shape on arrival. Close group-move orders should still let units bunch up.
- The game should keep some roughness: traffic jams, blocked lanes, and imperfect
  local motion are acceptable flavor.
- Do not recreate StarCraft II-style global flow fields as the default solution.
- Keep the tick path deterministic, bounded, and panic-free.
- Preserve fog safety: any new target, projectile, facing, or event information
  must still pass through the projection boundary.

## Palette of Options

### Option A: Small Stationary Exception

Extend `is_collision_anchored` to include deployed machine gunners and maybe
firing riflemen.

This is the smallest code change, but it is the wrong primitive. The current
anchored exception makes units neither push nor get pushed. That is correct for
latched harvesters because other units must be able to pass through the economy
slot. It is not correct for a deployed machine gun or future AT gun, because
other units would be able to overlap or ghost through it.

Use this only for truly non-solid attachments such as harvesters. Do not use it
as the main "hold ground" model.

### Option B: Weighted Collision and Footing

Keep tile A* unchanged and replace symmetric collision pushes with a collision
profile:

- `Ghost`: ignored by unit collision, for latched harvesters if that behavior is
  still desired.
- `Soft`: moving infantry and workers.
- `Firm`: idle/firing infantry.
- `Braced`: deployed MG, setting-up MG, future deployed AT gun.
- `Heavy`: tanks and other vehicles.

Pairs still use circle overlap, deterministic sorted ids, and passability checks.
The push split is no longer always 50/50. It is based on inverse resistance:

- moving soft unit into braced unit: moving unit absorbs almost all push;
- soft unit into firm unit: moving unit absorbs most push;
- tank into soft infantry: infantry absorbs more push;
- tank into braced weapon: braced unit absorbs little or no push, tank stalls or
  slides;
- equal profiles: split roughly evenly as today.

This directly fixes the "stationary units get shoved" problem while preserving
the current local, slightly rough RTS feel. It also keeps units out of global
pathfinding, so chokes can still clog instead of elegantly solving themselves.

Main risk: if two high-resistance units overlap due to spawn, deployment, or
construction, the resolver still needs a deterministic fallback. A good fallback
is "higher footing holds, lower footing moves; if equal, split."

### Option C: Local Avoidance Layer

Before stepping along the path, compute a short local steering vector from
nearby blockers:

- move toward a path lookahead point;
- add separation from nearby solid units;
- add stronger separation from `Firm`, `Braced`, and `Heavy` profiles;
- clamp the resulting velocity by speed and terrain/building passability.

This reduces direct collision work and makes units try to route around planted
weapons before overlap occurs. It can sit on top of Option B. It should remain
local and cheap: no global flow fields, no large dynamic obstacle map, and no
attempt to make every group move perfect.

This option gives nicer behavior but has more tuning risk. It can also erase too
much jank if over-tuned.

### Option D: Distance-Sensitive Formation Goals

Extend `MoveCoordinator::order_group_move` so group movement assigns per-unit
destination slots from the selected units' starting layout.

The current goal spreading searches tiles around the clicked point. That avoids
every unit using the exact same tile, but it does not know whether the command is
a long march or a tiny regroup click. A distance-sensitive formation pass would
compute:

- the selected group's centroid;
- each selected unit's offset from that centroid;
- the distance from centroid to clicked point;
- a formation scale from `0.0` for near clicks to `1.0` for far clicks.

Then each unit gets a desired destination:

```text
formation_goal = clicked_point + start_offset * formation_scale
```

Near click among the selected units: `formation_scale` is near `0.0`, so units
use the existing compact goal spread around the click. Far click across the map:
`formation_scale` is near `1.0`, so the group arrives in roughly the shape it
started in. Mid-distance clicks blend between the two.

This should not be a rigid formation lock. It is only arrival-slot assignment.
Units can still path independently, get delayed, slide around obstacles, and
arrive somewhat messy. That preserves the desired roughness while fixing the
bad case where four well-spaced units are ordered across the map and all try to
collapse onto one point at the end.

The simple version preserves offsets in world orientation. A more polished
version rotates offsets so the group's starting "front" faces the movement
direction, but that is probably unnecessary early and may make movement feel too
managed.

### Option E: Persistent Standing Slots

Add reservations for final standing positions, firing positions, and deployed
weapon footprints. Goal spreading currently assigns target tiles at order time,
but those positions are not persistent reservations and do not model combat
stances.

Useful reservations:

- a final move slot around the command point;
- a firing slot around a target or choke;
- a harvest/construction slot, if the economy model changes later;
- a deployed crew-weapon footprint or arc origin.

This helps groups settle without all fighting for the same final point. It is
also a good foundation for "set up AT gun facing this lane." It is more
bookkeeping than Option B or Option D, so it should not be the first fix unless
final-position behavior becomes the central problem.

### Option F: Vehicle Locomotion

For vehicles, stop treating `facing` as instantaneous. Add explicit body
orientation and limited turn rate:

- `body_facing`;
- `desired_body_facing`;
- optional `velocity`;
- `turn_rate`;
- optional `accel` and `brake`;
- optional reverse speed or pivot-in-place behavior.

Path following should use a lookahead point rather than "face the next tile
center right now." A tank can keep aiming its hull through the next few waypoints
and only gradually rotate toward the local path corridor. Speed can drop when the
desired direction is far from the hull direction.

This fixes tank body jiggle and makes side/rear armor meaningful. It intentionally
adds some clumsy vehicle behavior, which fits the requested flavor better than
perfect steering.

### Option G: Turret and Traverse Model

Add independent weapon direction for units that need it:

- tanks: `turret_facing`, `turret_turn_rate`, `aim_tolerance`;
- deployed MG/AT gun: `weapon_facing`, limited `traverse_arc`, slow traverse;
- riflemen: can keep using body facing or a very cheap weapon-facing alias.

Combat changes from "in range means face and fire" to:

1. acquire target;
2. stop or slow if the unit must fire stationary;
3. rotate body and/or turret toward the target;
4. fire only when weapon angle is within tolerance and cooldown is ready.

For AT guns, this gives several design knobs:

- fixed arc while deployed, must tear down to cover a different direction;
- slow traverse inside a wide arc;
- very slow whole-body rotation while deployed;
- can fire out of arc only after a setup penalty.

This is the required path for turret swivel time and future firing arcs.

### Option H: Full Dynamic Navigation / Flow Fields / ORCA

Use a more advanced crowd system: flow fields, ORCA/RVO, navmesh agents, or a
dynamic obstacle cost map.

This is overkill for the current game. It risks making movement too polished,
increases tuning and determinism burden, and would fight the desired "some jank
for flavor" direction. Keep it as future reference only if army sizes grow far
beyond what local steering and weighted collision can handle.

## Recommended Direction

Use a staged hybrid:

1. Keep the current tile A* and `MoveCoordinator`.
2. Replace symmetric collision with solid weighted collision profiles.
3. Add distance-sensitive formation goals for group move orders.
4. Add body-facing turn rates for tanks first, then optionally for all units.
5. Add turret/weapon-facing state and aim gates for tanks and deployed weapons.
6. Add small local avoidance only after weighted collision exposes the remaining
   bad cases.
7. Add persistent standing/deployment slots only when final-position behavior
   needs it.

The core concept is:

> Global pathing gives a rough route. Local locomotion decides how a unit tries
> to occupy space, how firmly it holds that space, and where its body/weapon is
> actually pointing.

This keeps the current architecture recognizable and preserves the game's rough
movement character.

## Collision Model Sketch

Add a rules-level helper:

```rust
pub enum CollisionProfile {
    Ghost,
    Soft { resistance: f32 },
    Firm { resistance: f32 },
    Braced { resistance: f32 },
    Heavy { resistance: f32 },
}
```

The exact shape can be smaller than this, but the resolver should ask a helper
instead of matching kinds and orders inline.

Profile should depend on kind and state:

- harvesting worker: `Ghost` if current pass-through mining behavior is kept;
- constructing worker: likely `Firm` or `Braced`, not `Ghost`, if the worker
  should block traffic while building;
- moving worker/rifleman/AT/MG: `Soft`;
- idle or firing rifleman/AT: `Firm`;
- setting-up/deployed MG: `Braced`;
- future deployed AT gun: `Braced`;
- moving tank: `Heavy`;
- stopped/firing tank: `Heavy` with higher resistance.

For each overlapping pair:

```text
if either is Ghost:
    skip that pair
else:
    overlap = radius_a + radius_b - distance
    share_a = resistance_b / (resistance_a + resistance_b)
    share_b = resistance_a / (resistance_a + resistance_b)
    push a by share_a * overlap away from b
    push b by share_b * overlap away from a
```

This formula means the lower-resistance unit moves more. If a proposed push
lands on impassable terrain or a building footprint, transfer more push to the
other unit as the current resolver already does. If neither can move, leave a
bounded residual overlap and mark both as crowded for movement/stuck handling.

The no-overlap invariant should stay, but it may need to understand `Ghost`
pairs and any intentionally unresolved "both trapped" residue.

## Body and Turret State Sketch

Extend movement/combat state rather than adding scattered top-level fields:

```rust
pub struct MovementState {
    pub body_facing: f32,
    pub desired_body_facing: f32,
    pub velocity: (f32, f32),
    // existing order/path/stuck fields...
}

pub struct CombatState {
    pub attack_cd: u32,
    pub target_id: Option<u32>,
    pub setup: WeaponSetup,
    pub weapon_facing: f32,
    pub desired_weapon_facing: f32,
}
```

The current protocol `facing` should remain body facing. Add optional fields only
when needed:

- `weaponFacing` or `turretFacing`;
- `weaponArc` only if the client needs to render or debug the arc;
- `setupState` remains for crew-weapon setup.

Client interpolation should angle-interpolate `facing` and `weaponFacing` by the
shortest angular delta. Tank rendering should rotate hull by `facing` and barrel
by `weaponFacing`.

## Side and Rear Damage Sketch

Once body facing is stable, facing-aware damage can be a pure combat rule.

Input:

- attacker position;
- victim position;
- victim body facing;
- attacker/victim kinds;
- base damage.

Compute the angle from the victim to the attacker and compare it with victim
body facing:

```text
0 degrees is front of victim
front arc: roughly +/-45 degrees
side arc: roughly 45..135 degrees on either side
rear arc: roughly outside 135 degrees
```

Then apply armor multipliers only where they matter:

- infantry vs infantry: no facing modifier;
- small arms vs tank: still ineffective unless design changes;
- AT/tank vs tank: front reduced, side normal or boosted, rear boosted;
- tank vs building: no body-facing modifier unless buildings later get facings.

This belongs in `rules::combat`, not in `services::combat`, so the service still
orchestrates while rules own formulas.

## Preserving Useful Jank

Recommended jank to keep:

- Units do not block A* globally.
- Chokes can clog.
- Groups can arrive messy.
- Formation preservation is an arrival preference, not a guaranteed marching
  formation.
- Tanks can slow, pivot, or get temporarily boxed in.
- Deployed weapons are strong positional commitments.
- Local avoidance is short-range only and never guarantees clean flow.

Jank to remove:

- Stationary firing units sliding around because passers-by nudge them.
- Far move orders collapsing a loose group into an artificial clump.
- Tank hull direction snapping at every tile waypoint.
- Tank barrel and hull being the same angle.
- Firing being instant just because the target is in range.

## Implementation Order

### Phase 1: Solid Footing

- Add collision profile helpers.
- Change collision push split from 50/50 to resistance weighted.
- Make deployed/setup machine gunners and firing stationary infantry firm or
  braced.
- Decide whether constructing workers should remain pass-through or become firm.
- Add tests for planted units holding position against moving units.

### Phase 2: Distance-Sensitive Formation Goals

- In `MoveCoordinator::order_group_move`, compute the selected group's centroid
  and each unit's starting offset.
- Blend from compact goal spreading for near clicks to offset-preserving slots
  for far clicks.
- Search near each desired slot for a passable tile, using deterministic order.
- Keep the result as per-unit goals, not a persistent formation lock.
- Add tests for far-click preservation and near-click regrouping.

### Phase 3: Angle Interpolation Cleanup

- Keep the server's existing `facing` field, but make the client interpolate it
  by shortest angular distance.
- This can reduce visible snap even before vehicle locomotion changes.

### Phase 4: Tank Body Locomotion

- Add limited body turn rate for tanks.
- Follow a path lookahead point instead of instantly facing each tile center.
- Reduce speed when the hull is badly misaligned, or allow slow pivot-in-place.
- Add tests that body facing changes gradually across a 90-degree turn.

### Phase 5: Turrets and Aim Gates

- Add weapon/turret facing to combat state.
- Add aim tolerance before firing.
- Add `turretFacing`/`weaponFacing` to projection and client rendering.
- Make tank hull and barrel draw independently.
- Add tests that a tank cannot fire until the turret has traversed onto target.

### Phase 6: Facing-Aware Armor

- Add a `hit_facing` helper in `rules::combat`.
- Apply front/side/rear multipliers for armored victims.
- Add tests for front, side, and rear AT hits.

### Phase 7: Deployed AT Guns

- Add AT-gun or AT-team deployed state if desired.
- Add fixed or slow-traverse firing arc.
- Add commands/UI only after the server state machine is clear.

## Test Cases to Pin Down

- A deployed machine gunner overlapped by a moving rifleman keeps its position;
  the rifleman is displaced.
- A firing rifleman is firmer than a moving rifleman but less firm than a deployed
  MG.
- A tank can push soft infantry more than infantry can push a tank.
- A tank colliding with a braced weapon stalls/slides instead of casually moving
  the weapon.
- Two equal moving units still split push and do not overlap.
- Ghost harvesters keep the current economy behavior, if that behavior is still
  desired.
- Four spaced units ordered far away arrive with roughly similar relative
  spacing.
- Four spaced units ordered to a point among themselves compact into a tighter
  group.
- A tank following a path with a sharp corner changes body facing gradually.
- Client interpolation rotates angles through the shortest direction.
- A tank with turret facing away from a target waits before firing.
- A front AT hit and a rear AT hit produce different damage against a tank.
- Fogged snapshots never expose hidden target ids or new weapon-facing fields in
  a way that reveals unseen enemies.

## Open Product Choices

- Should a braced MG be completely immovable by tanks, or should tanks slowly
  crush/push it if ordered directly through it?
- Should firing riflemen hold ground only while their attack cooldown is active,
  or as long as they have a target?
- Should idle infantry be `Firm`, or only `Soft` until they are firing or given a
  hold-position command?
- What distance threshold should switch a group command from regrouping to
  preserving formation?
- Should far-order formation offsets stay in world orientation, or rotate toward
  the movement direction?
- Should future AT guns be their own unit kind, or should AT teams deploy into an
  AT-gun stance?
- Should tanks turn in place when path heading is too different, or keep moving
  slowly while turning?
- Should side/rear damage be only for armored victims, or also for crew weapons
  and buildings later?
