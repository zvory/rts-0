# Phase 4 - Renderer Feedback View Model

## Phase Status

- [ ] Not implemented.

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

- [ ] Define the feedback view-model shape.
- [ ] Build the view model from model/intent state.
- [ ] Route renderer feedback through the view model.
- [ ] Remove renderer-side calls that prune TTLs, mutate intent, or reach through broad state for
      feedback-only data.
- [ ] Add focused tests for the view model.
- [ ] Run verification and record exact results in the handoff.

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
