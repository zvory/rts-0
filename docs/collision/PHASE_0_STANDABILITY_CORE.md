# Phase 0 - Standability Core

Goal: add shared geometry and standability helpers without changing broad gameplay behavior yet.

This phase creates the vocabulary and tests needed before production, construction, movement, and
collision start depending on the same legality checks.

## Scope

In scope:

- Add pure body geometry helpers.
- Add exact unit-body-vs-static-world legality checks.
- Add building-footprint-vs-unit-body intersection helpers.
- Add tests that document current failure cases as helper-level behavior.
- Keep existing systems mostly wired as-is unless a helper replacement is tiny and behavior-neutral.

Out of scope:

- Do not change production spawn behavior yet.
- Do not block production queues yet.
- Do not change construction placement behavior yet.
- Do not rewrite pathfinding.
- Do not implement local steering.

## Suggested Files

- `server/src/game/services/geometry.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/services/mod.rs`
- `server/src/game/services/occupancy.rs` only for small wrapper reuse.
- `server/src/game/invariants.rs` only for helper imports or test-only assertions.

## Geometry API

Keep the API small and explicit:

```rust
pub(crate) struct CircleBody {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

pub(crate) struct RectBody {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

pub(crate) fn unit_body(kind: EntityKind, x: f32, y: f32) -> Option<CircleBody>;
pub(crate) fn building_rect_for_footprint(kind: EntityKind, tile_x: u32, tile_y: u32)
    -> Option<RectBody>;
pub(crate) fn building_rect_for_entity(map: &Map, e: &Entity) -> Option<RectBody>;
pub(crate) fn circle_intersects_rect(circle: CircleBody, rect: RectBody) -> bool;
```

Prefer exact axis-aligned rectangle math over tile-center shortcuts.

## Standability API

Suggested predicates:

```rust
pub(crate) fn unit_static_standable(
    map: &Map,
    occ: &Occupancy,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool;

pub(crate) fn unit_spawn_standable(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool;

pub(crate) fn building_site_clear(
    map: &Map,
    entities: &EntityStore,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool;
```

`unit_static_standable` must check:

- finite coordinates,
- whole unit circle inside world bounds,
- circle does not overlap impassable terrain tiles,
- circle does not overlap existing building footprint rectangles.

`unit_spawn_standable` adds dynamic unit bodies. It should reject overlap with all living units,
including ghost/pass-through workers.

`building_site_clear` must check:

- in-bounds footprint math with checked arithmetic,
- every footprint tile has passable terrain,
- footprint rectangle does not intersect existing building rectangles,
- footprint rectangle does not intersect resource node occupancy,
- footprint rectangle does not intersect living unit circles.

## Implementation Steps

1. Add `geometry.rs` with pure helpers and focused unit tests.
2. Add `standability.rs` with static and dynamic predicates.
3. Reuse existing `Occupancy` for tile-level building footprint checks, but do not let that remain
   the only source of truth for body-vs-rectangle intersections.
4. Keep helper iteration deterministic. Prefer `entities.iter()` because it sorts ids.
5. Add no panics on invalid entity kind or bad coordinates. Invalid inputs return `false`.
6. Add a test showing a tank center outside a building tile can still have its radius intersect the
   building rectangle.
7. Add a test showing an occupied spawn circle is rejected by `unit_spawn_standable`.
8. Add a test showing a building footprint is rejected when it intersects a unit circle even if the
   unit center is outside the footprint tiles.

## Tests

Add Rust tests:

- `tank_body_intersects_building_even_when_center_tile_is_clear`
- `unit_static_standable_rejects_body_clipping_building`
- `unit_spawn_standable_rejects_existing_unit_overlap`
- `building_site_clear_rejects_unit_body_intersection`
- `building_site_clear_rejects_resource_node_footprint`
- `standability_rejects_non_finite_coordinates`

Run:

```bash
cd server && cargo fmt && cargo test standability geometry occupancy
cd server && cargo test
```

## Acceptance Criteria

- There is one reusable place to ask whether a unit body or building site is legal.
- Tests cover the exact failure mode that tile-center checks miss.
- Existing gameplay remains effectively unchanged in this phase.
- No protocol or client files change.
