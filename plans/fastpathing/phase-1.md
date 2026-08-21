# Phase 1 - Dense Search Headroom

## Phase Status

- [x] Implemented and retained after passing the correctness and performance gates.

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

Implemented manually without the phase runner or a PR. The frozen harness/reference revision is
`e7ef438051bcdad2449823915a1e30bc99c9ca15`; the dense-search implementation revision is
`57a91c48f6aff1142559407671df39c69e78ed82`. Raw paired measurements are checked in beside this
document as `phase-1-results.json`.

- Corpus v1 full workload hash: `23c2ab3020408f49`; all-cap semantic hash:
  `e017b84139cd07db`. The release cold/search-backed lane hash is `9393917eb369f14a` and its semantic
  hash is `d751fe462ad18702`. Baseline and candidate match on every path coordinate, world-waypoint
  bit, expansion count, cap outcome, and benchmark output accumulator.
- The independent test-only Dijkstra oracle resolves the effective goal first and compares raw
  tile/state graph cost before waypoint conversion. Uncapped ordinary and direction-sensitive goal
  paths equal the oracle cost. Twenty-four seeded map families across both profiles and caps
  `{0, 1, 2, 8, 64, 4096, 32768}` match the retained tuple-`HashMap` implementation exactly.
- Eleven alternating release pairs on an Apple M5 Pro produced 477.015 ms baseline versus
  246.157 ms candidate median for 240 full reconstructed/converted cold requests: 1.938x faster,
  paired median ratio 0.5160, one-sided bootstrap 90% upper ratio 0.5179. Baseline/candidate/ratio
  MAD were 0.345%/0.245%/0.352%.
- Eleven alternating 900-tick Hellhole pairs produced a median average tick ratio of 0.9457 with a
  90% upper ratio of 0.9495; median p95 ratio was 0.9285. All 11 pairs improved. The generated
  900-frame artifact was byte-identical (26,910,978 bytes, SHA-256
  `7054deb3347441de285e252a8275de8e8eb2ca09e84795e25f1d121983e168e2`), and action, projectile,
  death, shuttle, respawn, entity, and serialized-payload counters matched in every pair.
- Dense retained scratch is 12 bytes/tile for ordinary searches and 96 bytes/tile plus a 12-byte
  start sentinel for direction-sensitive searches. That is 190,512/1,524,108 bytes at 126x126 and
  460,992/3,687,948 bytes at 196x196 when each profile is retained independently. Generation wrap
  clears stamps once and restarts at generation 1; normal requests do not clear the arrays.
- Phase 2 should keep `Passability::dimensions()` and `SearchScratch` as the seam: replace repeated
  `Passability::passable`/`movement_cost` neighbor evaluation with profile edge-table lookup while
  leaving dense state indices, heap entries/order, and reconstruction unchanged.

Verification: the focused corpus/parity/oracle/wrap/memory tests; the full `rts-sim` nextest suite;
`cargo clippy -p rts-sim --lib -- -D warnings`; sim architecture check; docs health; and
`git diff --check`.
