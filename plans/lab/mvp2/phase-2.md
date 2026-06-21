# Phase 2 - Lab Tool Intent Boundary

## Phase Status

- [ ] Planned.

## Objective

Create an explicit client-side lab tool intent boundary so setup tools can be armed by the lab
panel and consumed by normal match input without panel-owned viewport listeners or `GameState`
shims.

## Work

- Extend `ClientIntent` with a small active lab tool state, cancellation path, and conflict rules
  with normal placement, command-card mode, command-target mode, drag selection, and camera input.
- Expose a narrow `Match` method or injected controller that lets `LabPanel` arm and cancel lab
  tools without importing input internals.
- Add input handling that consumes a world click for the active lab tool before normal selection or
  command targeting when that priority is intentional.
- Route the consumed click through a small callback interface that receives exact world
  coordinates and current tool payload.
- Add a minimal placeholder or test-only lab tool path if needed to prove the boundary before the
  spawn palette is rebuilt.
- Add escape/right-click or equivalent cancellation behavior consistent with existing placement and
  command-target cancellation.
- Keep renderer feedback lightweight in this phase. A cursor or active-tool status is enough if a
  full preview would create extra scope.

## Expected Touch Points

- `client/src/client_intent.js`
- `client/src/match.js`
- `client/src/input/index.js`
- `client/src/input/selection.js` if click priority touches selection
- `client/src/lab_panel.js`
- `client/src/lab_client.js` only if the controller needs a request helper
- `client/styles.css` for minimal active-tool state if needed
- `tests/client_contracts.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `docs/context/client-ui.md` if the section list or collaborator summary changes

## Implementation Checklist

- [ ] Define the active lab tool state shape and cancellation semantics in `ClientIntent`.
- [ ] Add a narrow arm/cancel API between `LabPanel` and `Match`.
- [ ] Ensure active lab tools clear or block conflicting placement and command-target modes.
- [ ] Add input routing that passes exact clicked world coordinates to the lab tool callback.
- [ ] Verify normal selection, drag selection, camera controls, and command-targeting behavior are
      unchanged when no lab tool is active.
- [ ] Add tests for arm, cancel, click-consume, and priority behavior.
- [ ] Update client design docs if this becomes a stable interaction contract.
- [ ] Run focused verification and record exact results in the handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If command-card or hotkey cancellation behavior is touched, also run:

- `node tests/hud_command_card.mjs`
- `node tests/hotkey_profiles.mjs`

## Manual Test Focus

In a lab, arm the placeholder or first available lab tool, click the world, cancel with the expected
key or pointer action, and confirm normal selection still works after cancellation. Also confirm a
normal match has no lab tool affordance and selection behavior is unchanged.

## Handoff Expectations

Describe the active lab tool state shape, the input priority rules, and how later phases should
send real lab operations through the callback. Call out any renderer feedback intentionally deferred
to avoid broad UI churn.
