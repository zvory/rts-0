# Phase 2 - Construction and Building Placement Correctness

Goal: construction never creates a building footprint that intersects an existing unit body or
static object.

This phase replaces tile-center placement checks with body-aware footprint checks and turns worker
ejection into a defensive fallback rather than a normal construction mechanism.

## Dependencies

- Phase 0 standability helpers.

## Scope

In scope:

- Replace `footprint_placeable` internals with `building_site_clear`.
- Make build command feedback and arrival-time construction use the same predicate.
- Ensure the constructing worker must be outside the final footprint before scaffold creation.
- Add body-aware tests for tanks near building footprints.

Out of scope:

- No building rotation.
- No construction reservations across many pending build commands.
- No UI placement preview changes unless the server/client mirror is already wrong.
- No change to resource-node movement behavior.

## Files To Touch

- `server/src/game/services/occupancy.rs`
- `server/src/game/services/commands.rs`
- `server/src/game/services/construction.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/invariants.rs`

## Design

Current placement checks reject a unit only when its center tile is inside the footprint. That is
insufficient for tanks and any future large unit.

Use rectangle-vs-circle checks:

```text
candidate building footprint rectangle
    must not intersect any living unit circle
```

This includes the worker. The worker may be inside the requested footprint when the command is
issued only as an intent convenience, but the build path must move it to an outside staging point.
At scaffold creation time, if the worker body still intersects the footprint, construction must not
start.

## Worker Ejection

The existing ejection path should become defensive cleanup only:

- Prefer not to create a scaffold if the worker intersects the site.
- Keep `eject_worker_if_inside` temporarily to recover legacy or manually constructed bad states.
- Add an invariant so new normal gameplay stops relying on ejection.
- Remove or narrow ejection during Phase 5 if tests prove it is no longer needed.

## Implementation Steps

1. Implement `building_site_clear` in Phase 0 if not already done.
2. Rework `footprint_placeable` as a thin wrapper over `building_site_clear` so existing call sites
   keep compiling.
3. Update build command validation to use the same predicate as arrival-time construction.
4. Preserve the current special command-time behavior where a worker inside the requested footprint
   can receive an order to walk out, but document that this is command feedback only.
5. In construction arrival, reject the scaffold if any living unit body intersects the footprint.
6. Keep notice behavior stable: "Cannot build there" is enough.
7. Add invariant coverage for unit-body-vs-building-footprint overlap at end of tick.

## Tests

Add Rust tests:

- `building_site_rejects_tank_body_touching_footprint_edge`
- `build_order_can_start_when_worker_inside_intent_but_stages_outside`
- `construction_revalidates_worker_body_outside_footprint`
- `construction_rejects_other_unit_body_intersecting_footprint`
- `completed_building_never_overlaps_non_ghost_unit_body`
- Existing build path and AI build-site tests still pass.

Run:

```bash
cd server && cargo fmt && cargo test construction::tests commands::tests occupancy::tests standability
cd server && cargo test
```

## Acceptance Criteria

- Building placement and construction use the same body-aware site-clear predicate.
- A tank near a footprint edge can block construction even when its center tile is outside the
  footprint.
- New construction does not depend on teleporting workers out of buildings.
- AI build-site selection remains compatible because it goes through the shared wrapper.
