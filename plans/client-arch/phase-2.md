# Phase 2 - Neutral Command Targeting Boundary

## Phase Status

- [x] Implemented.

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

## Implementation Segments

Mark each segment complete as it lands:

- [x] Move `command_composer.js` to the neutral client path without changing its public API.
- [x] Update all client, test, and docs imports or path references.
- [x] Remove the old architecture-checker allowlist entry if Phase 1 has landed.
- [x] Run command-targeting, minimap, and context-menu contract tests.
- [x] Run client smoke when practical and record any skipped verification with a reason.

## Verification

- `rg "input/command_composer|./command_composer|../client/src/input/command_composer" client tests docs`
- `node tests/client_contracts.mjs`
- `node tests/minimap_input_contracts.mjs`
- `node tests/input_context_menu_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- Client smoke when practical:
  - start the server
  - `node tests/client_smoke.mjs`

## Manual Test Prompt

At handoff, ask the user to do this quick browser check:

> Manual testing requested, 5-10 minutes:
> 1. Start a match and select units that can receive targeted commands.
> 2. Use a command-card targeted ability/order, then click in the world and confirm the order still
>    issues.
> 3. Shift-issue a targeted order and confirm queued behavior still works.
> 4. Click the minimap to issue or route a command if the selected units support it.
> 5. Report any stuck targeting cursor, lost Shift queueing, or console error.

## Handoff Expectations

In the final handoff, include the completed segment checklist, exact verification output summary,
and the filled manual testing prompt above. Tell the next agent to start Phase 3 only after this
phase is committed, merged to `main`, and pushed.

## Safety Notes

This phase should be a path move plus import updates only. It is intentionally conservative: the
same class, methods, and tests should continue to prove behavior. Do not combine this with command
target feature work.

## Outcome

No gameplay or visual change. `GameState` no longer depends on the `input/` area, so command
targeting can evolve without turning the shared model into an input facade.
