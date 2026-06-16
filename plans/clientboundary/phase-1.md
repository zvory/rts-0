# Phase 1 - Baseline Contracts

## Phase Status

- [ ] Not implemented.

## Objective

Freeze current client boundary behavior before extracting state and facades.

## Work

- Add focused tests for command-target lifetime, preview clearing, HUD command descriptor dispatch,
  prediction optimism handoff, and renderer feedback's expected state shape.
- Update `docs/design/client-ui.md` to describe the target boundary and compatibility-shim rule.
- Avoid moving runtime behavior in this phase.

## Expected Touch Points

- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `tests/prediction_controller.mjs`
- `scripts/check-client-architecture.mjs`
- `docs/design/client-ui.md`

## Implementation Checklist

- [ ] Inventory existing client boundary tests.
- [ ] Add command-target and preview lifetime coverage.
- [ ] Add HUD dispatch and prediction optimism coverage.
- [ ] Document target boundary and migration rule.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `node scripts/check-client-architecture.mjs`
- `node tests/hud_command_card.mjs`
- `node tests/prediction_controller.mjs`
- Focused `node tests/client_contracts.mjs`

## Manual Test Focus

Live match smoke for select, move, attack, queued target clicks, worker build placement, and
command-card hotkeys.

## Handoff Expectations

List frozen behaviors, uncovered edge cases, and the current direct state fields that later phases
must migrate.
