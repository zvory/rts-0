# Phase 3 - Shared Frame Entity Views

## Phase Status

- [ ] Pending.

## Objective

Reduce repeated entity interpolation and allocation across the client frame pipeline. A frame should
compute the common entity views once, then pass or cache those views for fog, renderer, minimap, HUD,
and observer consumers that need the same data.

## Work

- Identify the per-frame entity views currently requested through `state.entitiesInterpolated()` and
  `state.selectedEntities()`:
  - renderer interpolated entities with prediction display;
  - current-frame entities without prediction for local fog-source filtering;
  - minimap entity blips;
  - selected entity detail for HUD selection panel;
  - observer analysis rows when the overlay is visible.
- Introduce a small frame-local view object or cache owned by the app-shell frame loop, `Match`, or
  `GameState`, whichever best matches current dependency direction.
- Keep view lifetime short and explicit. Cached arrays must not outlive the frame or become mutable
  shared state that later code can accidentally treat as authoritative.
- Preserve current interpolation and prediction behavior:
  - renderer still uses the frame alpha and prediction display when appropriate;
  - fog-source filtering still uses no-prediction authoritative positions;
  - replays, spectators, dev watchers, and prediction-disabled clients remain correct.
- Update renderer, minimap, fog update, HUD, and observer paths to accept injected frame views where
  useful instead of re-querying state.
- Add tests or debug counters that prove repeated `entitiesInterpolated()` calls drop for the harness
  workloads without relying on raw per-frame telemetry in normal uploads.

## Expected Touch Points

- `client/src/frame_recovery.js`
- `client/src/match.js`
- `client/src/state.js`
- `client/src/renderer/index.js`
- `client/src/minimap.js`
- `client/src/hud_selection_panel.js`
- `client/src/observer_analysis_overlay.js`
- `tests/client_contracts.mjs`
- `docs/design/client-ui.md` if public client module contracts change
- `docs/perf-tracing.md` if the debug interpretation changes

## Implementation Checklist

- [ ] Define the frame-local entity view shape and ownership.
- [ ] Replace repeated same-frame interpolation calls in renderer, minimap, fog, HUD, and observer
      paths where practical.
- [ ] Keep prediction, spectator, replay, and dev-viewer behavior unchanged.
- [ ] Add focused coverage for frame-view cache behavior and stale-frame avoidance.
- [ ] Run before/after browser perf harness workloads and save artifact paths in the handoff.
- [ ] Run verification and record exact results.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `node scripts/client-perf-harness.mjs --workload matt-alex-replay --seconds 10`
- `node scripts/client-perf-harness.mjs --workload vehicle-wall-stress --seconds 10`
- `git diff --check`

If client design docs change, also run:

```bash
node scripts/check-docs-health.mjs
```

## Manual Test Focus

Open a normal match with prediction enabled, a replay, and a spectator/dev watcher path if practical.
Confirm unit interpolation looks smooth, prediction display still appears only where expected,
selection panel details stay current, minimap blips track units, and fog does not drift from visible
units.

## Handoff Expectations

List the final frame-view fields, which consumers use them, and which `entitiesInterpolated()` call
sites intentionally remain. Include before/after call-count or timing evidence from the harness and
warn Phase 4 about any renderer path that still allocates heavily.
