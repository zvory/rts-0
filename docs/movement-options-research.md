# RTS Movement Options Research

This is a research and product-shaping note for a future movement system. The
phased implementation plan it informed has been completed and its docs removed;
see `DESIGN.md` for the implemented movement, collision, and steering behavior.
This note does not
change the live wire protocol or simulation contracts. Any later implementation
that changes entity state, protocol fields, combat rules, balance, or module
boundaries must update `DESIGN.md` in the same change.

## Sources Considered

The useful external references are less about copying one famous movement system
and more about separating the problem into layers.

- Dave Pottinger, [Coordinated Unit Movement][pottinger-1] and
  [Implementing Coordinated Movement][pottinger-2]. These are the closest fit
  for an RTS like this. The key ideas are group movement vs formation movement,
  movement execution as a separate problem from pathfinding, and several possible
  cohesion levels: same speed, same path, and same arrival time.
- Craig Reynolds, [Steering Behaviors for Autonomous Characters][reynolds]. This
  is useful because it separates action selection, steering, and locomotion. For
  this codebase, that maps to orders/coordinator, local steering/collision, and
  physical body/turret state.
- Elijah Emerson, [Crowd Pathfinding and Steering Using Flow Field Tiles][emerson].
  This is the Supreme Commander 2-style approach: use flow-field tiles so many
  units can share goal-directed movement efficiently. It is good reference, but
  it is probably too polished and too global for the current game.
- Treuille, Cooper, and Popovic, [Continuum Crowds][continuum]. This is the high
  end of field-based crowd movement. It confirms that smooth field movement can
  solve crowding, but it also shows exactly the kind of frictionless crowd flow
  we probably do not want.

[pottinger-1]: https://www.gamedeveloper.com/programming/coordinated-unit-movement
[pottinger-2]: https://www.gamedeveloper.com/programming/implementing-coordinated-movement
[reynolds]: https://www.red3d.com/cwr/papers/1999/gdc99steer.html
[emerson]: https://www.gameaipro.com/GameAIPro/GameAIPro_Chapter23_Crowd_Pathfinding_and_Steering_Using_Flow_Field_Tiles.pdf
[continuum]: https://grail.cs.washington.edu/projects/crowd-flows/continuum-crowds.pdf

## Current Code Constraints

The current server model is already split in a useful way:

- `PathingService` runs tile A* over terrain and building footprints. Units do
  not block paths.
- `MoveCoordinator` owns path request budgeting, goal spreading, spawn search,
  repath throttling, and `AwaitingPath` processing.
- `movement_system` consumes tile-center waypoints and moves units continuously
  in world pixels.
- `resolve_collisions` runs after movement and resolves unit overlap with
  symmetric 50/50 circle pushes.
- `is_collision_anchored` currently makes harvesting and constructing workers
  pass-through: they neither push nor are pushed.
- `facing` is a single instant angle. Movement sets it from the current path
  segment; combat sets it from the current target.
- Tanks render hull and barrel from the same `facing`.
- Client interpolation blends `x/y`, but not angles.

That means the core problem is not "replace A*." The current global path layer
can stay. The missing layer is local locomotion: how units occupy space, how
firmly they hold that space, how they preserve group shape, and how bodies and
weapons turn.

## Product Requirements

- Idle units should be easy to shove aside when active movement needs space.
- Units that are actively firing, holding, or deployed should not casually slide
  around when other units pass.
- Deployed machine gunners and future AT guns should feel planted.
- Firing riflemen should have some footing, but should not behave like buildings.
- Tanks need stable body direction for front/side/rear armor.
- Tanks need independent turret direction, with traverse time affecting firing.
- Crew weapons need limited firing arcs or slow traverse later.
- Far group move orders should roughly preserve starting shape on arrival.
- Close group move orders should still let selected units regroup or bunch up.
- Traffic jams, blocked lanes, and imperfect local motion are acceptable flavor.
- Avoid StarCraft II-style or Supreme Commander-style polished global flow unless
  army sizes force it later.
- Keep the tick path deterministic, bounded, and panic-free.

## Option Palette

### Option 1: Reuse Anchoring For Stationary Units

Make deployed MGs, firing riflemen, or future AT guns return true from
`is_collision_anchored`.

Verdict: reject as the main solution.

This is tempting because it is small, but the current anchored behavior is really
`Ghost`, not `Solid`. A latched miner being pass-through is useful because the
economy can keep moving around resource nodes. A deployed machine gun should not
be pass-through; other units should be forced around it or displaced by it.

Keep this primitive only for special attachment states that intentionally do not
occupy space.

### Option 2: Weighted Collision And Footing

Keep units out of global pathfinding, but change local collision from symmetric
50/50 pushes to resistance-weighted pushes.

Profiles:

- `Ghost`: pass-through attachment states, such as current harvesters if that
  behavior remains.
- `Soft`: moving infantry, workers, and idle infantry with no hold/deploy state.
- `Firm`: firing infantry or units explicitly holding ground.
- `Braced`: deployed or setting-up crew weapons.
- `Heavy`: tanks and future vehicles.

Pair resolution:

```text
if either profile is Ghost:
    skip the pair
else:
    overlap = radius_a + radius_b - distance
    share_a = resistance_b / (resistance_a + resistance_b)
    share_b = resistance_a / (resistance_a + resistance_b)
    push a by share_a * overlap away from b
    push b by share_b * overlap away from a
```

The lower-resistance unit moves more. Idle units should use a low enough
resistance that a moving unit can noticeably shove them aside instead of being
blocked by a passive body. If one push would land in terrain or a building
footprint, transfer more push to the other side, similar to the current resolver.
If neither can move, leave a bounded residual overlap and let stuck handling
react.

What this buys:

- Idle units are easy to clear out of the way.
- Active, holding, or deployed units hold ground without becoming buildings.
- Moving units still collide and create jams.
- Tanks can be physically heavier than infantry.
- Deployed MGs and AT guns become real positional commitments.

This should be the first movement-system implementation phase.

### Option 3: Distance-Sensitive Formation Goals

This addresses the "four guys right-click far away" issue.

The current goal spreading finds unique nearby tiles around the clicked point. It
does not know whether the command is a long march or a close regroup. Replace or
extend it with distance-sensitive formation slots:

```text
centroid = average selected unit position
offset_i = unit_i.position - centroid
move_distance = distance(centroid, clicked_point)
formation_scale = smoothstep(near_threshold, far_threshold, move_distance)
desired_goal_i = clicked_point + offset_i * formation_scale
```

Behavior:

- Near click among the selected units: `formation_scale` is near `0`, so units
  compact around the clicked point.
- Far click across the map: `formation_scale` is near `1`, so units preserve
  roughly the same starting shape on arrival.
- Medium-distance click: blend between regrouping and preserving shape.

This is not a marching-formation system. It only assigns arrival slots. Units
still path independently, get delayed by jams, and arrive somewhat messy.

Open choice: keep offsets in world orientation at first. Rotating offsets toward
the movement direction is more formation-like, but may feel too managed.

### Option 4: Local Steering Layer

Add a lightweight steering pass before or during movement:

- follow a path lookahead point;
- separate from nearby solid units;
- steer more strongly away from `Firm`, `Braced`, and `Heavy` profiles;
- treat idle `Soft` units as low-priority obstacles that can be pushed through
  instead of routed around;
- clamp to unit speed and terrain/building passability.

This follows the Reynolds-style separation of "intent" from "steering", but it
should remain short-range and local. Do not build a dynamic obstacle cost map yet.

What this buys:

- Units try to avoid planted weapons before hard overlap happens.
- Chokes still clog.
- Movement gets less stupid without becoming smooth global crowd flow.

This is useful after weighted collision, not before it.

### Option 5: Vehicle Body Locomotion

For tanks, stop using instantaneous `facing` as the real body direction.

Add state such as:

```rust
pub struct MovementState {
    pub body_facing: f32,
    pub desired_body_facing: f32,
    pub velocity: (f32, f32),
    // existing path/order/stuck fields...
}
```

Tank movement should:

- choose a lookahead point along the path;
- rotate `body_facing` toward that desired direction at a limited turn rate;
- slow down or pivot when the body is badly misaligned;
- keep the existing `facing` protocol field as body facing.

What this buys:

- Tank hulls stop twitching at every tile-center direction change.
- Front/side/rear armor becomes meaningful.
- Tanks feel clumsy in a useful way.

This can be tank-only first. Infantry can keep simple facing unless they need
more sophistication later.

### Option 6: Turrets, Weapon Facing, And Aim Gates

Add independent weapon direction to combat state:

```rust
pub struct CombatState {
    pub attack_cd: u32,
    pub target_id: Option<u32>,
    pub setup: WeaponSetup,
    pub weapon_facing: f32,
    pub desired_weapon_facing: f32,
}
```

Combat should become:

1. acquire or keep target;
2. stop, slow, or continue based on unit kind;
3. rotate body and/or weapon toward target;
4. fire only when target is in range, weapon angle is within tolerance, and
   cooldown is ready.

For tanks, render hull from `facing` and barrel from `weaponFacing`. For future
AT guns or deployed AT teams, `weapon_facing` can be constrained by an arc.

What this buys:

- Turret swivel time matters.
- A tank cannot instantly fire at a target behind it.
- Deployed crew weapons can cover lanes instead of 360-degree space.

### Option 7: Persistent Standing And Deployment Slots

Add reservations for final positions, firing positions, and deployed weapon
footprints.

This is stronger than Option 3. Option 3 gives one-shot per-unit move goals.
Slots are persistent reservations with ownership and lifecycle.

Useful future slots:

- final move slots around a command point;
- firing slots near cover or around a target;
- deployment slots for MGs and AT guns;
- build/harvest slots if those systems need stronger occupancy later.

What this buys:

- Cleaner final settling.
- Better setup for "deploy this AT gun facing this lane."
- Potential hooks for cover and forests later.

Cost:

- More state to maintain.
- More invalidation edge cases.
- Higher risk of turning normal movement into a rigid formation system.

This should wait until weighted collision and distance-sensitive formation goals
show where persistent reservations are actually needed.

### Option 8: Flow Fields / Continuum Crowds / ORCA

Use a higher-end crowd system:

- flow fields for many units moving to a shared goal;
- dynamic cost fields;
- velocity obstacle methods such as ORCA/RVO;
- continuum crowd-style potential fields.

Verdict: keep as reference, not as the current direction.

These systems solve real large-crowd problems, but they also tend to make
movement cleaner and more coordinated than this game wants. They increase the
determinism and tuning burden, and they would replace the useful jank of chokes,
traffic, and positional obstruction with smooth throughput.

Revisit only if army sizes grow enough that per-unit A* and local steering stop
being viable.

## Recommended Direction

Use a staged hybrid:

1. Keep tile A* and `MoveCoordinator`.
2. Add weighted collision and footing profiles.
3. Add distance-sensitive formation goals for group move orders.
4. Add angle interpolation on the client.
5. Add tank body turn rates and path lookahead.
6. Add turret/weapon-facing state and aim gates.
7. Add small local steering if collision still produces avoidable stupidity.
8. Add persistent deployment or standing slots only when there is a concrete
   use case.

The guiding model:

```text
Orders choose intent.
Pathing gives a rough route.
Formation goals choose where each selected unit is trying to end up.
Local locomotion decides how each unit occupies space.
Body and weapon state decide where the unit is actually pointing.
Combat rules decide whether the shot is possible and how damage applies.
```

This keeps rough RTS movement while removing the specific bad jank:

- passive idle bodies acting like planted obstacles;
- planted combat units sliding around;
- long-distance group commands collapsing into clumps;
- tank bodies jittering as path waypoints change;
- tank barrels being welded to hull direction;
- instant firing without traverse or arc constraints.

## Facing-Aware Damage Sketch

Once tank body facing is stable, side/rear damage can be a pure rule in
`rules::combat`.

Inputs:

- attacker position;
- victim position;
- victim body facing;
- attacker/victim kinds;
- base damage.

Classify the hit by comparing the angle from victim to attacker against victim
body facing:

```text
front: roughly +/-45 degrees
side: roughly 45..135 degrees on either side
rear: roughly outside 135 degrees
```

Suggested first rule:

- infantry vs infantry: no facing modifier;
- small arms vs tank: still ineffective unless balance changes;
- AT/tank vs tank front: reduced damage;
- AT/tank vs tank side: normal or moderately boosted damage;
- AT/tank vs tank rear: boosted damage;
- tank vs building: no facing modifier unless buildings later get facings.

## Test Cases To Pin Down

- Deployed MG overlapped by moving rifleman keeps position; rifleman is displaced.
- Firing rifleman is firmer than moving rifleman but less firm than deployed MG.
- Tank pushes soft infantry more than infantry pushes tank.
- Moving rifleman can shove an idle rifleman aside without getting stuck behind
  it.
- Idle rifleman is easier to displace than a firing or holding rifleman.
- Tank colliding with braced weapon stalls or slides instead of casually moving
  the weapon.
- Equal moving units still split push and do not overlap.
- Ghost harvester behavior remains intentional and tested if kept.
- Four spaced units ordered far away arrive with roughly similar relative
  spacing.
- Four spaced units ordered to a nearby point compact into a tighter group.
- Client angle interpolation rotates through the shortest direction.
- Tank body facing changes gradually across a sharp path turn.
- Tank turret facing away from target delays firing.
- Front, side, and rear AT hits produce different tank damage.
- Fogged snapshots never expose hidden target ids or hidden target-derived
  weapon angles.

## Open Product Questions

- Should a braced MG be completely immovable by tanks, or can tanks slowly shove
  or crush it?
- Should firing riflemen become `Firm` only during cooldown, or for as long as
  they have a target?
- How low should idle-unit resistance be relative to moving infantry?
- What distance thresholds separate regrouping from formation preservation?
- Should far-order formation offsets stay in world orientation, or rotate toward
  movement direction?
- Should AT teams deploy into an AT-gun stance, or should AT guns be a separate
  unit?
- Should tanks pivot in place when misaligned, or crawl forward while turning?
- How much useful jank should survive after local steering is added?
