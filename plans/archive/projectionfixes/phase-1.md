# Phase 1 - Lab Control Feedback

Status: done.

## Goal

Make lab-selected non-local owners receive the same visual feedback as the local player would in a
normal live match. This phase should not change server command semantics; it should make the client
read the selected controllable owner consistently for feedback and presentation.

## Scope

- Introduce or reuse a small client helper/read model that answers:
  - can this owner be controlled in the current state?
  - is this owner the current feedback owner?
  - which selected owner, if any, is the issue-as owner?
- Route renderer feedback through that helper instead of raw `state.playerId` where the feedback is
  about controlled-owner affordances:
  - selected order-plan lines
  - setup/field-of-fire wedges
  - mortar range rings
  - rally lines and rally plans
  - selected-entity ring ownership coloring
  - debug path overlays when the diagnostic is available for selected lab-controlled units
- Update control-group admission so lab-selected non-neutral entities from a single controllable
  owner can be stored, recalled, and pruned consistently.
- Update combat and point-fire audio "self"/"other" category to use the selected lab issue-as owner
  when one exists, while preserving normal live behavior outside lab.
- Keep command feedback honest where practical: local markers may still appear immediately for a
  submitted command, but they should be associated with the controlled owner and should not imply a
  server event was delivered.

## Expected Touch Points

- `client/src/lab_control_policy.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/renderer/feedback.js`
- `client/src/renderer/entities.js`
- `client/src/state.js`
- `client/src/input/control_groups.js`
- `client/src/match_combat_audio.js`
- focused client tests or smoke helpers if present

## Constraints

- Do not import lab transport or lab panel modules into renderer/input/HUD modules.
- Do not make ordinary spectators controllable. Lab control should continue to require operator
  role and a valid selected owner.
- Do not broaden what the server sends. This phase only changes how already-projected client data is
  interpreted for lab feedback.
- Preserve normal player, spectator, replay, and dev behavior when no lab control policy is active.

## Verification

- Run the client architecture check:

```bash
node scripts/check-client-architecture.mjs
```

- Add or update focused JS tests if an existing lightweight test can exercise the helper/read model
  without a live server.

## Manual Testing Focus

In lab full-world, select P2 units and buildings, issue move/attack/rally commands, and confirm
order lines, rally lines, range rings, setup wedges, selection colors, and audio category match the
controlled owner. Repeat with P1 selected to confirm normal local-owner behavior still looks right.

## Player-Facing Outcome

Lab operators can control either side without the UI visually treating non-P1 units as enemies or
uncontrolled units.

## Handoff

After implementation, summarize the control-owner helper/read model, which feedback paths now use
it, any remaining feedback paths intentionally left local-player based, and the manual lab flows
that passed.
