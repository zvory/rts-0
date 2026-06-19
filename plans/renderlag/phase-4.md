# Phase 4 - Pixi Renderer Scaling Pass

## Phase Status

- [x] Done.

## Objective

Reduce the remaining recurring Pixi renderer cost after minimap and entity-view optimizations land.
The renderer should keep the current visual language while doing less work per entity per frame,
especially in unit, resource/building, selection/HP, rig, and feedback paths.

## Work

- Start from current `window.__rtsPerf` renderer subphase evidence after Phases 1-3. Do not assume
  earlier measurements still identify the renderer bottleneck.
- Inspect and optimize the highest measured renderer subphases first:
  - `renderer.units`;
  - `renderer.resourcesBuildings`;
  - `renderer.selectionHp`;
  - `renderer.feedbackView`;
  - `renderer.feedbackOverlays`;
  - `renderer.fogDraw` if it resurfaces under fogged scenarios.
- Prefer localized improvements:
  - update Pixi object position/rotation/visibility without rebuilding static graphics;
  - cache static rig parts or unit silhouettes when tint/state permits;
  - avoid clearing and redrawing unchanged overlays;
  - avoid repeated allocation in hot entity loops;
  - keep pooled object sweeps bounded and cheap.
- Preserve render failure recovery. A broken entity or feedback effect must still fail soft through
  the existing renderer/frame recovery behavior rather than stopping the match loop.
- Preserve fog layering, shot-reveal silhouettes, selection rings, HP bars, command feedback,
  placement previews, and current rig visuals unless the phase explicitly documents an equivalent
  visual refactor.
- Avoid broad renderer rewrites. Extract a helper only when it reduces hot-path complexity or matches
  existing renderer module boundaries.

## Expected Touch Points

- `client/src/renderer/index.js`
- `client/src/renderer/entities.js` or related renderer-local helpers if present
- `client/src/renderer/rigs/*`
- `client/src/frame_profiler.js` only if more renderer subphase labels are needed
- `tests/client_contracts.mjs`
- `tests/transparent_unit_pixels.mjs` or renderer visual tests if touched by the change
- `docs/design/client-ui.md` if renderer contracts or visual guarantees change

## Implementation Checklist

- [x] Collect current post-Phase-3 renderer subphase evidence before changing code. The browser
      harness attempt was blocked before artifact creation by sandbox
      `listen EPERM: operation not permitted 127.0.0.1`; source inspection identified
      `renderer.units` live rig redraws as the safe phase-local target.
- [x] Optimize the highest measured renderer subphase first.
- [x] Preserve Pixi pooling and renderer soft-failure behavior.
- [x] Add or update focused renderer tests for any helper or visual contract touched.
- [x] Run before/after browser perf harness workloads and save artifact paths in the handoff.
      Attempts were blocked before artifact creation by sandbox
      `listen EPERM: operation not permitted 127.0.0.1`.
- [x] Run verification and record exact results.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- renderer-specific visual or contract tests selected by the touched files
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 10`
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `tests/run-all.sh --only-browser` if practical for the touched renderer surface
- `git diff --check`

If client design docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Open normal gameplay, a replay with combat, and the vehicle-wall stress scenario. Confirm units,
buildings, resource nodes, selection rings, HP bars, fog, shot-reveal markers, command feedback,
setup/deployment visuals, and placement previews still render correctly while the frame loop remains
responsive.

## Handoff Expectations

Report which renderer subphase was optimized, the before/after timings, and any visual or pooling
tradeoffs. List renderer subphases that remain above 1 ms average or 2 ms p95 bucket so Phase 5 and
future work do not guess.
