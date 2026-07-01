# Phase 2 - Frozen Client Planning UX

## Phase Status

Status: pending.

## Objective

Make queued artillery setup create a clear client planning anchor. After the player Shift-clicks a
setup facing point, the setup target mode should de-arm even while Shift remains physically held,
and the client should keep frozen planned field-of-fire cones at the accepted projected setup
origins so Point Fire targeting is visually understandable.

## Scope

- Add client intent state for frozen queued setup planning previews. Keep it separate from the
  mouse-following `antiTankGunSetupPreview` so it can survive command-target changes.
- When a queued setup command is issued from world input or minimap input:
  - send the existing `setupAntiTankGuns(..., queued: true)` command,
  - store frozen cone records for selected owned Artillery and Anti-Tank Guns using the same
    projected origin logic as the queued setup preview,
  - compute the frozen facing from each projected origin to the clicked setup point,
  - clear the armed setup command target even though Shift remains held.
- Point Fire targeting should use frozen Artillery setup cones as preview origins when they are
  available for the selected artillery. The min/max range and field-of-fire feedback should be based
  on the planned emplacement origin/facing, not the artillery's current packed position.
- Keep the frozen preview local-only. The server's owner-only `orderPlan` remains the accepted
  authoritative marker after snapshots arrive.
- Clear frozen cone records when they would mislead the player: affected unit deselected, explicit
  cancel, unqueued replacement command, Stop/Hold, match teardown, unit no longer owned/visible to
  command owner, or server `orderPlan` no longer contains the matching queued setup.
- Preserve current non-queued setup behavior: unqueued setup can remain mouse-following until the
  click and does not create a persistent queued planning anchor.
- Update renderer feedback so frozen setup cones can be drawn while Point Fire targeting is armed.

## Expected Touch Points

- `client/src/client_intent.js`
- `client/src/input/commands.js`
- `client/src/input/index.js`
- `client/src/minimap.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/hud_command_card.js`
- `client/src/hud.js` if command-card ability availability needs targeted adjustment
- `tests/client_contracts/state_input_contracts.mjs`
- `tests/client_contracts/config_contracts.mjs`
- `tests/client_contracts/renderer_feedback_contracts.mjs`
- `tests/client_contracts/hud_contracts.mjs`
- `docs/design/client-ui.md`

## Edge Cases To Cover

- Shift-click setup de-arms setup targeting even though Shift remains held.
- Frozen setup cones remain visible while Point Fire targeting is armed.
- Point Fire preview range/field-of-fire uses the frozen planned artillery origin and facing.
- Moving the mouse after queued setup does not move the frozen cones.
- Re-arming setup and Shift-clicking a new setup replaces or updates the frozen cone for the same
  unit instead of stacking contradictory previews.
- Minimap queued setup follows the same de-arm and freeze behavior as world input.
- Selection change, Stop/Hold, Escape, match teardown, or server order-plan mismatch clears stale
  frozen cones.
- Mixed selections freeze setup cones for setup-capable units, but Point Fire preview only uses
  frozen Artillery cones.

## Verification

- Focused client contract tests for input, minimap, command composer lifetime, and renderer
  feedback view-model behavior.
- `node scripts/check-client-architecture.mjs`.
- `git diff --check`.

## Manual Test Focus

In a local match, select artillery, right-click move, Shift-click Set Up, and move the mouse. Confirm
the setup target is no longer armed, the planned cones stay fixed at the future setup location, and
Point Fire targeting previews against those fixed cones.

## Handoff Expectations

Describe the frozen preview state shape, lifetime rules, and how it reconciles with authoritative
`orderPlan` snapshots. Call out any UI ambiguity that remains, especially if final server rejection
can still differ from the client preview because of movement/pathing or ammo changes.
