# Phase 6 - GameState Helper Extraction

Status: done.

## Goal

Extract narrow helpers from `client/src/state.js` while preserving `GameState` public behavior,
snapshot application order, interpolation semantics, and the client intent boundary.

## Scope

- Read `docs/context/client-ui.md` and the `GameState` section of
  `plans/hotspots/responsibility-map.md`.
- Extract transient visual-effect buffers and read/query helpers first.
- Consider selection/control-group helpers only if command-budget admission remains shared and the
  same public methods stay covered by contracts.
- Keep prediction and optimistic overlay extraction out of scope unless the implementation proves a
  tiny mechanical helper split with existing coverage.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if new state helper paths
  are not grouped under `client-state-model`.

## Touch Points

- `client/src/state.js`
- narrowly named state/model helper modules
- GameState, selection, command-budget, or intent-state contract tests
- `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if grouping needs new helper paths
- `plans/hotspotcleanup/phase-6.md`

## Constraints

- Do not change the data shape consumed by renderer, HUD, minimap, input, or match.
- Do not move browser-local placement, command targeting, command-card mode, lab tools, previews, or
  command feedback back into `GameState`.
- Do not change snapshot application order, effect timing, interpolation, command-budget admission, or
  prediction semantics.
- Do not introduce imports from renderer, HUD, or input into state helpers.

## Verification

- `node tests/client_contracts.mjs` or targeted GameState contracts plus the stable runner
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

## Manual Testing Focus

Manually check selection/control groups, resource/death deltas, projectile and smoke effects, and
prediction overlays in a local match or replay if touched.

## Handoff

After implementation, mark this phase done and summarize the extracted helpers, preserved public
methods, commands run, manual checks, and any state clusters intentionally left for a later design
pass.
