# Phase 5 - Prediction Display Boundary

## Phase Status

- [ ] Not implemented.

## Objective

Separate prediction bookkeeping from `GameState` display mutation.

## Work

- Introduce an explicit prediction view or display overlay seam.
- Keep `PredictionController` responsible for client sequence allocation, pending commands,
  acknowledgements, and optimistic bookkeeping.
- Make `GameState` apply a named display overlay rather than letting prediction code mutate broad
  display state directly.
- Preserve authoritative snapshot dominance and prediction-disabled spectator/replay/dev-watch
  paths.

## Prediction Overlay Contract

Prediction code produces display overlays; it does not mutate snapshot truth. `PredictionController`
owns sequencing, pending commands, acknowledgements/rejections, and optimistic UI bookkeeping.
`GameState` applies named display overlays through a small method such as
`applyDisplayOverlay("prediction", overlay)` and can clear that source independently.

Overlay invariants:

- authoritative snapshots dominate on every apply
- `entitiesInterpolated(..., { includePrediction: false })` remains prediction-free for fog and
  authority reads
- predicted entity overlays are own-unit-only
- optimistic production/rally overlays cannot unlock commands, supply, tech, fog, collision,
  pathing, or visibility
- disabling prediction clears prediction overlays and optimistic UI display

## Expected Touch Points

- `client/src/prediction_controller.js`
- `client/src/state.js`
- `client/src/match.js`
- `tests/prediction_controller.mjs`
- Tri-state prediction scenarios if selected

## Implementation Checklist

- [ ] Identify current prediction display mutations.
- [ ] Add explicit prediction view/update seam.
- [ ] Route `GameState` updates through a named overlay method.
- [ ] Preserve prediction-disabled paths.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/prediction_controller.mjs`
- Tri-state scenarios for spectator/replay no prediction, move prediction, train optimism, rally
  optimism, and hidden blocker/no leak when selected by changed files

## Manual Test Focus

Train optimism, rally optimism, prediction toggle/settings, spectator mode, replay mode, and
dev-watch no-command-sequence behavior.

## Handoff Expectations

Record whether any WASM prediction adapter assumptions remain coupled to `Match`.
