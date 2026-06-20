# Phase 8 - Deep Render Diagnostics

## Phase Status

- [x] Done.

## Objective

Add enough local render diagnostics to identify the next expensive frame path after the minimap
optimizations. The output should explain whether frame cost is coming from Pixi object churn, rig
redraws, overlays, fog, minimap invalidation, entity-view work, DOM/HUD updates, browser frame gaps,
or a missing measurement category.

## Work

- Add bounded per-frame or per-window counters to the local profiler for renderer and HUD work:
  - Pixi display objects created, reused, hidden, swept, or destroyed;
  - rig redraws attempted, skipped, and completed;
  - `Graphics.clear()`/redraw counts for units, HP bars, selection rings, feedback overlays, and fog;
  - minimap static/fog cache invalidation reason counts;
  - entity-view cache hits, misses, and intentional uncached call sites;
  - selected-unit/HUD and observer-analysis dirty-guard hits and misses.
- Include shape context that explains cost without raw entity dumps: entity count, selected count,
  visible tile count, viewport, DPR, replay/live/dev mode, and workload id.
- Add a long-frame context block that identifies the slowest top-level phase and the slowest nested
  renderer/minimap category when available.
- Keep normal uploads bounded. Raw frame arrays, raw entity data, replay data, high-cardinality
  labels, stack traces, and Chrome traces must remain local-only artifacts.
- Prefer counters that can be asserted in tests over ad hoc console logs.
- Update docs so future investigators know which counter points at which renderer subsystem.

## Expected Touch Points

- `client/src/frame_profiler.js`
- `client/src/renderer/index.js`
- `client/src/renderer/rigs/*`
- `client/src/minimap.js`
- `client/src/frame_entity_views.js`
- `client/src/hud.js`
- `client/src/hud_selection_panel.js`
- `client/src/observer_analysis_overlay.js`
- `scripts/client-perf-harness.mjs`
- `docs/perf-tracing.md`
- `tests/client_contracts.mjs`
- renderer or minimap contract tests selected by touched files

## Implementation Checklist

- [x] Define a bounded diagnostic schema for local render counters.
- [x] Add renderer counters for object churn, redraws, and rig work.
- [x] Add minimap, entity-view, HUD, and observer dirty-guard counters.
- [x] Attach the counters to harness summaries without uploading raw/high-cardinality data.
- [x] Add tests for counter reset, aggregation, and absent-counter compatibility.
- [x] Document how to interpret each diagnostic category.
- [x] Run the render-lag suite and record which category is currently most expensive.
  - Attempted `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10`, but this
    executor sandbox rejected local server binding with `listen EPERM: operation not permitted
    127.0.0.1` before any workload could run. No current most-expensive live category was measured
    in this sandbox; rerun the same command from a normal local environment or PR-capable runner.
- [x] Run verification and record exact results.
  - `node tests/client_contracts.mjs` passed.
  - `node scripts/check-client-architecture.mjs` passed.
  - `node scripts/check-docs-health.mjs` passed.
  - `git diff --check` passed.
  - `node scripts/client-perf-harness.mjs --list` passed.
  - `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10` blocked by sandbox
    `listen EPERM: operation not permitted 127.0.0.1`.
- [x] Mark this phase as done in this file.

## Verification

- `node scripts/client-perf-harness.mjs --render-lag-suite --seconds 10`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- renderer/minimap focused tests selected by changed files
- `node scripts/check-docs-health.mjs`
- `git diff --check`

## Manual Test Focus

Open a normal match, the Matt/Alex replay, the vehicle-wall stress scenario, and a selected-unit
view. Confirm rendering still looks normal and the generated artifacts contain nonzero diagnostics
for the relevant paths without leaking raw entity lists or replay contents.

## Handoff Expectations

List every new diagnostic counter, where it is collected, where it appears in artifacts, and which
counter currently names the next likely bottleneck. If the diagnostics still cannot explain an 8 ms
frame-work p95, say exactly which missing measurement should be added next.
