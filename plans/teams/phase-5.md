# Phase 5 - Client Team Model, Inspection, and Command Safety

Status: planned.

## Goal

Make the client understand own, ally, enemy, and neutral relationships. Allied units should be
inspectable and visually understandable, but they must not become group-commandable or accidentally
attackable through normal UI paths.

## Scope

- Replace gameplay direct owner comparisons with `GameState` relationship helpers where the logic is
  about relationship, not strict command ownership.
- Keep own-control checks strict for:
  - command emission
  - prediction
  - optimistic production/rally
  - control groups
  - build/gather/train/research/cancel/ability execution
- Selection rules:
  - single-click own entities behaves as today
  - single-click allied entity selects it for inspection
  - box selection selects own units only, with existing own-building fallback
  - ctrl/meta same-kind selection selects own entities only
  - shift-add direct click may include an allied inspection target, but must not create a commandable
    mixed group
- Right-click rules:
  - enemy target issues attack
  - resource target with selected workers issues gather
  - own or allied entity target falls through to ordinary move-to-point behavior
- HUD and command card:
  - allied-only selections show read-only inspection details
  - no command buttons emit commands for allied-only selections
  - resources/supply/upgrades remain local-player only
- Renderer and minimap:
  - distinguish own, ally, enemy, and neutral/resource
  - keep entity body color per owner, not per team, unless a later art pass changes this deliberately
- Score UI:
  - add Team column
  - highlight all rows whose `teamId` matches `winnerTeamId`
  - keep `winnerId` support for singleton FFA compatibility

## Expected Touch Points

- `docs/design/client-ui.md`
- `client/src/state.js`
- `client/src/input/`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/minimap.js`
- `client/src/match.js`
- `client/src/app.js`
- `client/src/renderer/`
- `client/src/prediction_controller.js`
- `client/src/sim_wasm_adapter.js`
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/input_context_menu_contracts.mjs`
- `tests/minimap_input_contracts.mjs`
- `tests/client_smoke.mjs`
- `tests/team_integration.mjs`

## Verification

```bash
node tests/client_contracts.mjs
node tests/input_context_menu_contracts.mjs
node tests/minimap_input_contracts.mjs
node tests/team_integration.mjs
node tests/client_smoke.mjs
node scripts/check-client-architecture.mjs
```

Required automated scenarios:

- `isAllyOwner` and `isEnemyOwner` classify from `start.players`.
- Box selection skips allied units.
- Single-click can select an allied entity for inspection.
- Allied-only selection produces no command emission.
- Right-clicking an allied entity with own units selected sends move, not attack.
- Minimap or renderer contract distinguishes ally from enemy.
- Score table renders Team column and highlights all winning-team rows.
- Prediction remains own-unit-only.

## Acceptance Criteria

- Allied units are inspectable but not commandable.
- Allied units are not attackable through normal UI command targeting.
- Own-only command and prediction paths stay own-only.
- Team score display is clear and test-covered.

## Manual Testing Focus

One browser pass: click an allied unit, box near allied units, right-click an allied unit with own
units selected, and inspect the score screen. Prefer using an automated setup URL or scripted team
room created by `tests/team_integration.mjs`.

## Handoff Requirements

The phase handoff must distinguish relationship replacements from strict ownership checks and name
the tests that prove no allied command is emitted.
