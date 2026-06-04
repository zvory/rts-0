# Tank Movement - Multi-Phase Plan

This plan replaces the current "large circle follows tile waypoints" tank feel with a
server-authoritative tracked-vehicle locomotion model. The goal is not to delete A* first. The goal
is to stop asking tile A* to produce believable tank motion.

Tile A* should remain the coarse route planner. Tanks should then follow that route through a tank
controller with hull facing, track-like differential steering, acceleration limits, reverse/pivot
behavior, and body-aware collision.

## Scout Findings

Current movement is already layered enough to avoid a full protocol rewrite:

- `PathingService` owns tile A* and static terrain/building route checks.
- `MoveCoordinator` owns command-to-path conversion, group destination spreading, spawn search,
  path request budgeting, and path smoothing for tank move orders.
- `movement_system` consumes world-pixel waypoints and already has a tank-only branch for bounded
  hull rotation, speed scaling, oil burn, local steering, stuck handling, and static repath.
- `MovementState` already stores `facing`, `path`, `path_goal`, stuck counters, and tank oil fields.
- Snapshots already expose `facing` for body/hull orientation and `weaponFacing` for the turret.
- The client already interpolates `x`, `y`, `facing`, and `weaponFacing`.

That means Phase 1 can be done inside the existing movement seam without changing the wire protocol.

The deeper refactors are internal simulation refactors:

- Unit geometry is currently circular (`geometry::unit_body`, `standability`, spawn checks, building
  placement checks, invariants, and collision all assume circles).
- Tank collision still behaves like an oversized disk, which contributes to the visible "personal
  space" problem.
- Local steering can still choose sideways-looking displacement because it proposes a direct landing
  direction, not track commands.
- Collision resolution moves unit centers directly after locomotion, so tanks can still be displaced
  in ways that do not look like their tracks caused the motion.
- Pathing clearance uses tile radius, not an oriented footprint or swept vehicle hull.

## Phases

- [Phase 0 - Measurements, fixtures, and acceptance criteria](PHASE_0.md)
- [Phase 1 - Tank locomotion controller inside the current circle-body model](PHASE_1.md)
- [Phase 2 - Path following, reverse, pivot, and stuck behavior](PHASE_2.md)
- [Phase 3 - Vehicle body geometry refactor](PHASE_3.md)
- [Phase 4 - Collision, traffic, and local avoidance for heavy vehicles](PHASE_4.md)
- [Phase 5 - Client presentation and player feedback](PHASE_5.md)
- [Phase 6 - Integration, balance, and documentation audit](PHASE_6.md)

## Non-Negotiable Invariants

1. **Server authority stays intact.** Clients still send intent only; the server owns movement.
2. **No tank-only wire protocol unless proven necessary.** `facing`, `weaponFacing`, position, and
   existing state fields are enough for the first implementation.
3. **`Game::tick()` stays panic-free.** Any new geometry or controller math must reject non-finite
   values and stale ids as no-ops.
4. **A* remains coarse route intent.** The path planner can improve, but realistic tank motion comes
   from the locomotion/controller layer.
5. **No sideways tank locomotion.** A tank may rotate, reverse, pivot, or be collision-displaced, but
   commanded self-motion should come from hull/track state.
6. **Geometry changes are mirrored through all users.** If tank bodies stop being circles, update
   standability, spawn checks, building placement checks, collision, invariants, and tests together.
7. **Fog/projection does not change.** Movement realism must not leak hidden entity positions or
   target directions.
8. **Balance notes are collected.** Any changed tank speed, turn rate, acceleration, fuel burn, body
   size, or traffic behavior needs player-facing patch-note bullets.

## Suggested Implementation Order

Implement the phases in order. Phases 0-2 are the narrowest path to improved tank feel. Phases 3-4
are where the true deep refactor lives: replacing circular vehicle occupancy with body-aware
geometry and making collision/traffic respect that body.

If schedule pressure is high, stop after Phase 2 and playtest. If tanks still feel like they have an
invisible bubble, Phase 3 is not optional; that symptom is mainly caused by circular body geometry.

