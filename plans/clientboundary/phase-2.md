# Phase 2 - Client Intent State

## Phase Status

- [x] Done.

## Objective

Extract transient client intent state from `GameState` into a model-area helper.

## Work

- Introduce a helper that owns placement, command-card submenu state, command targeting, command
  feedback, mining preview, ability preview, and related intent slots.
- Keep `GameState` compatibility accessors for one phase so existing callers continue to work.
- Move methods such as begin/end placement, begin/end command targeting, preview updates, and
  command feedback TTL behind the helper.

## Expected Touch Points

- `client/src/state.js`
- `client/src/command_composer.js`
- New model-area helper file
- `scripts/check-client-architecture.mjs` if classification is needed
- `docs/design/client-ui.md`

## Implementation Checklist

- [x] Add client intent helper in the model area.
- [x] Move intent state and methods behind the helper.
- [x] Preserve `GameState` compatibility accessors.
- [x] Update tests for the helper and compatibility surface.
- [x] Run verification and record exact results in the handoff.

Historical note: this phase is already marked done. The checked list reflects completed phase
status; later phases should rely on the committed implementation and handoff rather than treating
the old unchecked list as pending work.

## Verification

- `node scripts/check-client-architecture.mjs`
- Focused client contract tests for command target, placement, feedback TTL, and previews
- `node tests/hud_command_card.mjs`

## Manual Test Focus

Worker build cancel/confirm, attack/move targeting, Shift queued targeting, hover mining previews,
support-weapon setup, and abilities.

## Handoff Expectations

Identify remaining direct `state.commandTarget`, `state.placement`, and preview callers to migrate.
