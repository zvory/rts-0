# Phase 2 - Hotkey Model, Schema, and Presets

## Objective

Add the hotkey data model, schema validation, presets, local persistence, and import/export helpers.
This phase should make hotkey profiles testable without requiring the HUD/input path to fully
consume them yet.

## Scope

- Define stable command identity strings for every current command-card command.
- Define the command-card context model used for conflict detection.
- Define shared command identities for Move, Attack, and Stop across units.
- Add Grid and Classic RTS immutable presets.
- Add custom profile representation with name, description, base preset, bindings, and metadata.
- Store custom profiles and active profile selection in local storage.
- Normalize key values.
- Reject modifier customization in the schema for the first version.
- Add JSON import/export helpers using `schemaVersion: 1`.
- Validate:
  - missing known command bindings
  - unknown command identities
  - invalid keys
  - duplicate keys in the same rendered command-card context
  - unresolved new-command fallback cases
- Implement new-command fallback:
  - command-card slot key where conflict-free
  - first label letter where conflict-free
  - unresolved if neither works
- Keep unresolved states available only during migration/import/editing; saved valid profiles must
  bind every known command.

## Likely Touch Points

- new `client/src/hotkeys/` modules or an equivalent focused client area
- `client/src/hud_command_card.js` / descriptor code if command identities are added there
- `client/src/hud.js` if descriptors currently live there and need identity fields
- targeted tests under `tests/` or client-side contract tests
- `docs/design/client-ui.md` if new module contracts are exposed

## Verification

- Unit/contract tests for schema validation.
- Tests proving Grid derives keys from command-card slots.
- Tests proving duplicate keys are allowed across non-overlapping contexts and rejected within the
  same rendered context.
- Tests proving import warnings for unknown commands and fatal errors for invalid keys/conflicts.
- `node scripts/check-client-architecture.mjs`

## Manual Testing Focus

Use a temporary debug hook or settings placeholder to switch active profiles and export/import JSON.
Confirm custom profile metadata is preserved, local storage survives reload, and invalid imported
profiles show clear validation feedback.

## Handoff Expectations

The handoff should list all command identity strings, describe how rendered contexts are generated
for validation, and explain the hotkey service API that Phase 3 should consume.

## Player-Facing Outcome

No full gameplay change yet. The game can now store, validate, import, export, and select hotkey
profiles behind the settings surface.

