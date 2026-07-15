# Phase 1 - Make Measurement Cheap and Representative

## Phase Status

- [ ] Not started.

## Objective

Remove high-frequency diagnostic bookkeeping from renderer inner loops while preserving the same
per-frame and report-window counter totals. Restore the existing `supply-300-active` workload to a
passing, server-authoritative setup so later performance claims can distinguish the full-world
client ceiling from normal active-player behavior. Recapture both lanes before changing fog or rig
execution.

## Why This Is First

`Renderer._recordRenderDiagnostic` accounts for 5.5% self CPU and
`FrameProfiler.recordDiagnosticCounter` adds another 1.2%. The renderer currently reports hot rig
events one part at a time: the baseline records roughly 13,214 route-hidden skips and 1,742 redraw
attempts every frame, each flowing through label normalization and multiple aggregation maps.
Reducing that observer cost makes the Phase 2 and Phase 3 profiles more trustworthy and improves
ordinary profiled frames directly.

The required active-player comparison currently times out after 30 seconds before its setup
assertion observes the exact scenario, player, prediction, supply, cap, and entity composition.
Both debug and explicitly prebuilt release-server attempts failed the same way; the partial profile
must not be used as baseline evidence.

## Scope

### Batch renderer diagnostics

- Add a bounded bulk-counter path to `FrameProfiler` that accepts already-normalized label totals
  for the current frame and updates the ordinary, report-window, and active-frame aggregates with
  the same clamping rules as individual counters.
- Give the renderer a reusable per-frame diagnostic accumulator. Hot rig, PNG rig, redraw, graphics
  clear, display-object reuse, and entity/category paths should increment numeric fields or a
  bounded known-label table and flush each nonzero label once near the end of the renderer update.
- Keep the existing single-counter API for rare events and external callers. Do not replace
  exception boundaries, remove failed-draw counters, or concatenate unbounded entity ids into
  labels.
- Avoid allocating a fresh closure, string, array, or `Map` entry for every part merely to count a
  known event. A per-frame flush must retain exact totals, including `maxFrame` and
  `avgActiveFrame`, rather than sampling or dropping counters.
- Ensure a failed renderer update still flushes or safely discards one bounded accumulator without
  leaking counts into the next frame.

### Restore the active comparison lane

- Reproduce the `supply-300-active` setup timeout from current `origin/main` and inspect the harness
  server log, launch URL, dev-scenario room flow, and prediction readiness state.
- Make the smallest repair needed for the existing active workload to reach its authored fixture.
  Preserve `scenarioId=supply_stress_active`, seed `0x5a000300`, player 1, `spectator:false`, ready
  compatible prediction, 300 supply, supply cap 50, 201 projected regular entities, and the exact
  per-owner/per-kind composition in `scripts/client-perf/workloads.mjs`.
- Do not increase the timeout as the sole fix, mutate client state, relax setup validation, turn the
  browser into a spectator, or change the fixture's unit composition.
- Keep the normal harness-owned server path working. A release binary may be used for diagnosis,
  but a committed fix cannot require a developer-specific absolute binary path.

## Expected Touch Points

- `client/src/frame_profiler.js`
- `client/src/renderer/index.js`
- `client/src/renderer/rigs/runtime.js`
- `client/src/renderer/rigs/png_runtime.js`
- other renderer helpers that emit per-part counters
- focused profiler, renderer, and client-perf contract tests
- the smallest active-dev-scenario/harness files identified by the timeout investigation
- `docs/perf-tracing.md` if the counter collection contract changes

Do not modify fog computation, animation sampling, route coverage, the snapshot stream, protocol,
or balance in this phase.

## Verification

Run the smallest focused tests that cover the final touch points, including:

```bash
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
node scripts/client-flamegraph.mjs --preview
node scripts/client-flamegraph.mjs --workload supply-300-active --seconds 15 --preview
node scripts/check-docs-health.mjs
git diff --check
```

If `tests/client_contracts.mjs` supports a narrower documented selector, use it during iteration and
run the complete command before delivery. Retain both passing harness `summary.json` files and both
ranked flame-graph summaries under ignored `target/client-perf/` paths with identical viewport, DPR,
CPU throttle, sampling interval, and duration.

## Acceptance Evidence

- Hot rig counter events are aggregated in bounded renderer-owned state and flushed no more than
  once per nonzero label per frame.
- Existing diagnostic counter totals, per-frame maxima, report reset behavior, and failed-draw
  categories remain contract-covered.
- `_recordRenderDiagnostic` plus `recordDiagnosticCounter` no longer form a dominant steady-state
  self-time stack. Use paired ranked summaries; do not encode one machine-specific percentage as a
  permanent test threshold.
- `supply-300-hellhole-stream` still proves 900 frames, 30 Hz, 380 entities, no WebSocket, and no
  server simulation.
- `supply-300-active` passes every existing authoritative setup assertion before sampling. Any
  timing captured by a failed setup remains explicitly discarded.
- No production-cap or device-certification claim is made from either local profile.

## Manual Visual Test

Use the project-local `interact` skill from the implementation worktree. Open a small Pixi Lab
scene containing a Tank, infantry, one setup weapon, a building, fog, and visible feedback; inspect
the authoritative scene, capture one clean 1000x700 DPR 1 PNG, inspect it once, reject any
blank/stale/missing-texture result, close the session, and preserve only the returned Tailnet
Preview URL in the handoff. Also move the camera, allow several live frames to run, and leave and
re-enter once to catch accumulator leakage across renderer teardown.

## PR and Handoff Requirements

Mark this phase Done in the implementation commit. Run
`scripts/agent-pr.sh --verification "<focused checks and both admissible profiles passed>"`, then
`scripts/wait-pr.sh <pr>` and verify the phase head is reachable from `origin/main`.

The handoff must include the active-workload root cause and exact retained assertions, the bulk
counter contract, before/after diagnostic call evidence, both profile locations/settings, remaining
top functions, the Interact Preview URL, and the Phase 2 fog work plus its core manual tests.
