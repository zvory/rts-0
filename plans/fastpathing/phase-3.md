# Phase 3 - Infantry Terrain Routes

## Phase Status

- [x] Implementation complete.
- [ ] Performance acceptance complete: the Hellhole and finalization gates pass, but the matched
  infantry terrain full-path ratio misses the phase's 0.50 median / 0.55 upper target.

## Objective

Enable faster-time road, slow-terrain, and elevation-aware routing for ordinary infantry Move and
Attack Move orders, then preserve that strategic route through one-time smoothing and explicit
anchor-following movement.

## Authoritative Terrain Metric

- Define one directed fixed-point edge travel-time cost derived from the existing cardinal/diagonal
  distance and road, slow-tile, and uphill/downhill speed ratios. Add no new balance values.
- Document multiplier composition, rounding, equality, boundary ownership, and direction. Road plus
  slow overlay must match its existing multiplicative tick-speed behavior; reversing an elevation
  edge may produce a different cost.
- Factor out unit base speed only when mathematically valid for route comparison. Ignore temporary
  ability/status fields in static path choice.
- Retain tree avoidance and explicit nonnegative shaping separately but in the production route
  objective where they already affect choice.
- Use a weighted admissible geometric heuristic scaled from the minimum attainable cardinal and
  diagonal per-distance costs, or zero when no tighter proof is available. Do not reuse 10/14
  octile after road discounts without an admissibility proof.
- Expose one shared deterministic `RouteCostModel` with both raw `edge_cost(tile, tile)` and
  continuous `segment_cost(world, world)`. `segment_cost` traverses exact tile boundaries; finalizer
  comparisons recost both the direct candidate and retained world-space polyline with this function
  rather than comparing a continuous segment to accumulated grid-edge cost.
- An anchor introduced by a non-time shaping penalty such as tree avoidance remains protected unless
  `segment_cost` represents that same penalty. Terrain-time equality alone cannot erase an anchor
  whose graph purpose is absent from the continuous model.

## Route Policy

- Add an explicit policy such as `LegacyShape` versus `FastestTerrainTime` to `PathRequest`, graph
  profile lookup, cache key, corpus events, and Dijkstra fixtures.
- Phase 3 assigns `FastestTerrainTime` only to ordinary infantry Move and Attack Move. Direct Attack,
  gather, build, repair, deconstruct, abilities, and other interaction routes remain `LegacyShape`.
- `StaticRouteAnalyzer` uses `FastestTerrainTime` after rollout. Footprint/build/deconstruct and all
  interaction helpers use `LegacyShape`; every test/helper `PathRequest` constructor specifies a
  policy explicitly so compilation forces any future caller to choose.
- Keep distinct policy-indexed cost planes: shared legality may be common, but `LegacyShape` retains
  Phase 2 costs while `FastestTerrainTime` selects the new terrain-time costs. Activating weighted
  infantry must not mutate interaction-route scoring.
- Store the assigned path policy alongside authoritative serialized movement path state, with a
  missing-field default of `LegacyShape`. Change it atomically only when a path is assigned/replaced
  or actually cleared: deferred/failed repaths preserve the existing path and policy unless current
  behavior clears both; consuming the final waypoint or explicitly clearing the path resets policy
  to `LegacyShape`; vehicle recovery waypoints preserve it. Checkpoint/Lab restore deserializes the
  stored policy, and derived-state rebuild never changes it.

## Authored-Route and Runtime Contract

- Raw A* produces an optimal tile route under the graph objective. One-time finalization may replace
  a raw subpath with a continuous segment only when an independent deterministic tile-boundary
  traversal proves the full-body sweep legal and `segment_cost` says the direct candidate costs no
  more than the retained raw world-space polyline.
- Finalization uses prefix costs and a bounded candidate scan; it does not perform fixed-eight-pixel
  terrain sampling or repeatedly integrate the whole remaining route.
- Every waypoint left after finalization is an authoritative anchor. For `FastestTerrainTime`,
  disable the 30 Hz greedy multi-waypoint skip. Preserve ordinary arrival/crossed-current-waypoint
  popping, stuck detection, and repathing, but movement may neither pop nor select a target beyond
  the current authored waypoint merely because a later sweep or final goal is clear.
- Do not add new corridor reacquisition in this phase. If displacement makes the current authored
  waypoint unreachable through existing movement/recovery rules, use the existing blocked debounce
  and transition to `AwaitingPath` for a new route.
- Weighted ordinary Move/Attack Move routes do not use the old clear-line direct bypass. A future
  bypass requires a body-legal segment whose cost equals a proven global lower bound; this phase
  simply runs the optimized search.
- Interaction, build, gather, repair, deconstruct, and ability landing routes retain their current
  policy unless a focused test proves they are ordinary movement.

## Work

- Freeze and document the metric before changing production behavior; extend the Phase 1 Dijkstra
  edge generator and Phase 2 infantry table from that same seam.
- Activate the directed terrain costs for eligible infantry routes.
- Extend `FastestTerrainTime` cache identity with route policy plus a deterministic terrain-cost
  revision/fingerprint covering roads, slow overlays, elevation, and every cost-bearing authored
  layer. Retain the Phase 2 content blocker fingerprint separately; never use monotonic graph
  generation as a cache key.
- Add one-time body-sweep plus continuous cost finalization and raw-prefix recosting.
- Replace the runtime infantry next-next visibility skip with the authored-anchor contract above.
- Keep fallback progress keyed to legacy geometric octile distance, not the weighted production
  heuristic. Preserve first-strict-improvement and cap/count behavior.
- Update source-of-truth pathing and terrain-composition documentation.
- Stage concise factual player-facing patch-note copy.

## Expected Touch Points

- `server/crates/rules/src/terrain.rs`
- Phase 2 graph/profile construction
- `server/crates/sim/src/game/services/pathing/request.rs`
- `server/crates/sim/src/game/services/pathing/route_finalize.rs`
- `server/crates/sim/src/game/services/pathing/authoring.rs`
- a focused route-cost/finalization helper
- `server/crates/sim/src/game/services/movement/waypoints.rs`
- serialized/defaulted movement-path policy state and its assignment/clear seams
- focused movement/order tests and terrain-rich fixtures
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Intentional Behavior Boundary

Terrain-bearing raw paths, authored routes, movement trajectories, snapshots, and replays may differ
from legacy main. On terrain-neutral unique-optimum raw searches, Phase 2 path coordinates must
remain identical; equal-cost geometry may differ only if the cost formula creates a documented tie.
Repeated runs of the new build must remain byte-deterministic, and every difference must be
attributable to the written metric or anchor-following contract.

## Correctness and Performance Gates

- In oracle mode, the raw pre-finalization tile path reaches the effective goal, is edge-legal, and
  has graph cost exactly equal to Dijkstra's distance.
- The finalized continuous route is independently body-checked and recosted with `segment_cost` and
  costs no more than the raw world-space route recosted with that same function. Every removed span
  satisfies this invariant separately.
- Exhaustive unreachable cases are oracle-unreachable. Capped paths use the documented geometric
  progress metric, first-strict-improvement rule, and cap/count convention; differences are limited
  to the changed weighted frontier and are enumerated.
- A shallow forest is bypassed only when the detour is faster; a sufficiently wide forest is crossed
  when that is faster. An offset road and both elevation directions match the metric.
- Smoothing/finalization time is at most 10% of total path-request time and is paid once, not at 30 Hz.
- Infantry matched search-backed terrain cold full-path median ratio is at most 0.50 and paired upper
  ratio at most 0.55 against the frozen harness-only SHA; all-profile search-backed and warm gates in
  `plan.md` pass for infantry lanes. Report former-direct-bypass requests separately.
- Hellhole tick upper ratio is at most 1.03 against both Phase 2 and the frozen harness-only SHA. Budget
  exhaustion/deferral increases must be fully attributable to newly search-backed weighted requests;
  the eight-request allowance and heavy threshold stay unchanged, and 40 non-heavy requests resolve
  within at most `ceil(40 / 8) = 5` ticks.

## Verification

- Raw-graph Dijkstra equality with the production cap disabled or exhaustive budget.
- Independent continuous final-route recost and body-legality/property tests.
- Terrain-neutral unique-optimum parity and deterministic new-build replay comparisons.
- Shallow/wide forest, offset road, road-plus-slow, uphill/downhill reverse, dynamic building, tree,
  and exact interaction-route tests.
- Frequent 40-unit formation movement with distinct goals and unchanged scheduling allowances.
- Eleven paired terrain corpus and Hellhole runs against Phase 2 and the frozen harness-only SHA.
- Focused `rts-sim` pathing/movement tests and simulation architecture check.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Use debug paths and arrival behavior for: a forest where going around wins, a forest where crossing
wins, a parallel offset road, road under slow overlay, a route reversed across elevation, a 40-unit
formation move, and a building closed/reopened across an active route. Confirm motion remains smooth
without tile-center locking and exact interaction orders still land correctly.

## Handoff Expectations

Report the metric and rounding, edge/segment recost relationship, exact anchor-pop and displacement
rules, intentional legacy differences, oracle/replay evidence, terrain/full-corpus ratios, Hellhole
ratios, and staged patch-note text. Tell Phase 4 which finalization and movement rules are shared
versus infantry-specific, and mark this phase done.

## Completion Evidence

Implemented manually without the phase runner or a PR, based on Phase 2 revision
`e6afd1e6b0ccd57d93ae2caffa8946ad169603b7`. Raw paired measurements are checked in as
`phase-3-results.json`.

- The shared directed metric scales cardinal/diagonal distance `10/14` by `780`. Source-tile road
  and slow ratios and source-to-destination elevation multiply as exact rationals before one ceiling
  division. Level-open/road/slow/road-plus-slow/uphill/downhill cardinal costs are respectively
  `7800/5200/10400/6934/9750/6000`. Continuous recost walks exact half-open tile boundaries at
  1/1024-pixel distance precision and uses the same ratios. Non-time tree shaping stays separately
  protected.
- `FastestTerrainTime` applies only to ordinary-infantry Move and Attack Move. All interactions and
  vehicles keep `LegacyShape`. Policy is part of graph/cache identity and serialized movement state,
  defaults to legacy for old state, changes atomically with path assignment, survives deferred
  repaths/recovery insertion, and resets when the final waypoint is consumed or the path is cleared.
- Raw weighted A* matches Dijkstra exactly on directed road/slow/elevation fixtures. One-time
  finalization independently recosts retained and direct spans, conservatively body-checks each
  removal, bounds its candidate scan, and protects tree anchors. Weighted movement consumes only the
  current authored anchor; it does not use the old direct bypass or the 30 Hz next-next skip. A
  collision-displaced unit may consume the current anchor only after crossing its outgoing plane
  within the half-tile route corridor. Existing blocked debounce/repath remains the displacement
  recovery rule. The completed snaking-corridor regression clears four Machine Gunners in 2,919
  ticks versus its 2,927-tick baseline.
- The 240-request paired terrain lane returned stable reference/candidate hashes
  `5d5ae97dd3787ed5` / `0bf4a0514afe44a7` and 230,320 / 229,556 expansions. Eleven alternating runs
  measured 31.263 ms reference versus 21.433 ms candidate: 1.459x faster, paired median ratio
  0.6796, bootstrap 90% upper ratio 0.6848. This misses the required 0.50/0.55 gate. The frozen
  Phase 1 harness-only revision lacks an equivalent rich `PathingService` lane, so the same-binary
  retained rich reference is the only exact matched comparison. Finalization was 9.43% of candidate
  request time, passing its 10% gate.
- Eleven alternating 900-tick Hellhole pairs against Phase 2 measured a 1.0103 median tick ratio and
  1.0127 bootstrap 90% upper ratio, passing the 1.03 phase gate; median p95 ratio was 1.0102 and its
  upper ratio 1.0138, below 1.05. Wall time ratio was 0.9958 with 10/11 pairs faster, while tick CPU
  improved in 1/11 pairs. Candidate runs were internally byte-deterministic: both 900-frame streams
  were 26,981,493 bytes with SHA-256
  `1c97f9e35142c1c46ab7d4138515c75c4c54db94e99c16bb4dc3e21074ea734c`.
- Phase 2 and Phase 3 Hellhole streams intentionally differ because terrain-bearing infantry routes,
  trajectories, and combat timing are allowed to change. The Phase 2 stream remains 26,910,978
  bytes with SHA-256 `7054deb3347441de285e252a8275de8e8eb2ca09e84795e25f1d121983e168e2`;
  candidate totals were 17,824 attacks, 118 projectiles, 807 deaths, and 807 respawns, identically in
  every candidate run.
- Forty distinct non-heavy infantry requests resolve inside the unchanged allowance of eight
  searches per tick: all 40 are assigned within exactly five ticks. Verification covers the full
  `rts-sim` suite, `rts-rules`, Clippy with warnings denied, simulation architecture, docs health,
  and diff whitespace checks.
- The staged player note says: “Infantry Move and Attack Move orders now choose faster routes using
  roads, slow terrain, and elevation, and follow the authored route without skipping strategic
  anchors.” It has not been delivered.
- Phase 4 should reuse the metric, policy-indexed graph/cache planes, continuous finalizer, serialized
  policy, and authored-anchor runtime rule. It must opt car and pivot-vehicle request seams into
  `FastestTerrainTime` only after preserving oriented clearance, turn costs, hull legality,
  lookahead, and reverse recovery; recovery waypoint insertion already preserves the assigned
  policy.
