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
- Relevant tri-state prediction scenarios, if selected by changed files

## Manual Test Focus

Train optimism, rally optimism, prediction toggle/settings, spectator mode, replay mode, and
dev-watch no-command-sequence behavior.

## Handoff Expectations

Record whether any WASM prediction adapter assumptions remain coupled to `Match`.
