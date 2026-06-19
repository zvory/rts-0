# Phase 1 - Frame Phase Profiler

## Phase Status

- [ ] Not started.

## Objective

Add low-overhead client-side frame phase attribution so a slow frame can be explained without opening
DevTools and manually copying console output. This phase should turn "FPS is low" into "these concrete
frame phases are expensive" while keeping the data local/debug-only.

## Work

- Add a small browser-side profiler owned by the match/app shell, for example
  `client/src/frame_profiler.js`.
- Instrument the existing frame loop around these top-level phases:
  - frame gap and total frame work
  - latency refresh and alpha computation
  - camera update
  - input update
  - prediction visual advance
  - fog update
  - renderer render
  - HUD update
  - minimap render
  - observer analysis overlay update
  - health publish
- Add renderer sub-phase attribution inside `client/src/renderer/index.js` for the major existing
  boundaries: entity interpolation/prep, feedback view build, resources/buildings, units, selection/HP,
  shot reveals, sweeps, fog draw, feedback/effects overlays, and placement.
- Track bounded rolling aggregates, not unbounded raw frame history. Include at least count, total,
  max, p50/p95 or equivalent bucketed percentiles, slow-frame count, worst phase, and recent summary.
- Include useful shape context in local summaries: entity count, selected count, remembered building
  count, visible tile count, viewport size, canvas backing size, device pixel ratio, prediction mode,
  and document hidden/focused state.
- Expose local inspection through a stable debug object such as `window.__rtsPerf` and/or
  `window.__rtsDebug`, with a copyable summary method. Console logging should be opt-in or limited to
  rare slow-frame summaries.
- Keep existing FPS overlay/status behavior working. The current `MatchHealth` live FPS and one-minute
  FPS values are input context for this phase, not a replacement for phase timing.
- Do not add protocol fields, server logs, Fly upload behavior, or harness scripts in this phase.

## Expected Touch Points

- `client/src/frame_recovery.js`
- `client/src/match.js`
- `client/src/match_health.js`
- `client/src/renderer/index.js`
- new client profiler module, if useful
- `client/src/bootstrap.js` or debug-surface wiring, if needed
- `tests/client_contracts.mjs` for pure JS contract coverage
- `docs/perf-tracing.md` if local debug usage needs operator documentation

## Implementation Checklist

- [ ] Add the bounded frame profiler and summary API.
- [ ] Instrument the top-level match frame phases.
- [ ] Instrument renderer sub-phases without changing rendered behavior.
- [ ] Expose local summaries through `window.__rtsPerf` or an equivalent stable debug surface.
- [ ] Add focused tests for aggregation, bounds, percentile/worst-phase behavior, and debug-surface
  shape.
- [ ] Document how to inspect/copy local summaries.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-docs-health.mjs` if docs are touched
- `git diff --check`

If a browser smoke is practical, also run `tests/run-all.sh --only-browser` or the narrow browser
suite selected by `node tests/select-suites.mjs --from=origin/main`.

## Manual Test Focus

Open a normal local match and confirm the FPS overlay/status still updates, rendered gameplay still
looks normal, and `window.__rtsPerf` exposes a useful summary after at least 10 seconds. In a replay or
dev scenario, confirm the summary identifies frame phases without flooding the console.

## Handoff Expectations

Summarize the exact phases measured, the debug object/API added, the overhead risks, and any slow
phase candidates observed during manual testing. Tell the next agent which aggregate fields are stable
enough to promote into `ClientNetReport` in Phase 2.
