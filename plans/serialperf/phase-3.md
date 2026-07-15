# Phase 3 - Reuse Spatial Index Storage

## Phase Status

- [x] Done — skipped after scouting; the implementation was not attempted.

## Scout Result

A fresh 900-tick Time Profiler run on `main` at
`93d8a23303f4b262d6be4d5dc2d317823b776bf2` attributed 95/4,932 samples (1.926%) to all
`SpatialIndex::build` work and 43/4,932 samples (0.872%) to allocation beneath those builds.
Immediately preceding Hellhole profiles agreed: total spatial-build cost was 2.107–2.391% and its
allocation cost was 0.832–1.126%. These scouting profiles are not acceptance benchmarks, but they
establish a ceiling far below this phase's required 8% median gain.

The proposed change would reuse allocation while preserving every clear, refill, per-cell sort,
query, and rebuild boundary, so its realizable gain is smaller than the already insufficient total
build cost. That ceiling does not justify high-complexity ownership plumbing through `DerivedState`,
tick phases, and collision passes; Phase 2's rejection also fails this phase's formal precondition.
No Phase 3 candidate was created, and the serial performance plan is closed for archival.

## Objective

Retain and reuse spatial-grid allocation capacity across existing rebuilds without changing when or
how any spatial index is rebuilt.

## Preconditions

- Phases 1 and 2 received initial and final `ACCEPT` verdicts and merged. Any prior rejection stops
  this plan; do not start Phase 3 without explicit user direction.
- Start from current `origin/main`, record its SHA, and read `docs/context/server-sim.md`,
  `game/services/spatial.rs`, `game/systems.rs`, collision index rebuilds, and the `DerivedState`
  registry.
- Follow the common exact-output, nine-pair, independent-review, rejection, PR, and handoff rules
  in `plans/serialperf/plan.md`.

## Work

- Reuse one owned `SpatialIndex` allocation sequentially through pre-command, post-movement,
  pre-collision, every collision pass, and final snapshot-interest construction. Seed the tick with
  the prior rebuildable `DerivedState.final_spatial` allocation and return that same owner as the
  rebuilt final index; clear/refill only after the preceding consumer is finished.
- Preserve every rebuild boundary, the one-tile cell definition, ascending ids within cells, query
  results, collision pair order, and the final index returned for snapshot interest filtering.
- Keep the reused index rebuildable and absent from checkpoints; do not add simultaneous duplicate
  scratch indexes or another persistent cache owner. Retained storage must be
  `O(map tiles + peak live entities)`, not `O(map tiles × historical per-cell occupancy)`.
- Do not introduce incremental spatial updates, dirty sets, changed collision passes, changed query
  shapes, unsafe indexing, global pools, locks, or parallel construction.
- Add focused tests that compare fresh-build and reused-build cells/queries through movement,
  insertion, removal, restore, map-size mismatch, and repeated collision refill. Cover exhaustive
  rectangle/circle/all-id query equality and an adversarial dense group moving through many cells;
  record outer-cell and total inner-id capacity to prove it does not grow with historically visited
  cells, and document the explicit pruning/cap policy. Update the design registry for the changed
  persistent owner.

Expected touch points are `game/services/spatial.rs`, `game/systems.rs`, collision plumbing,
`game/derived_state.rs`, focused tests, and possibly `docs/design/server-sim.md`.

## Exact Output Gate

Generate parent and candidate 900-frame Hellhole streams from separate release builds, require
byte-for-byte equality with `cmp`, and record the identical SHA-256 hash. Run focused spatial,
collision, movement, checkpoint/derived-state tests and the sim architecture check; any ordering or
output difference rejects the candidate.

## Performance Gate

Warm the separate parent and candidate harness binaries twice, then collect nine alternating paired
900-tick runs under `target/server-perf/serialperf-phase-3/`. Report all pairs and median API total,
elapsed/realtime factor, and tick/API tails. This phase necessarily changes `DerivedState`/tick/
collision ownership, so it is high complexity and must clear 8%, eight of nine pairs, and all
common semantic/tail gates. Capture before/after flame graphs and confirm combined inclusive
`SpatialIndex::build`/refill cost fell rather than moving work outside the measured interval.

## Independent Complexity Review

Spawn a fresh read-only subagent after the diff and evidence are ready. Require it to review retained
capacity, ownership/lifetime complexity, query/order parity, tests, SHA-256 and non-timing-field
evidence, before/after flame graphs, and every raw benchmark pair, calculate the result itself, then
return `ACCEPT`, `REJECT`, or one justified `RERUN`.

If the improvement is swallowed by noise or is small relative to the scratch plumbing, reject it.
Do not mark the phase done, push a PR, delete the candidate, start the measured checkpoint, or
broaden the phase into incremental indexes; preserve the local branch/worktree and artifacts, then
return a blocked handoff, stop, and give the user the plain-language inspection report required by
`plan.md`. Failure to spawn a fresh reviewer or obtain a well-formed verdict is also blocked.

## Completion and Handoff

After the initial `ACCEPT`, run and record the pre-plan-versus-final-candidate cumulative benchmark
and fresh profile required by `plan.md`; only then mark this file `Done` and run focused checks.
Follow the common Exact Final-Source Shipping Gate, obtain final review on the post-quality source,
then arm auto-merge, wait, and verify the accepted runtime commit on `origin/main`. Report phase and
cumulative measurements, parity hash, both reviewer verdicts, and remaining distance to 8×. A
rejection stops before shipping or later checkpoint work.

Manual testing focus: run dense mixed-unit movement through the Hellhole center and inspect a short
deterministic replay for unchanged collision resolution and target acquisition.
