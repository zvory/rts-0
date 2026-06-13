# Phase 5 - Client Faction Surface

Status: Designed, not implemented.

## Objective

Make the browser client render and command different faction catalogs without breaking the current
HUD. This phase should turn server-side faction/economy/ability contracts into clear command cards,
resource displays, placement options, hotkeys, and visual fallbacks.

## Scope

- Make client config/catalog access faction-aware.
- Support faction-specific build menus instead of one global `WORKER_BUILDABLE` list.
- Support faction-specific train/research/ability buttons.
- Update HUD resource rendering for the chosen Phase 3 resource strategy.
- Add visual fallbacks for unknown or fixture units/buildings so protocol additions do not render
  blank.
- Update hotkey profile behavior so new faction command ids are stable and do not collide
  accidentally.
- Keep no-framework/no-build-step client conventions.
- Preserve current faction command card DOM/classes/hotkeys unless a documented migration is
  required.

## Expected Touch Points

- `client/src/config.js`
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/input/`
- `client/src/renderer/units.js`
- `client/src/renderer/buildings.js`
- `client/src/hotkey_profiles.js`
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `docs/design/client-ui.md`

## Verification

- Command-card descriptor tests for current faction parity.
- Command-card descriptor tests for fixture faction build/train/research/ability cards.
- Client protocol parity tests for new faction/resource/ability fields.
- Hotkey profile tests for new command ids.
- Client architecture checker if imports or module boundaries change.
- Client smoke test when visible HUD/rendering behavior changes.

## Manual Testing Focus

Start current-faction debug mode and verify the command card, build placement, training,
researching, and ability buttons. If fixture faction is exposed in a dev path, verify its resource
display and command card show only fixture-legal actions.

## Handoff Expectations

The handoff must identify the client catalog entry points, added descriptor tests, and any visual
fallbacks that are intentionally temporary. It should tell Phase 6 how to add real faction UI data
without editing command-card logic.

## Player-Facing Outcome

The current UI should look and behave unchanged. The client becomes capable of presenting a
different faction's tech tree and ability set through data-backed descriptors.

