# Phase 4 - Renderer Feedback View Model

## Phase Status

- [x] Done.

## Objective

Give renderer feedback a narrow, stable data shape instead of broad `GameState` access.

## Work

- Add a renderer-facing feedback view builder for placement, command feedback, previews, ability
  objects, selected entities, and any other feedback-only state.
- Make the feedback view a plain data object built outside `renderer/feedback.js`, with no
  `GameState`, `ClientIntent`, or mutating methods. The builder owns TTL pruning and shape
  normalization; draw functions consume data only.
- Include only renderer feedback inputs such as placement ghost data, placement-resource hints,
  already-pruned command feedback markers, mining preview, support-weapon setup preview, ability
  target preview, authoritative ability objects/events, and selected-entity overlay inputs.
- Keep drawing behavior visually identical.
- Avoid splitting `renderer/feedback.js` unless a small area-local helper naturally supports the
  new boundary.
- Add tests for placement, ability target preview, resource mining preview, and command feedback
  view-model shape.

## Expected Touch Points

- `client/src/renderer/feedback.js`
- `client/src/state.js` or the new model helper
- `client/src/renderer/index.js` or `client/src/match.js` if render signatures change
- `docs/design/client-ui.md`

## Implementation Checklist

- [x] Define the feedback view-model shape.
- [x] Build the view model from model/intent state.
- [x] Route renderer feedback through the view model.
- [x] Add focused tests for the view model.
- [x] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- Focused client contracts for feedback data
- Renderer feedback/view-model contract coverage in `tests/client_contracts.mjs`
- `client-smoke` when practical for rendered behavior

## Manual Test Focus

Placement ghost, mining range hint, support-weapon cones, mortar/artillery target markers, ability
previews, and queued command markers.

## Handoff Expectations

Include screenshot or manual notes for visual risks that were watched but not automated.

## Post-Phase Boundary Notes

This phase is complete on `main`. Phase 6 should verify that renderer feedback no longer prunes
TTLs, mutates intent, or reaches through broad state for feedback-only data outside the view-model
builder.
