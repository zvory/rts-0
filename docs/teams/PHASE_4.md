# Phase 4 - Client Team Interactions and Score UI

Goal: make the client treat allies as allies: inspectable, non-commandable, non-attackable, and
visible in lobby/game-over UI.

This phase should not create shared control.

## Owner Classification

Replace direct owner comparisons in client gameplay modules with `GameState` helpers from Phase 0.

High-risk files:

- `client/src/input.js`
- `client/src/hud.js`
- `client/src/renderer.js`
- `client/src/minimap.js`
- `client/src/main.js`

Required classifications:

- Own: `owner === playerId`
- Ally: non-neutral owner whose `teamId` matches local player's `teamId`
- Enemy: non-neutral owner whose `teamId` differs
- Neutral: owner `0` or resource kinds

## Selection Rules

Implement the requested interaction rules:

- Own units and own buildings behave as today.
- Allied units and buildings can be single-click selected for inspection.
- Box selection selects only own units, with own buildings as the existing fallback.
- Ctrl/meta select-same-kind selects only own units.
- Shift-add selection may include an allied inspection target only if the click is direct. It must
  not let allied units enter commandable unit groups.
- Command card stays empty or inspection-only for allied selections.
- `_selectedOwnUnitIds()` and `_selectedWorkerIds()` remain own-only.

No allied command should ever be sent to the server.

## Right-Click Behavior

Right-clicking an allied unit should not issue attack.

Given current behavior, implement this by treating allied units like own units for target
classification:

- Enemy target: attack.
- Resource target with selected workers: gather.
- Everything else, including own or allied unit/building: move to clicked world point.

This preserves today's "right-click own unit is not a special command" behavior.

## Visual Treatment

Renderer and minimap should make relationships readable:

- Own selection ring: existing own color.
- Ally selected/hover ring: distinct ally color.
- Enemy selected/hover ring: existing enemy color.
- Entity body tint should remain the owner's player color, not a team color.
- Minimap should distinguish own, ally, enemy, and neutral/resource. Ally may use owner color plus
  ally outline, or a consistent ally blip color if owner colors are too noisy.

Do not use flags, national symbols, or large decorative UI.

## HUD and Scoreboard

HUD:

- Single selected allied entity shows owner name/team and hp/details.
- Allied production details are read-only.
- Resources/supply bar remains local player only.

Score screen:

- Add Team column to per-player score rows.
- Highlight every row on `winnerTeamId`.
- Keep `winnerId` highlighting only for singleton-team compatibility.

## Files to Touch

- `DESIGN.md`
- `client/src/state.js`
- `client/src/input.js`
- `client/src/hud.js`
- `client/src/renderer.js`
- `client/src/minimap.js`
- `client/src/main.js`
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs`

## Tests

Add client-side tests where practical:

- `isAllyOwner` and `isEnemyOwner` classify based on start payload teams.
- Box selection skips allied units.
- Single-click can select an allied entity for inspection.
- Command card does not produce command buttons for allied-only selection.
- Right-click allied entity sends move, not attack.
- Score table renders Team column and highlights all winning-team rows.

Run:

```bash
node tests/client_contracts.mjs
node tests/client_smoke.mjs
```

## Acceptance Criteria

- Allied units are not attackable through normal UI.
- Allied units are not group-selectable or commandable.
- Allied single-click inspection works.
- Score UI clearly indicates teams.
- Local resource/supply UI remains local player only.
