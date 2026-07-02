# Phase 2 - Minimap Fog Scheduling And Batching

## Phase Status

- [x] Done.

## Objective

Remove the recurring per-frame cost of drawing minimap fog over every tile. Fog should remain
accurate to the current client fog grids, but the minimap should not run a full visibility/explored
tile pass on every animation frame when the grids have not changed.

## Work

- Add a fog-layer update path that is invalidated by visibility/exploration changes instead of by
  every `requestAnimationFrame`.
- Choose the simplest measured representation that removes the full per-frame tile loop:
  - cached fog overlay canvas;
  - row-run batching;
  - `ImageData` writes;
  - or another localized representation with better measured cost.
- Preserve current minimap fog semantics:
  - visible tiles are clear;
  - explored hidden tiles are dimmed;
  - unexplored tiles are heavily dimmed;
  - impassable terrain keeps its lighter fog wash;
  - no-fog dev scenarios and reveal-all paths stay cheap and correct.
- Keep resource blips, entity blips, viewport outline, and pings readable after fog batching. If the
  implementation changes draw order for performance, document why the visible result is equivalent.
- Avoid using client-side fog as authority. The cache may depend on local explored history and the
  server-provided visible grid, but it must not reveal unseen data.
- Include a narrow timing probe or harness summary in the handoff that separates Phase 1 static-cache
  wins from this phase's fog-layer win.

## Expected Touch Points

- `client/src/minimap.js`
- possible minimap fog helper module
- `client/src/fog.js` only if a lightweight version or dirty marker is needed
- `client/src/frame_recovery.js` only if the frame loop needs a safe scheduling hook
- `tests/minimap_input_contracts.mjs`
- `tests/client_contracts.mjs`
- `docs/perf-tracing.md` if minimap fog interpretation changes

## Implementation Checklist

- [x] Add a minimap fog cache or batched draw path.
- [x] Invalidate the fog visual only when visibility/exploration inputs change.
- [x] Preserve visible, explored, unexplored, impassable, reveal-all, and no-fog behavior.
- [x] Keep minimap dynamic overlays readable and ordered.
- [x] Add focused tests around fog cache invalidation or helper output where practical.
- [x] Run before/after browser perf harness workloads and save artifact paths in the handoff. Attempts were blocked before artifact creation by sandbox `listen EPERM: operation not permitted 127.0.0.1`.
- [x] Run verification and record exact results.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/minimap_input_contracts.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `node scripts/client-perf-harness.mjs --workload selected-unit-hud-stress --seconds 10`
- `git diff --check`

If docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Open a normal fogged match and move units through unexplored, explored-hidden, and currently visible
areas. Confirm the minimap fog state updates promptly, does not flicker, does not reveal hidden
enemies, and still distinguishes water/stone map shape. Also inspect no-fog dev scenarios and
replays.

## Handoff Expectations

Report the before/after minimap terrain and fog costs separately if instrumentation is available.
State how fog invalidation is triggered, any cadence tradeoff, and whether Phase 3 can assume
minimap rendering no longer calls `entitiesInterpolated()` for cached terrain or fog work.
