# Phase 2 - Precomputed Pathing Edges

## Phase Status

- [x] Implemented manually and retained after passing correctness and performance gates.

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

Implemented manually without the phase runner. The implementation revision is `696ebdb9b`; raw
paired measurements are checked in as `phase-2-results.json`.

- The graph has three proven current profiles: infantry-like normal, vehicle-body normal, and
  oriented vehicle-clearance/pinch, with radius retained as a profile parameter. Each 256-tile COW
  page stores one passability byte and eight legacy-direction `u32` extra-cost entries per tile;
  `u32::MAX` is the illegal-edge sentinel. Base step and turn costs stay outside the table, keeping
  the legacy saturating-add and heap/tie order. On the 126x126 measurement map, each profile uses
  523,908 immutable bytes and 525,893 dynamic bytes. Three-profile initialization median was
  14.824 ms.
- The authored-map key is the materialized content hash covering dimensions, terrain/roads,
  elevation, doodads/trees, no-vehicle and slow tiles, and the remaining overlays. The exact ordered
  dynamic topology key is `(id, kind, pos_x_bits, pos_y_bits)`; owner, completion, and moving units
  are excluded. Changed blocker tiles recompute edge origins within
  `max(radius + 2, vehicle-clearance ? 4 : 2)` tiles. Across 202 updates/run, median p50/p95 were
  0.0279/0.0450 ms and the observed worst case was 0.2190 ms. Exhaustive current-table versus rich
  reference checks cover initial close, overlap, partial/full reopen, all profiles, and caps
  `{0,1,2,8,64,4096,32768}`.
- Eleven alternating rich, prebuilt-occupancy, cold, no-bypass, full-return pairs kept semantic hash
  `2d18e58c0dbb5782` and 469,321 expansions for all 240 requests. Phase 1 rich evaluation median was
  143.440 ms versus 51.622 ms candidate: 2.779x faster, paired median ratio 0.3595, bootstrap 90%
  upper ratio 0.3607. The frozen Phase 1 harness-only revision did not contain a rich
  `PathingService` lane (its release benchmark used a generic `Grid`), so the immediately preceding
  Phase 1 rich generator is retained in the candidate binary as the matched reference. The 2.779x
  incremental gain already exceeds the plan's 2x cumulative threshold without taking credit for
  Phase 1's separate dense-search gain.
- Eleven alternating 900-tick Hellhole pairs against merged Phase 1 produced median average-tick
  ratio 0.8948 with bootstrap 90% upper ratio 0.9272; median p95 ratio was 0.8502 and all 11 pairs
  improved. Every semantic counter matched. The final 900-frame artifact is byte-identical to Phase
  1: 26,910,978 bytes, SHA-256
  `7054deb3347441de285e252a8275de8e8eb2ca09e84795e25f1d121983e168e2`.
- Verification: exhaustive graph/reference and topology tests; complete `rts-sim` suite (1,316
  passed, 5 release-only ignored); `cargo clippy -p rts-sim --lib -- -D warnings`; sim architecture
  check; docs health; and `git diff --check`. Release sampling attributes the remaining Hellhole hot
  work to vehicle movement/static-clearance checks (`vehicle_route_context`, `static_clearance_px`,
  and `body_hits_static_blocker`), not path edge evaluation.
- Phase 3 should change only the infantry profile's base `extra_costs` policy. Keep `PathGraph`
  ownership, dynamic pages, topology invalidation, dense state indices, and cache fingerprinting
  unchanged; directional elevation can populate directed infantry base costs without duplicating
  graph state.
