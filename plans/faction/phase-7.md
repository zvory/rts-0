# Phase 7 - Client Faction Surface

Status: Designed, not implemented.

## Objective

Make the browser client render and command different faction catalogs using the generated or
mechanically checked client mirror. Preserve the current HUD while adding fixture-faction coverage
for alternate resources, build menus, production, and abilities.

## Scope

- Make client config/catalog access faction-aware.
- Consume generated or mechanically checked faction data rather than hand-maintaining divergent
  client descriptors.
- Support faction-specific build menus instead of one global `WORKER_BUILDABLE` list.
- Support faction-specific train/research/ability buttons.
- Update HUD resource rendering for the generic resource payload introduced in Phase 3.
- Add visual fallbacks for unknown or fixture units/buildings so protocol additions do not render
  blank.
- Update hotkey profile behavior so new faction command ids are stable and do not collide
  accidentally.
- Disable prediction in the client when the start payload says the selected faction/resource model
  is unsupported by WASM.
- Keep no-framework/no-build-step client conventions.
- Preserve current faction command card DOM/classes/hotkeys unless a documented migration is
  required.

## Expected Touch Points

- `client/src/config.js`
- generated or checked client catalog artifacts/scripts
- `client/src/protocol.js`
- `client/src/state.js`
- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/input/`
- `client/src/renderer/units.js`
- `client/src/renderer/buildings.js`
- `client/src/hotkey_profiles.js`
- `client/src/match.js`
- `client/src/sim_wasm_adapter.js`
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `tests/protocol_parity.mjs`
- `docs/design/client-ui.md`

## Verification

- Command-card descriptor tests for current faction parity.
- Command-card descriptor tests for fixture faction build/train/research/ability cards.
- Client protocol parity tests for faction/resource/ability fields.
- Generated-client-catalog or parity tests proving JS descriptors match Rust.
- Hotkey profile tests for new command ids.
- Prediction-disable test for unsupported non-default factions.
- Client architecture checker if imports or module boundaries change.
- Client smoke test when visible HUD/rendering behavior changes.

## Manual Testing Focus

Start current-faction debug mode and verify the command card, build placement, resources, training,
researching, ability buttons, and prediction status. If fixture faction is exposed in a dev path,
verify its resource display and command card show only fixture-legal actions.

## Handoff Expectations

The handoff must identify the client catalog entry points, generated/parity-check command, added
descriptor tests, prediction-disable behavior, and any visual fallbacks that are intentionally
temporary. It should tell Phase 8 what real faction UI data the brief/spec must provide.

## Player-Facing Outcome

The current UI should look and behave unchanged. The client becomes capable of presenting a
different faction's tech tree, resource set, and ability set through data-backed descriptors.
