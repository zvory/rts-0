# Phase 8 - Client Command Safety and Ally Inspection

Status: done.

## Goal

Make the in-match client understand own, ally, enemy, and neutral relationships. Allied units should
be inspectable and visually understandable, but they must not become group-commandable or
accidentally attackable through normal UI paths.

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
  - single-click own entities behaves as today.
  - single-click allied entity selects it for inspection.
  - box selection selects own units only, with existing own-building fallback.
  - ctrl/meta same-kind selection selects own entities only.
  - shift-add direct click may include an allied inspection target, but must not create a
    commandable mixed group.
- Right-click rules:
  - enemy target issues attack.
  - resource target with selected workers issues gather.
  - own or allied entity target falls through to ordinary move-to-point behavior.
- HUD and command card:
  - allied-only selections show read-only inspection details.
  - no command buttons emit commands for allied-only selections.
  - resources/supply/upgrades remain local-player-only.
- Renderer and minimap should distinguish own, ally, enemy, and neutral/resource in contract-tested
  ways.
- Prediction and sim-wasm client adapters must parse team fields and remain own-unit-only.
- Keep normal lobby exposure for non-FFA team presets gated until this phase's client command-safety
  tests pass.

## Expected Touch Points

- `docs/design/client-ui.md`
- `client/src/state.js`
- `client/src/input/`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/minimap.js`
- `client/src/match.js`
- `client/src/renderer/`
- `client/src/prediction_controller.js`
- `client/src/sim_wasm_adapter.js`
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/input_context_menu_contracts.mjs`
- `tests/minimap_input_contracts.mjs`
- `tests/prediction_controller.mjs`
- `tests/client_smoke.mjs`
- `tests/team_integration.mjs`

## Verification

```bash
node tests/client_contracts.mjs
node tests/input_context_menu_contracts.mjs
node tests/minimap_input_contracts.mjs
node tests/prediction_controller.mjs
node tests/team_integration.mjs
node tests/client_smoke.mjs
node scripts/check-client-architecture.mjs
```

Required automated scenarios:

- `isAllyOwner` and `isEnemyOwner` classify from `start.players`.
- Box selection skips allied units.
- Single-click can select an allied entity for inspection.
- Allied-only selection produces no command emission.
- Mixed own/allied selection cannot create commands for allied entity ids.
- Right-clicking an allied entity with own units selected sends move, not attack.
- Minimap or renderer contract distinguishes ally from enemy.
- Prediction remains own-unit-only.

## Acceptance Criteria

- Allied units are inspectable but not commandable.
- Allied units are not attackable through normal UI command targeting.
- Own-only command and prediction paths stay own-only.
- Remaining direct owner comparisons are documented as strict ownership checks or queued follow-up.

## Manual Testing Focus

One browser pass using scripted setup if available: click an allied unit, box near allied units, and
right-click an allied unit with own units selected.

## Handoff Requirements

The phase handoff must distinguish relationship replacements from strict ownership checks and name
the tests that prove no allied command is emitted.

## Implementation Notes

- Relationship replacements: client attack targeting now uses `GameState.isEnemyOwner`, and renderer
  and minimap inspection colors distinguish own, ally, enemy, and neutral/resource.
- Strict ownership checks retained: command emission, control groups, prediction, optimistic UI,
  production/rally commands, build/gather/train/research/cancel, and ability execution remain
  local-player-only.
- Contract coverage proving allied command safety:
  `node tests/client_contracts.mjs`,
  `node tests/input_context_menu_contracts.mjs`,
  `node tests/minimap_input_contracts.mjs`, and
  `node tests/prediction_controller.mjs`.
