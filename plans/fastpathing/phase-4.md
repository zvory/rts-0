# Phase 4 - Vehicle Terrain Routes

## Phase Status

- [x] Complete on `zvorygin/fastpathing-phase4` (based on Phase 3 revision `cfbe6ddea`).

## Objective

Extend Phase 3's terrain metric and authored-anchor contract to cars and pivot vehicles while
preserving vehicle clearance, direction state, turn preference, hull legality, lookahead, and
reverse recovery.

## Design Constraints

- Reuse the authoritative terrain-time seam; do not create a vehicle-specific interpretation of
  roads, slow overlays, or elevation.
- Retain incoming-direction state and turn costs wherever they are semantic. Preserve hard
  clearance, diagonal pinch, vehicle corner penalties, diagonal-to-L expansion, current-hull sweep
  legality, Scout Car segment limits where still required, and deterministic reverse recovery.
- A terrain-faster shortcut cannot override clearance or turn shaping. Finalization must compare the
  documented composite objective or use a conservative bound proven not to worsen it.
- Anchors introduced by clearance, corner, turn, diagonal-to-L, or recovery shaping remain protected
  unless the continuous comparator explicitly represents the same constraint/cost.
- Every retained vehicle waypoint is an authored anchor. Steering may inspect later authored
  segments for orientation, but neither `route_accepts_waypoint` nor direct-goal logic may pop or
  select a movement target beyond the current authored waypoint merely because the final goal is
  visible.
- A vehicle may consume its current anchor through the existing proximity/passed-waypoint guards
  only when the immediate next join is hull-legal. Recovery may insert a bounded temporary waypoint,
  then must resume the same authored path or request a repath; it may not scan or target farther
  anchors.
- Do not add a global state lattice, nonholonomic planner, collision redesign, formation allocator,
  or new heading-expanded landmark tables.

## Work

- Extend Phase 2 car and pivot profile edge costs with Phase 3's directed terrain metric while
  retaining clearance/corner/turn shaping.
- Assign and persist `FastestTerrainTime` for eligible vehicle Move and Attack Move paths through the
  same Phase 3 route-policy seam. Keep Direct Attack and exact interaction routes `LegacyShape` only
  for this vehicle-motion phase; Phase 4.5 converts every production ground order after vehicle
  legality and recovery are proven.
- Extend raw-graph Dijkstra coverage to the exact direction-state/composite objective.
- Move eligible vehicle simplification to one-time authored finalization with deterministic terrain
  recost and hull/body legality.
- Constrain vehicle route-context lookahead, later-waypoint acceptance, and direct-goal decisions to
  the authored adjacent corridor.
- Audit tree refinement, diagonal-to-L waypoints, Scout Car three-tile handling, pivot turns, reverse
  waypoints, blocked debounce, and interaction routes.
- Keep fallback progress geometric and independent from the weighted A* heuristic.
- Update the final all-profile contract in source-of-truth docs and stage the vehicle-facing patch
  note.

## Expected Touch Points

- Phase 2 vehicle graph/profile construction
- `server/crates/sim/src/game/services/pathing/route_finalize.rs`
- `server/crates/sim/src/game/services/movement/vehicle_route.rs`
- pivot/car motion and recovery helpers and tests
- dev scenario fixtures only when existing fixtures cannot express the required cases
- `docs/design/server-sim.md`
- `docs/design/balance.md`

## Intentional Behavior Boundary

Terrain-bearing vehicle routes and trajectories may differ from legacy main. Terrain-neutral unique-
optimum raw paths should remain equal to Phase 2; any finalization/lookahead difference must be an
enumerated consequence of replacing a terrain-blind bypass with authored anchors. Repeated new-build
runs must remain byte-deterministic.

## Correctness and Performance Gates

- In oracle mode, every goal-reaching raw direction-state path is legal and has composite graph cost
  exactly equal to Dijkstra's distance. Exhaustive unreachable and capped fallback behavior obeys the
  Phase 3 contract.
- Every final continuous segment passes independent current-hull/body legality and non-worsening
  composite `segment_cost` checks against the retained world-space polyline.
- No new corner clip, narrow-gap penetration, reverse oscillation, forest lock, turn stall, or
  order-clearing path failure occurs in the named scenarios.
- Budget exhaustion/deferral increases must be fully attributable to newly search-backed weighted
  vehicle requests; the request allowance and heavy threshold remain unchanged, and a 40-request
  non-heavy batch resolves within five ticks.
- The complete Move/Attack Move body-profile corpus meets the matched search-backed terrain,
  all-profile search-backed, and warm targets in `plan.md`; former-direct-bypass vehicle requests
  are reported separately. Phase 4.5 owns the final all-order corpus.
- Final Hellhole median tick upper ratio is at most 1.00 against the frozen harness-only SHA, at least
  8 of 11 pairs improve, and p95 remains within the plan's bound.

If correctness passes but performance misses, stop with per-profile, distance, expansion, edge,
queue, finalization, and update attribution. Do not add ALT or hierarchy inside this phase.

## Verification

- Raw direction-state Dijkstra equality and independent final-route recosting.
- Terrain-neutral unique-optimum parity and deterministic new-build replay comparisons.
- Dynamic close/overlap/reopen sequences for car and pivot profiles.
- Focused Scout Car, Tank, Anti-Tank Gun, tree, corner, clearance, diagonal-to-L, and reverse tests.
- Interact review of `scout_car_open_ground_l_path`, `scout_car_lake_reverse_l_path`,
  `replay_303_scout_car_forest_lock`, representative narrow tank turns, and terrain routes.
- Eleven paired full-corpus and Hellhole runs against Phase 3 and the frozen harness-only SHA.
- Relevant `cargo nextest` subset and simulation architecture check.
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Inspect a Scout Car joining/leaving a road, reversing through an L route, and recovering after a
building closure; inspect a Tank pivoting through a narrow turn and both vehicle types choosing
around versus through slow terrain in both elevation directions. Confirm the movement order remains
active through bounded recovery and the vehicle never escapes the authored corridor by targeting a
visible later anchor.

## Handoff Expectations

Report the vehicle composite cost and shaping contract, exact anchor/lookahead/recovery rules, all
intentional legacy differences, scenario artifacts, oracle/replay evidence, memory/update costs,
final paired performance ratios, and staged patch-note text. State whether the plan met the 2x
pathing target; if not, identify the dominant remaining request family. Hand Phase 4.5 the proven
car/pivot policy, finalizer, anchor, lookahead, and recovery seams needed to convert Direct Attack
and every exact interaction route without weakening vehicle legality. Mark this phase done.

## Completion Evidence

- Vehicle Move and Attack Move now persist `FastestTerrainTime`; Direct Attack and exact interaction
  routes remain `LegacyShape` for Phase 4.5. Vehicle edges combine the Phase 3 directed terrain-time
  metric with scaled clearance, corner, and incoming-direction turn shaping. Diagonal-to-L elbows
  use that same directed composite cost.
- Finalization is one-time and cached by exact endpoints, route/profile policy, blocker fingerprint,
  and raw route. It removes only provably equal-cost collinear spans, retains bends and authored
  clearance/turn/recovery anchors, applies the Scout Car three-tile limit, and requires hull-legal
  joins. Motion consumes at most one adjacent authored anchor per evaluation; it cannot bypass to a
  visible final goal or later waypoint. Bounded recovery resumes the same path or repaths.
- Oracle and regression coverage includes exact direction-state Dijkstra equality for Scout Car,
  Tank, and Anti-Tank Gun profiles, precomputed/direct terrain-cost equality, finalizer legality,
  anchor/lookahead limits, 40-request scheduling, the vehicle movement suite, and deterministic
  replay 281/303 cases. `cargo test --manifest-path server/Cargo.toml -p rts-sim --lib` passed
  1,334 tests (7 ignored); clippy with `-D warnings` and `rts-archcheck` passed.
- Eleven all-profile release samples (960 cold and 15,360 warm requests per sample) produced stable
  reference/candidate hashes `9a5b92a0d69cc0e5` / `a91153a2991dae45`. Cold median ratio was
  0.29156 (90% bootstrap upper 0.29287, 11/11 improved); warm median ratio was 0.23206 (upper
  0.23599). Candidate MAD was 0.43% cold and 2.46% warm. Finalization was 3.28% of candidate cold
  time. Graph initialization median was 7.55 ms; base/dynamic graph storage was 1,047,816 / 1,051,786
  bytes. This clears the 2x pathing target.
- Eleven Hellhole pairs against Phase 3 improved average tick CPU in 11/11 runs: medians 9,351 us to
  8,673 us, ratio 0.92490 (90% upper 0.92551). p95 ratio was 0.97716 and p99 ratio 0.96219. Harness
  wall-time ratio was 1.02312 because the new vehicle trajectories intentionally changed battle and
  snapshot work. Eleven additional pairs against frozen pre-Phase-1 revision `e7ef438051` improved
  average tick CPU from 10,774 us to 8,647 us: ratio 0.80072 (90% upper 0.80210), with 11/11
  improving. Frozen-baseline p95/p99/wall ratios were 0.78608/0.71417/0.92860. Two candidate
  900-tick snapshot streams were byte-identical at 24,899,122 bytes,
  SHA-256 `81d4b7488f7011e6a34c79b1a45332919ef58abcf58e617e7bb5e371cdc78d95`.
- Phase 3 and Phase 4 outputs intentionally differ on terrain-bearing vehicle routes: Hellhole
  snapshot bytes changed from 27,723,460 to 26,833,202 and event totals changed with combat timing.
  Repeated Phase 4 runs retained identical snapshot and event totals. Full measurement rows are in
  `phase-4-results.json`.
- Interact review covered the open-ground L completion and an active lake reverse-L route without
  order loss or recovery. Patch-note copy was staged locally and not delivered.
