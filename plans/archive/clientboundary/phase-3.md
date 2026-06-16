# Phase 3 - HUD And Input Intent Facade

## Phase Status

- [x] Done.

## Objective

Route HUD, input, and minimap intent mutations through the explicit client intent facade.

## Work

- Pass the client intent facade from `Match` into HUD, input, and minimap collaborators.
- Prefer role-shaped facade surfaces over broad state shims:
  - HUD intent: command-card mode/target reads, open/close submenu, begin placement, begin/end
    command target, and ability hover preview updates.
  - Input intent: placement/target reads, placement updates/end, target issue/hold/release, preview
    updates, and command feedback.
  - Minimap intent: command target read, target issue/end, and command feedback.
- Convert HUD command intent dispatch and input command-target refreshes to call the facade rather
  than mutating `GameState` directly.
- Preserve command issuing through `commandIssuer.issueCommand`.
- Keep temporary `GameState` compatibility fields until renderer migration is complete.

## Expected Touch Points

- `client/src/match.js`
- `client/src/hud.js`
- `client/src/input/index.js`
- `client/src/input/commands.js`
- `client/src/minimap.js`
- `docs/design/client-ui.md`

## Implementation Checklist

- [x] Add facade dependency injection from `Match`.
- [x] Convert HUD intent writes.
- [x] Convert input and minimap intent writes.
- [x] Preserve command issuer and command-budget behavior.
- [x] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/hud_command_card.mjs`
- Focused client contracts for hotkeys, targeting, and minimap targeting

## Manual Test Focus

Command-card buttons, hotkey repeat for train/cancel, minimap right-click move/attack/ability,
Escape cancellation, and right-click cancellation.

## Handoff Expectations

Note any compatibility fields still read by renderer, tests, or other modules.

## Post-Phase Boundary Notes

This phase is complete on `main`. Remaining phases should treat HUD/input/minimap intent routing as
the completed precondition and should preserve the rule that `commandIssuer.issueCommand` remains
the only gameplay command emission seam. Any lingering direct `GameState` intent reads should be
limited to renderer compatibility work scheduled for Phase 4 and removed in Phase 6.
