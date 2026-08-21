# Phase 1 - Dense Search Headroom

## Phase Status

- [ ] Ready for implementation.

## Objective

Make the existing planner substantially cheaper without changing any route, fallback, scheduling,
or gameplay behavior. Add only the focused evidence needed to prove that equivalence and measure
complete returned paths.

## Work

- Add a compact versioned ordered pathing-service event corpus built from deterministic synthetic and
  shipped-map fixtures. Encode `advance_tick`, cache clear/reset, topology replacement/mutation, and
  request events, including canonical map/building inputs, profile, start, requested goal, radius,
  route shape, optional direct segment, budget, `allow_pathfinding`, cache capacity, default budget,
  tick/request order, and cache boundaries. Independently test the documented nearest-passable goal
  resolution before the oracle comparison; the product receives `requested_goal`, while the semantic
  manifest records `expected_effective_goal`. Do not store derived occupancy arrays.
- Emit stable input and semantic hashes. Compare resolved/deferred outcome, effective goal, exact
  tile coordinates, world-waypoint `f32` bits, direct/search classification, cache result, expansions,
  scheduling work, budget exhaustion, path length, request resolution tick, vehicle diagonal-to-L
  expansion, and tree-refinement fallback.
- Add a release path-query mode that measures complete request/finalization results and reports
  per-lane counters. Reuse the existing Hellhole harness for whole-tick evidence; add only lightweight
  aggregate pathing counters when they do not perturb scheduling.
- Land the harness and hashed-planner parity/oracle fixtures first within the phase branch and record
  that commit SHA as the frozen baseline before changing search storage. Build that preserved parent
  commit for every later paired baseline measurement; do not compare against pre-harness current main
  or regenerate the baseline after dense search lands.
- Add a deliberately simple test-only dense Dijkstra solver at the raw tile/state graph seam. Run it
  with the production cap disabled or a budget at least the finite state count; use `u64` distance,
  independently generate legal edges, resolve the goal before search, reconstruct the raw path, and
  independently recost it.
- Keep capped and unreachable legacy behavior under exact product-versus-reference tests. Preserve
  fallback updates on successful relaxation when legacy octile distance strictly improves, first
  equal candidate wins, and the increment-before-`expanded > budget` convention.
- Replace `HashMap<(x, y, direction), ...>` cost/parent state with flat tile-indexed arrays and
  generation stamps. Ordinary zero-turn-cost routes use one state per tile; direction-sensitive
  routes lazily use `8N + 1` states with a dedicated start sentinel (or an equivalently explicit
  `9N` encoding). Do not seed eight first moves as a parity shortcut.
- Reuse allocations across sequential requests and define deterministic generation-wrap handling.
- Retain the current `BinaryHeap`, full `(f, g, ty, tx, dir)` order, neighbor order, stale-entry
  behavior, arithmetic, cap, best-key rule, nearest-passable order, reconstruction, and diagnostics.
- Keep the hashed implementation only as a test reference during the transition, then remove it from
  production before completing the phase.

## Expected Touch Points

- `server/crates/sim/src/game/pathfinding.rs`
- focused pathfinding/pathing-service test modules and small fixtures
- `server/crates/sim/src/game/services/pathing.rs`
- `server/src/tools/hellhole_perf_harness.rs` only for inert aggregate counters
- a focused server tool or mode for full-path corpus measurements
- `docs/design/testing.md` if the canonical harness contract changes

## Identity Requirement

Require exact output and diagnostic parity for cold queries and ordered warm/cache sequences,
including caps `{0, 1, 2, 8, 64, 4096, 32768}`. Cache eviction, cold-work scheduling charges,
eight-request allowance, heavy-search deferral, and the tick each unit leaves `AwaitingPath` must be
unchanged. Generated Hellhole semantic output, action/death/respawn counters, and deterministic replay
must remain identical.

## Correctness and Performance Gates

- Zero legacy parity mismatches across ordinary, vehicle, blocked, unreachable, equal-cost, dynamic,
  cache, and capped cases.
- In oracle mode, every goal-reaching raw path is edge-legal and has integer graph cost equal to the
  Dijkstra distance. Exhaustive unreachable cases are oracle-unreachable and retain the documented
  legal deterministic legacy fallback.
- Retained scratch memory is bounded and reported for 126x126 and 196x196 normal/direction profiles.
- On the matched lane with prebuilt occupancy, cold cache, direct bypass disabled, search-backed
  requests only, and complete reconstruction/finalization, full returned-path median time improves
  by at least 1.5x against the frozen harness-only SHA; 2x is the target. If it misses 1.5x, stop and
  profile rather than adding a different queue or hierarchy.
- The general Hellhole and p95 non-regression gates in `plan.md` pass.

## Verification

- Exhaustive/seeded small-map hashed-versus-dense comparisons.
- Focused corpus cold/warm semantic comparison and capped goldens.
- Raw-graph Dijkstra cost equality with the production cap disabled.
- Generation wrap, blocked goal, unreachable, diagonal corner, pinch, and direction-state tests.
- Existing pathing, cache, vehicle, movement, replay, and derived-state rebuild tests.
- Eleven paired release corpus and Hellhole measurements following `plan.md`.
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

No gameplay change is expected. Inspect one corpus failure report format and one infantry, Scout
Car, and Tank debug path to confirm any future mismatch will identify the first differing request,
path coordinate, diagnostic, and resolution tick.

## Handoff Expectations

Report the corpus/version hash, oracle boundary, dense layouts, generation-wrap policy, actual
scratch memory, exact parity hashes, full-path and Hellhole ratios, and remaining hot functions.
Tell Phase 2 how much time remains in rich passability/cost evaluation, and mark this phase done.

## Completion Evidence

In the implementation commit, replace this text with the phase revision, verification commands,
corpus and parity/oracle hashes, paired release measurements, memory result, and the exact Phase 2
graph seam.
