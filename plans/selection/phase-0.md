# Phase 0 - Inventory and Contract Decision

Status: Not started.

## Goal

Map the existing selected-unit cap, command hardening, mirrored balance data, and selected-panel UI
before changing behavior. Decide whether this rollout should use a manual mirrored constant/helper
or introduce generated client configuration for command-budget values.

## Scope

- Inventory client selection admission paths:
  - `client/src/input/selection.js`
  - `client/src/input/control_groups.js`
  - any `GameState` selection/control-group helpers
  - command composition paths under `client/src/input/` and `client/src/command_composer.js`
- Inventory selected-panel rendering in `client/src/hud.js` and related CSS.
- Inventory server command validation in `server/crates/sim/src/game/services/commands.rs`,
  including `MAX_UNITS_PER_COMMAND`, `dedupe_cap_units`, planner facts, and every `SimCommand`
  variant that carries unit ids.
- Inventory balance/config mirrors:
  - `server/crates/rules/src/balance.rs`
  - `server/src/config.rs`
  - `client/src/config.js`
  - any existing dump or parity tools.
- Identify focused tests that already cover selection, command-card context, protocol parity, and
  command hardening.

## Expected Deliverables

- A short note in this phase file listing:
  - every old 12-unit cap site found
  - every server command unit-list validation site found
  - the recommended mirror strategy for command-budget constants and weights
  - the tests that later phases should extend
- No gameplay behavior changes.
- No UI behavior changes.

## Verification

- Run only read-only or docs-focused checks needed for confidence, such as `rg` inventories.
- If the phase only updates this plan document, no automated suite is required.

## Manual Testing Focus

None. This is an inventory phase with no intended player-facing change.

## Handoff Expectations

The handoff must name the chosen mirror strategy and the exact files Phase 1 should edit for server
budget validation.
