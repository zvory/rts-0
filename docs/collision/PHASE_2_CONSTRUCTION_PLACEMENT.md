# Phase 2 - Construction and Building Placement Correctness

Goal: construction never creates a building footprint that intersects an existing unit body or
static object, while preserving the convenience of ordering a worker to build over its own current
position and walk out.

This phase replaces tile-center placement checks with body-aware footprint checks and turns worker
ejection into a defensive fallback rather than a normal construction mechanism.

## Dependencies

- Phase 0 standability helpers.

## Scope

In scope:

- Replace `footprint_placeable` internals with `building_site_clear`.
- Make build command feedback and arrival-time construction use the same geometry layer with
  explicit build-intent vs final-placement policies.
- Preserve command-time build-over-self behavior for the chosen builder.
- Ensure the constructing worker must be outside the final footprint before scaffold creation.
- Update the client placement preview so it matches the server's build-intent rules.
- Add body-aware tests for tanks near building footprints.

Out of scope:

- No building rotation.
- No construction reservations across many pending build commands.
- No change to resource-node movement behavior.
- No protocol changes; preview remains client-only/advisory and the server remains authoritative.

## Files To Touch

- `server/src/game/services/occupancy.rs`
- `server/src/game/services/commands.rs`
- `server/src/game/services/construction.rs`
- `server/src/game/services/move_coordinator.rs`
- `server/src/game/services/standability.rs`
- `server/src/game/invariants.rs`
- `client/src/input.js`
- `client/src/state.js` only if placement state needs to expose the chosen builder id.
- `client/src/renderer.js` only if the preview needs a visual distinction for build-over-self.
- `tests/client_contracts.mjs` or another focused client test if placement preview coverage already
  exists there.

## Design

Current placement checks reject a unit only when its center tile is inside the footprint. That is
insufficient for tanks and any future large unit.

Use rectangle-vs-circle checks:

```text
candidate building footprint rectangle
    must not intersect any living unit circle
```

Separate build intent from scaffold creation:

- Build intent and client preview may ignore the chosen builder's own body. This preserves the
  current convenience where a selected worker can place a building over itself, receive the build
  order, and walk to an outside staging point.
- Build intent and client preview must still reject other living unit bodies, resource nodes,
  terrain blockers, and existing building footprints.
- Scaffold creation uses the stricter final placement policy. At arrival time, if the worker body
  or any other living unit body still intersects the footprint, construction must not start.

For multi-worker selections, the preview should use the same chosen builder that confirm-click will
send in `cmd.build`. Today that is the first selected worker returned by the input path. Do not make
the preview ignore every selected worker unless the command semantics change too.

The client preview is advisory. The server repeats the same build-intent validation on command and
the stricter final validation on construction arrival.

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
3. Add an explicit build-intent predicate or policy that accepts a `builder_id` and ignores only
   that worker's own body.
4. Update build command validation to use the build-intent policy.
5. Update client placement preview to mirror the build-intent policy for the chosen builder. The
   preview should show a valid placement when the only dynamic blocker is that builder's body, and
   invalid placement when another unit body blocks the footprint.
6. In construction arrival, use the stricter final placement policy and reject the scaffold if any
   living unit body intersects the footprint.
7. Keep notice behavior stable: "Cannot build there" is enough.
8. Add invariant coverage for unit-body-vs-building-footprint overlap at end of tick.

## Tests

Add Rust tests:

- `building_site_rejects_tank_body_touching_footprint_edge`
- `build_order_can_start_when_worker_inside_intent_but_stages_outside`
- `client_preview_allows_chosen_worker_body_inside_footprint`
- `client_preview_rejects_other_unit_body_inside_footprint`
- `construction_revalidates_worker_body_outside_footprint`
- `construction_rejects_other_unit_body_intersecting_footprint`
- `completed_building_never_overlaps_non_ghost_unit_body`
- Existing build path and AI build-site tests still pass.

Run:

```bash
cd server && cargo fmt && cargo test construction::tests commands::tests occupancy::tests standability
node tests/client_contracts.mjs
cd server && cargo test
```

## Acceptance Criteria

- Build intent, client preview, and final construction use explicit body-aware policies.
- A selected worker can still place a building over itself as an order convenience.
- A tank near a footprint edge can block construction even when its center tile is outside the
  footprint.
- New construction does not depend on teleporting workers out of buildings.
- AI build-site selection remains compatible because it goes through the shared wrapper.
