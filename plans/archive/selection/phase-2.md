# Phase 2 - Client Selection Budget

Status: Done.

## Goal

Replace the client-side 12-unit selection cap with supply-budget admission for normal selection
flows. The player should be able to select many low-supply units or fewer high-supply units, with
Command Cars reliably expanding the budget by 12 each.

## Scope

- Add a client selection/command budget helper, preferably in a small module under `client/src/`
  or `client/src/input/`, that mirrors the Phase 1 constants and weight rules. If Phase 1 already
  introduced a command-send guard helper, extend that helper instead of creating a parallel budget
  implementation.
- Use `client/src/config.js` `STATS[kind].supply` for unit command weights and fall back to 1 for
  selectable entities without supply.
- Remove old `.slice(0, 12)` behavior from:
  - `_closestOwnUnitKindInViewport`
  - `_closestIdsToPoint`
  - any other selection helpers found in Phase 0
- Remove `GameState.MAX_SELECTION_SIZE` from ordinary playable selection admission by replacing the
  count-based truncation in `setSelection` and `addToSelection` with budget admission. Control-group
  storage can keep its old count cap until Phase 3, but ordinary selection must not keep 12 units as
  a second gameplay limit.
- Apply budget admission to:
  - direct click selection
  - shift-click add/remove selection
  - drag-box selection
  - ctrl/double-click same-unit and same-building selection
- Keep spectator and replay inspection behavior unchanged unless a candidate path is also used by a
  local playable own-selection operation. Budget admission in this phase is a player commandability
  rule, not an observer inspection limit.
- Preserve existing candidate order for non-Command Cars.
- Pre-admit Command Cars from the candidate set for additive and replacement selection so Command
  Car bonus is not dependent on drag-box, viewport, or id ordering. After Command Cars are admitted,
  fill remaining candidates in the normal existing order.
- Shift-add should add until full and ignore overflow. It should not trim or replace existing
  selected entities.
- Surface a lightweight overflow event or state flag for Phase 4 UI to consume.
- Ensure the Phase 1 command-send guard still wraps every outgoing human multi-unit command after
  selection admission changes. This phase should not broaden or remove that guard.

## Expected Deliverables

- The strict 12 selected-unit limit is gone from ordinary client selection.
- Base budget is 24 supply.
- Command Car budget bonus stacks.
- Buildings and non-combat selectable entities count as 1 unless they have a mirrored supply value.
- Overflow candidates are ignored and can trigger UI feedback later.
- Existing spectator/replay selection affordances still inspect visible entities without inheriting
  the playable selection budget.

## Verification

- Add focused client tests for the budget helper and selection admission order where practical.
- Run relevant Node suites or targeted test files identified in Phase 0.
- Manually inspect that no active `.slice(0, 12)` selection cap or `GameState.MAX_SELECTION_SIZE`
  ordinary-selection cap remains outside tests or archived docs.

## Manual Testing Focus

In a local match or scenario, select 24 Riflemen, four Tanks, mixed Tank/infantry groups, and
Command Car groups. Check box-select, shift-select, and double-click behavior, especially when the
Command Car appears far from the drag start.

## Handoff Expectations

The handoff must describe the client budget helper API, list the selection flows converted, confirm
whether spectator/replay selection was left unchanged, and call out the overflow signal that Phase 4
should render.
