# Phase 3 - HUD And Input Intent Facade

## Phase Status

- [ ] Not implemented.

## Objective

Route HUD, input, and minimap intent mutations through the explicit client intent facade.

## Work

- Pass the client intent facade from `Match` into HUD, input, and minimap collaborators.
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

- [ ] Add facade dependency injection from `Match`.
- [ ] Convert HUD intent writes.
- [ ] Convert input and minimap intent writes.
- [ ] Preserve command issuer and command-budget behavior.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/hud_command_card.mjs`
- Focused client contracts for hotkeys, targeting, and minimap targeting

## Manual Test Focus

Command-card buttons, hotkey repeat for train/cancel, minimap right-click move/attack/ability,
Escape cancellation, and right-click cancellation.

## Handoff Expectations

Note any compatibility fields still read by renderer, tests, or other modules.
