# Phase 2 - Precomputed Pathing Edges

## Phase Status

- [ ] Ready for implementation.

## Objective

Remove repeated passability and current-cost work from the hot relaxation loop while keeping the
existing production graph and every returned result exactly unchanged.

## Design Constraints

- Scope ownership to pathing graph data; do not redesign all occupancy or spatial derived state.
- Separate immutable authored-map/profile edge facts from per-room dynamic building effects.
- Give immutable base edges a deterministic authored-map pathing content key covering dimensions,
  terrain/roads, slow overlays, elevation, doodads/trees, no-vehicle tiles, and every other layer
  consumed by path legality or current costs. Compute it on map load/edit, not per request; any Lab
  or map replacement that changes those facts resets the base graph.
- A single building mutation must not rebuild every profile's complete map. Update only the changed
  footprint and the bounded radius/diagonal/clearance neighborhood that can affect legal edges and
  costs, or use an equally bounded dynamic overlay proven faster.
- Preserve the existing phase at which building insertion, removal, relocation, Lab mutation, and
  reopen become visible. Do not add owner or completion to the topology key unless current blocker
  rules actually use them. Units moving do not invalidate static pathing topology.
- Choose an edge encoding wide enough for audited tree, clearance, corner, and step costs; do not
  assume `u16` if it changes saturation or overflow behavior.
- Key data by proven routing profile, not blindly by `EntityKind`.

## Work

- Add a rebuildable per-room `PathGraph` under the existing pathing/`DerivedState` ownership seam.
  It owns map-independent arrays and the exact ordered building topology key
  `(id, kind, pos_x_bits, pos_y_bits)` used by current occupancy reuse; each system boundary creates
  a short-lived view borrowing `&Map` and current occupancy facts. Store deterministic tile/direction
  indexing, immutable base edges, a monotonic rebuild generation, and only the local overlay/update
  data required by pathfinding.
- Precompute current-main destination legality, body-radius clearance, diagonal orthogonal guards,
  oriented pinch restrictions, tree avoidance, vehicle clearance/corner penalties, and current
  slow-tile surcharge for each actual profile.
- Make dense A* relax through contiguous edge/profile reads while retaining the current heuristic,
  heap/tie order, turn penalty, cap/fallback rule, and diagnostics.
- Audit every building/static-body mutation and locally update affected edge origins/destinations.
  Test overlapping blockers and reopen so one removed footprint cannot reopen an edge still blocked
  by another.
- Keep the existing content-derived static fingerprint separate from the monotonic rebuild
  generation. Cache keys and scheduling parity retain the content fingerprint so close-then-reopen
  and derived-state rebuilds behave exactly as today; generation decides only whether tables need
  refresh. During this parity phase, preserve the current cache validation predicate exactly; do not
  strengthen tile-passability validation into full edge/corner validation and thereby change hit/miss
  or resolution ticks.
- Report immutable/dynamic bytes per profile, initialization time, local update p50/p95/worst case,
  relaxations, full-path time, and the remaining hottest functions.
- Update the `DerivedState` registry and pathing ownership/invalidation section of
  `docs/design/server-sim.md` if the graph becomes a new field or changes rebuild semantics.

## Expected Touch Points

- a focused graph module under `server/crates/sim/src/game/services/pathing/`
- `server/crates/sim/src/game/services/pathing.rs`
- `server/crates/sim/src/game/pathfinding.rs`
- `server/crates/sim/src/game/services/occupancy.rs`
- `server/crates/sim/src/game/services/pathing/tree_detours.rs`
- `server/crates/sim/src/game/derived_state.rs` if ownership changes
- focused topology/cache/profile tests
- `docs/design/server-sim.md`

## Identity Requirement

Require the complete Phase 1 legacy comparison: exact raw and finalized routes, waypoint bits,
diagnostics, capped fallback, cache sequence, scheduling/resolution ticks, Hellhole semantic output,
and deterministic replay. New graph memory/update counters are diagnostic and excluded from the
legacy manifest rather than treated as route-output differences.

## Correctness and Performance Gates

- Zero Phase 1 parity mismatches for every profile, cap, cache lane, and named topology update.
- Independently compare every precomputed/current edge and raw path cost against Phase 1's rich
  reference edge generator.
- Initial, close, overlapping close, partial reopen, unreachable chokepoint, and full reopen states
  match the reference graph exactly; no stale edge survives a phase boundary.
- Local building update p95 is at most 5 ms on the reference host and worst case is reported.
- On the same matched prebuilt-occupancy, cold-cache, no-bypass, search-backed, full-return lane from
  Phase 1, full returned-path time improves incrementally and cumulative throughput is at least 2x
  the frozen harness-only baseline SHA. Kernel-only timing is diagnostic, not the acceptance metric.
- General corpus/Hellhole non-regression gates in `plan.md` pass.

## Verification

- Exhaustive small-map rich-edge versus table comparisons for each routing profile.
- Phase 1 cold/warm corpus, caps, output hashes, and Dijkstra equality.
- Dynamic topology sequence with overlapping footprints and local update-bound assertions.
- Vehicle radius, pinch, clearance, corner, tree, and diagonal-to-L focused tests.
- Checkpoint, Lab, replay, and derived-state wipe/rebuild equivalence.
- Eleven paired corpus and Hellhole measurements against Phase 1 and the frozen harness-only SHA.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Place and remove overlapping buildings in an infantry corridor and a narrow vehicle turn. Confirm
debug paths change at the same boundary as before, never cut a corner, and reopen only after every
blocking footprint is gone.

## Handoff Expectations

Report profile definitions, table/overlay encoding, exact invalidation radius and mutation list,
memory, local update distributions, parity hashes, cumulative speedup, and remaining hotspots. Tell
Phase 3 how to replace only the infantry base edge cost without duplicating graph ownership, and
mark this phase done.

## Completion Evidence

In the implementation commit, replace this text with the phase revision, verification commands,
corpus/parity hashes, profile/table encoding, exact topology key and update bound, paired
measurements, memory, and the Phase 3 cost-policy seam.
