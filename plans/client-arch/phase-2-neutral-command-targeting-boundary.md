# Phase 2 - Neutral Command Targeting Boundary

## Objective

Remove the current concrete boundary inversion where `GameState` imports an input implementation
detail. Command target arming is shared client UI state, so its pure state machine should live in a
neutral module instead of under `input/`.

## Work

- Move `client/src/input/command_composer.js` to `client/src/command_composer.js`.
- Update imports:
  - `client/src/state.js`
  - `tests/client_contracts.mjs`
  - `tests/minimap_input_contracts.mjs`
  - any other direct imports found with `rg "command_composer"`
- Keep the exported class name and public methods unchanged.
- Do not change command-target semantics, hotkeys, Shift preservation, minimap behavior, or HUD
  command-card behavior.
- Update `docs/design/client-ui.md` and `docs/context/client-ui.md` if either names the old path.
- If Phase 1 has landed, remove the `state -> input/command_composer.js` allowlist entry instead of
  adding a permanent exception.

## Verification

- `rg "input/command_composer|./command_composer|../client/src/input/command_composer" client tests docs`
- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node tests/input_context_menu_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- Client smoke when practical:
  - start the server
  - `node tests/client_smoke.mjs`

## Safety Notes

This phase should be a path move plus import updates only. It is intentionally conservative: the
same class, methods, and tests should continue to prove behavior. Do not combine this with command
target feature work.

## Outcome

No gameplay or visual change. `GameState` no longer depends on the `input/` area, so command
targeting can evolve without turning the shared model into an input facade.
