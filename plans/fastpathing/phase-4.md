# Phase 4 - Vehicle Terrain Routes

## Phase Status

- [x] Rejected and reverted after playtesting. Retained for historical context only.

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
  same Phase 3 route-policy seam. Keep Direct Attack and exact interaction routes `LegacyShape`.
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
- The complete all-profile corpus meets the final matched search-backed terrain, all-profile search-
  backed, and warm targets in `plan.md`; former-direct-bypass vehicle requests are reported
  separately.
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
pathing target; if not, identify the dominant remaining request family for a separately approved
ALT/corridor/hierarchy plan. Mark this phase done.

## Completion Evidence

In the implementation commit, replace this text with the phase revision, verification commands,
vehicle policy/composite-cost contract, corpus/oracle hashes, intentional differences, scenario
artifacts, paired measurements, memory/update results, patch-note staging, and final target status.
