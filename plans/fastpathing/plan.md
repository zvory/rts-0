# Fast Pathing Plan

## Outcome

Phases 1 and 2 are retained: they improve search/storage and precompute the existing route graph
with exact legacy behavior parity. Phases 3 and 4 were implemented, playtested, rejected, and
reverted because terrain-weighted route selection made unit movement worse in practice. The later
Phase 4.5 expansion was reverted with them. The Phase 3 and 4 documents remain only as a record of
the rejected approach and are not ready implementation work.

## Purpose

Make pathfinding materially faster without changing route selection or movement behavior. The
retained result is the behavior-preserving dense search and precomputed-edge work from Phases 1 and
2. The terrain-aware goals below describe the rejected follow-on experiment, not current behavior
or planned implementation.

## Correctness References

The plan deliberately uses two references:

1. **Legacy parity** protects phases that claim to change only implementation. Phases 1 and 2 must
   return coordinate-for-coordinate tile paths, bit-identical world waypoints, identical cache and
   scheduling results, identical capped fallbacks, and identical deterministic replay/snapshot
   output.
2. **A test-only Dijkstra oracle** validates the intended graph rather than current implementation
   details. It operates at the raw tile/state search seam, before world-waypoint conversion,
   diagonal-to-L expansion, tree refinement, or continuous route smoothing. With the production cap
   disabled (or a budget at least the finite state count), a goal-reaching raw path must be edge-
   legal and its integer graph cost must equal Dijkstra's distance to the already-resolved effective
   goal.

For ordinary routes the finite state count is the number of tiles. For direction-sensitive routes it
is tile count times heading count plus the start sentinel. On an exhaustive unreachable query, the
oracle establishes only that the goal is unreachable; the production best-effort path is separately
required to be legal and deterministic.

The Dijkstra oracle does not define capped behavior. Phases 1 and 2 preserve the current procedure
exactly: fallback progress uses legacy geometric octile distance, updates on a successful relaxation
only when that distance strictly decreases, and keeps the first equal-distance candidate; expansion
is incremented before breaking when `expanded > max_expanded`, so the over-cap pop is counted but not
expanded. Terrain phases retain that geometric progress metric independently from the production A*
heuristic; changed edge costs may change the discovered frontier, but the returned endpoint must
still be the documented best discovered candidate under this policy.

## Overall Constraints

- Keep the Rust server authoritative, `Game::tick()` panic-free, and all client input bounded.
- Preserve fog, command validation, body legality, diagonal corner rules, expansion caps, 30 Hz
  movement, the eight search-backed requests per tick, heavy-search deferral, and cold-work
  scheduling charges on cache hits.
- Do not obtain speed by reducing path quality, expansion budgets, request allowances, movement or
  presentation cadence, or route fidelity.
- Preserve the public `Game` API, wire protocol, map schema, and existing terrain multipliers.
- Keep route costs deterministic and integer/fixed-point. Elevation makes the terrain graph directed.
- Separate ordinary infantry, oriented cars, and pivot vehicles when their legality or shaping
  differs, but share tables across entity kinds that tests prove use the same routing profile.
- Keep interaction routes, build/resource landing, tree refinement, vehicle reverse recovery, and
  queued-order semantics outside ordinary Move/Attack Move smoothing unless a phase explicitly
  adopts them.
- Rebuildable graph and search state belongs under `DerivedState` or its existing `PathingService`
  owner and is never serialized. Update `docs/design/server-sim.md` whenever ownership or rebuild
  semantics change.
- Each phase compares against the immediately previous merged phase and the frozen pre-Phase-1
  baseline. A missed gate stops with an attributed profile; it does not authorize ALT, HPA*, CCH,
  flow fields, formation rewrites, or collision changes.
- Semantic and oracle tests may run in CI. Wall-clock thresholds are paired release evidence in the
  PR, not portable hard CI gates.
- Terrain-routing phases stage concise factual patch-note copy before `scripts/agent-pr.sh`.
  Delivery remains opt-in after merge.

## Measurement Contract

Phase 1 adds the smallest deterministic evidence seam needed for later phases:

- a focused versioned request corpus covering cold and ordered warm/cache sequences;
- ordinary and direction-sensitive profiles;
- local, medium, long, blocked-goal, unreachable, equal-cost, diagonal, pinch, tree, clearance,
  distinct formation-slot, and dynamic building cases;
- terrain-rich routes from shipped maps plus the largest shipped map; and
- caps `{0, 1, 2, 8, 64, 4096, 32768}`.

The corpus is an ordered pathing-service event stream, not an unordered request bag. It records
`advance_tick`, cache clear/reset, topology replacement/mutation, request, `allow_pathfinding`,
cache capacity, and default budget using canonical map/building inputs rather than derived occupancy
arrays.

Use existing deterministic scenarios and Hellhole for end-to-end evidence rather than checking in a
large frozen match artifact. Emit two corpus identities: a stable workload hash excluding route
policy and cost-model version, used to pair identical map/topology/start/requested-goal/profile/budget
work across legacy and weighted planners; and a planner-input hash including policy and cost-model
version, used for semantic parity. Emit a compact semantic output hash separately.

For performance, warm each binary once and run 11 paired measurements on the same quiet host,
alternating candidate/baseline order. For each pair compute `candidate / baseline`; report the
median and a one-sided 90% bootstrap upper bound by resampling the 11 pair ratios, not pooled
requests. Record host/CPU policy, binary SHAs, corpus hash, raw JSON, full returned-path latency,
requests/sec, expansions, queue operations, cache outcomes, graph update/finalization time, memory,
Hellhole average/p95/p99, and wall time.

Reject and rerun a measurement set when baseline, candidate, or paired-ratio median absolute
deviation exceeds 2.5% of its median for the path corpus or 5% for Hellhole tick time. Do not use
maximum or pooled p99 as a hard gate.

Unless a phase states a stronger target, performance passes when:

- full returned-path median time is no worse and the paired 90% upper ratio is at most 1.03;
- Hellhole median tick upper ratio is at most 1.03;
- the median of per-run Hellhole p95 ratios is at most 1.05; and
- workload counters and semantic hashes match whenever legacy parity is required.

The complete terrain-aware system must additionally achieve:

- matched search-backed terrain cold full-path median ratio at most 0.50 and paired 90% upper ratio
  at most 0.55 against the frozen Phase 1 harness-only baseline SHA. This lane uses prebuilt
  occupancy, a cold cache, no
  direct bypass, and complete path reconstruction/finalization in both binaries;
- all-profile search-backed cold full-path median ratio at most 0.65 against the frozen Phase 1
  harness-only baseline SHA;
- warm-query median ratio at most 1.00 against that same baseline SHA, with each binary warming its
  own cache from the identical workload sequence even when weighted output geometry differs; and
- final Hellhole median tick upper ratio at most 1.00, with at least 8 of 11 pairs improving.

Former-direct-bypass requests are reported as a separate lane because current main can answer them
in nearly constant time while terrain-aware routing may need search to discover an offset road.
They are not part of the 2x claim, but their command-assignment latency and contribution to the live
Hellhole non-regression gate remain mandatory evidence.

## Phase Summaries

### [Phase 1 - Dense Search Headroom](phase-1.md)

Add a focused parity/performance corpus and raw-graph Dijkstra oracle, then replace tuple-keyed A*
maps with dense generation-stamped scratch. Keep the existing graph, heap order, cap, fallback,
cache, scheduling, and route output exactly unchanged. Proceed only with zero semantic mismatches,
no Hellhole regression, and at least 1.5x full returned-path speed, with 2x as the target.

### [Phase 2 - Precomputed Pathing Edges](phase-2.md)

Precompute current-semantics directed edges per routing profile so neighbor expansion becomes table
access instead of repeated radius, terrain, clearance, pinch, tree, and corner queries. Keep dynamic
building effects in a per-room locally updated overlay rather than rebuilding every profile's full
map for one footprint change. Require exact legacy parity, a positive incremental full-path win, and
at least 2x cumulative cold throughput before terrain behavior changes.

### [Phase 3 - Infantry Terrain Routes](phase-3.md)

Define the authoritative directed fixed-point terrain metric and activate it for ordinary infantry
Move and Attack Move routes. Author cost-preserving body-legal shortcuts once, make every retained
anchor authoritative, and forbid movement from deleting later anchors merely because a direct sweep
is clear. This phase intentionally changes terrain-bearing motion and therefore uses raw-graph
oracle equality, independent final-route recosting, deterministic new-build replays, gameplay
review, and the terrain-rich performance gate rather than legacy route equality.

### [Phase 4 - Vehicle Terrain Routes](phase-4.md)

Extend the same terrain metric and authored-anchor contract to cars and pivot vehicles while
preserving clearance, direction state, turning preference, hull legality, lookahead, and reverse
recovery. Inspect the named vehicle scenarios and classify every legacy difference caused by
removing direct-goal or later-waypoint bypasses. Finish only when the complete all-profile corpus
meets the 2x target and Hellhole remains no slower than the frozen baseline.

## Phase Index

1. [Phase 1 - Dense Search Headroom](phase-1.md)
2. [Phase 2 - Precomputed Pathing Edges](phase-2.md)
3. [Phase 3 - Infantry Terrain Routes](phase-3.md)
4. [Phase 4 - Vehicle Terrain Routes](phase-4.md)

## Explicitly Deferred

- Broad persistent occupancy/navigation snapshots. Phase 2 adds only the graph ownership and local
  invalidation required by measured hot edge evaluation.
- ALT landmarks. Reconsider only if the Phase 3/4 profile shows weighted expansions, rather than
  edge evaluation or smoothing, still dominate. Freeze the final metric before preprocessing.
- HPA/HAA, CRP, CCH, portal corridors, formation sharing, and sector flow fields. A new plan must
  select among them from final per-distance/profile evidence rather than assuming a hierarchy wins.
- Radix/Dial/indexed heaps, JPS, navmesh/funnel conversion, weighted Theta*, Field D*, vehicle state
  lattices, cooperative pathfinding, ORCA, collision redesign, and formation allocation changes.
- Terrain balance changes, temporary ability-field routing, client pathfinding, protocol debug
  payloads, and new map content.

## Implementation Process

A fresh executor agent implements each phase on its own `zvorygin/` branch through the repository's
`phase-runner` workflow. Each PR is owned, pushed with auto-merge armed, and must be definitely
merged with its head reachable from `origin/main` before the next phase starts. Mark the phase
document done in that phase's implementation commit.

After each phase, provide a handoff describing the final design, parity/oracle and performance
evidence, intentional differences, unresolved risks, what the next agent should do, and the core
manual tests. Manual testing should cover the named route and vehicle surfaces, not an exhaustive
matrix.

Correctness failure means the implementation PR must not merge and the phase remains incomplete.
A required performance failure likewise blocks merge and later phases; leave the branch available
with the attributed evidence and request a plan revision rather than merging a regression. Every
completed phase must write durable completion evidence into its phase document: implementation
revision, commands, corpus hash, parity/oracle hash, paired measurements, activation decisions, and
the next-phase seam. Chat or PR commentary alone is not the handoff record.

```bash
scripts/phase-runner.sh --plan fastpathing phase-1 --pr --wait
scripts/phase-runner.sh --plan fastpathing phase-2 --pr --wait
scripts/phase-runner.sh --plan fastpathing phase-3 --pr --wait
scripts/phase-runner.sh --plan fastpathing phase-4 --pr --wait
```
