# Phase 3 - Movement and Collision Integration

Goal: movement and collision correction use the same body legality as spawning and construction.

This phase is where the shared standability layer becomes part of per-tick locomotion.

## Dependencies

- Collision Phase 0.
- Movement Phase 0 weighted collision.
- Movement Phase 3 tank body locomotion should be integrated before or during this phase if it is
  still on a feature branch.

## Scope

In scope:

- Replace center-tile movement landing checks with unit-body static standability.
- Replace collision push target checks with unit-body static standability.
- Add end-of-tick invariant coverage for unit bodies intersecting building rectangles.
- Preserve weighted collision and ghost pass-through behavior.
- Preserve bounded A* and current path coordinator shape.

Out of scope:

- No local steering yet. That is Phase 4 of this collision plan and Phase 6 of the movement plan.
- No dynamic units as pathfinding blockers.
- No global flow fields.
- No broad tank physics rewrite beyond Movement Phase 3.

## Movement Phase 3 Integration

With tank body locomotion, tank movement looks roughly like:

```text
desired body angle from path lookahead
rotate hull toward desired angle
scale speed by body angle error
compute candidate step
validate candidate body position
```

The final validation must be:

```rust
standability::unit_static_standable(map, occ, tank_kind, candidate_x, candidate_y)
```

This prevents a turn-limited tank from solving one visual problem while still clipping its circular
body through a building edge. If the candidate is illegal, the tank may rotate in place, slide only
through a body-legal axis fallback, or trigger the existing static-blocked repath debounce.

Collision correction must not change tank facing. It only changes position.

## Movement Landing

Replace these center-tile checks:

- `tile_passable_at(...)`
- `stays_on_passable(...)`

with body-aware checks:

```rust
unit_static_standable(map, occ, kind, x, y)
```

Keep behavior policy-specific:

- Path following ignores dynamic units, because soft overlap plus collision remains the dynamic
  traffic model.
- Static terrain and buildings are hard blockers.
- Wall-slide attempts must validate the whole body, not just the center tile.

## Collision Correction

Keep weighted pair resolution from movement Phase 0, but validate each proposed push with the
same static body predicate.

If a pair cannot fully separate because every legal push is blocked by static geometry:

- leave the smallest residual overlap achievable that tick,
- keep deterministic behavior,
- increment no unbounded search,
- rely on movement/repath/steering to resolve over subsequent ticks.

After this phase, the tolerance should be for real static pinning only, not for ordinary missed
building-body collisions.

## Invariants

Add or strengthen invariants:

- Every non-ghost unit body must be static-standable at end of tick.
- No non-ghost unit body intersects any building footprint rectangle.
- Unit-unit overlap tolerance remains explicit and documented.
- Optional: ghost workers may overlap their latched resource/build site. Production spawn and final
  scaffold creation still cannot place on top of them. Build intent may ignore only the chosen
  builder's own body as described in Phase 2.

## Tests

Add Rust tests:

- `movement_rejects_tank_body_clipping_building_corner`
- `wall_slide_uses_unit_body_clearance`
- `collision_push_does_not_move_tank_body_into_building`
- `tank_body_locomotion_rotates_without_illegal_step_when_blocked`
- `unit_body_vs_building_invariant_catches_manual_bad_state`
- Existing stuck, sidestep, weighted collision, and tank-body tests still pass.

Run:

```bash
cd server && cargo fmt && cargo test movement::tests invariants::tests standability
cd server && cargo test
```

## Acceptance Criteria

- Movement cannot move a unit body into a building even when its center tile remains passable.
- Collision correction cannot push a unit body into a building.
- Movement Phase 3 tank rotation and speed scaling remain intact.
- Existing pathing remains bounded and deterministic.
- No protocol or client files change.
