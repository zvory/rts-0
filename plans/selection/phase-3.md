# Phase 3 - Control Groups and Command Guard Hardening

Status: Not started.

## Goal

Prevent control groups from bypassing the selection supply budget and harden the Phase 1
command-send guard against the new control-group paths. Control-group save, add, and recall should
preserve only legal selections, and outgoing commands should continue to refuse over-budget unit
lists that the server would reject.

## Scope

- Apply budget admission to `client/src/input/control_groups.js` and any underlying `GameState`
  control-group storage helpers.
- Decide from Phase 0 inventory whether illegal over-budget saved groups can exist from older
  runtime state. If so, recall should filter to a legal admitted set rather than restoring the full
  group.
- Control-group save should store the current legal selection.
- Control-group add should add current legal selection candidates until the saved group reaches a
  legal budget. It should ignore overflow instead of trimming unrelated existing group members.
- Control-group recall should:
  - remove dead/missing/invisible entities according to existing behavior
  - pre-admit Command Cars from the group so their bonus is reliable
  - fill remaining entities in saved order until budget is full
  - update selection to the admitted set
  - rewrite the saved control group to that same legal admitted set, so tab counts, repeated recall,
    selected-group highlighting, and double-tap camera jumps all use the same entities
- Re-audit outgoing human command composition/sending paths after control-group changes. The Phase 1
  command-send guard should remain the single client-side send gate; extend it only for paths missed
  by Phase 1 or introduced by this phase. Honest clients should not send commands the Phase 1 server
  validation will reject.
- Command-send checks should validate the exact ids being submitted by each command path, not the
  broader current selection. This matters for subset commands such as workers-only gather/build,
  setup/teardown-capable units, ability carriers, and selected production buildings.
- Keep AI command generation unaffected.

## Expected Deliverables

- Control groups cannot save, add, or recall over-budget human selections.
- Any over-budget saved group discovered at recall is normalized to its legal admitted ids.
- Outgoing human command unit lists are still checked against the client budget before send, using
  the same guard introduced in Phase 1.
- Overflow from control-group recall or command sending can trigger the same UI feedback signal as
  ordinary selection overflow.
- Double-tap camera jump still operates on the recalled legal control-group entities.

## Verification

- Add focused tests for control-group save/add/recall with:
  - 24 one-supply units
  - over-budget Tanks
  - one and multiple Command Cars
  - a Command Car late in stored order
- Add or update command composition tests proving over-budget commands restored through
  control-group recall are not sent.
- Add subset-command tests proving the send guard validates submitted ids only, including at least one
  workers-only or ability-carrier command from a broader legal selection.
- Run the relevant targeted Node test files only.

## Manual Testing Focus

Assign and recall legal and over-budget groups, including Tank-heavy groups and groups with
Command Cars late in order. Confirm double-tap camera jump still centers on the recalled legal
group and normal command hotkeys still issue orders.

## Handoff Expectations

The handoff must name every control-group operation touched, confirm whether recall rewrites saved
groups to the admitted ids, identify any additions made to the Phase 1 command-send guard, and
explain how outgoing command overflow is presented to the player or suppressed.
