# Phase 4 - Setup Tool Cleanup

## Phase Status

- [x] Done.

## Objective

Bring the remaining selected-entity setup actions into the same explicit lab tool and control
pattern where that improves precision, consistency, or maintainability.

## Work

- Review the current delete, move, owner reassignment, resource, research, vision, import, and
  export controls for inconsistent state ownership or coordinate-entry behavior.
- Move selected-entity repositioning to a click-to-world lab tool using the Phase 2 boundary.
- Keep delete and owner reassignment contextual to the selected entities, with clear disabled
  states when no valid selection exists.
- Preserve resource, research, vision, import, and export flows unless a small cleanup is needed to
  match the result/status pattern.
- Make batch mutation result handling explicit enough that stale ids or partial failures are not
  hidden behind optimistic UI state.
- Keep mixed-owner inspection allowed while making gameplay commandability and selected-entity
  mutations clear to the operator.
- Avoid adding new protocol operations unless the existing typed lab ops cannot express the cleanup
  safely.

## Expected Touch Points

- `client/src/lab_panel.js`
- `client/src/client_intent.js`
- `client/src/match.js`
- `client/src/input/index.js`
- `client/src/lab_control_policy.js` if selected-entity affordances need clearer policy helpers
- `tests/client_contracts.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` only if protocol shape changes

## Implementation Checklist

- [x] Inventory setup controls that still rely on manual coordinates or hidden panel state.
- [x] Move selected-entity repositioning to the shared lab tool click path.
- [x] Tighten selected-entity delete and owner reassignment enabled/disabled states.
- [x] Ensure lab result/status display handles stale ids and rejected operations clearly.
- [x] Preserve resource, research, vision, import, and export behavior.
- [x] Add tests for selected-entity tool state, click coordinates, disabled states, and rejected
      result display.
- [x] Run focused verification and record exact results in the handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- `git diff --check`

If protocol changes are required:

- `node tests/protocol_parity.mjs`

## Manual Test Focus

Select one or more entities, move them by clicking the world, delete them, and reassign ownership.
Confirm rejected operations show clear errors, resource/research controls still work, and scenario
import/export still round trips after setup edits.

## Handoff Expectations

List which old form behaviors were removed, which were intentionally preserved, and which setup
actions now use the shared lab tool path. Include any remaining UX rough edges that should be
handled in Phase 5 rather than reopened architecturally.
