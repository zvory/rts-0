# Phase 1 - Profile Schema, Presets, and Storage

Status: Implemented

## Goal

Implement the headless hotkey profile system before building the editor. Profiles should validate
against the command identity/context contract from Phase 0, persist locally, and resolve active
bindings for HUD labels and input activation. This phase may expose minimal diagnostics but does not
need the full settings editor.

## Scope

- Add immutable `Grid` and `Classic RTS` presets.
- Add local custom profile storage and active-profile selection in browser local storage.
- Add JSON schema-version handling for exported/imported profile payloads.
- Add validation for unknown identities, missing known identities, invalid keys, unresolved
  bindings, and same-context duplicate keys.
- Implement new-command fallback for custom profiles.
- Implement clone-from-preset behavior for custom profiles.
- Wire active profile resolution into command-card labels, tooltip labels, and command activation.
- Keep imported profile replacement semantics at the service level, even if the file picker UX waits
  until Phase 4.

## Expected Touch Points

- New hotkey profile/service modules under `client/src/`
- `client/src/hud_command_card.js`
- `client/src/hud.js`
- `client/src/input/commands.js`
- `client/src/app.js` and `client/src/match.js` for dependency injection of the active profile
  service
- `tests/hud_command_card.mjs`
- `tests/client_contracts.mjs`
- New focused Node tests for profile validation and storage

## Design Notes

- Grid should not serialize every command to fixed slot keys; it should resolve from the rendered
  slot so layout changes automatically update Grid.
- Direct profiles should map identity strings to normalized single-key names.
- Profile imports should replace the target profile payload, not merge per-binding changes.
- Unknown imported identities are warnings, not fatal. Missing known identities should use the
  fallback order: rendered Grid slot key, first letter of command label, then unresolved.
- Normal saved profiles cannot retain unresolved bindings.
- Presets should be immutable templates. Editing a preset should first clone it into a custom
  profile with name and description metadata.

## Verification

- Add tests for preset immutability, cloning, active-profile selection, local-storage read/write
  fallback, and schema-version parsing.
- Add tests for invalid keys, missing known commands, unknown commands, same-context conflicts, and
  unresolved migration state.
- Add tests that Grid follows changed descriptor slots while direct profiles do not.
- Run `node tests/hud_command_card.mjs`.
- Run targeted `node tests/client_contracts.mjs`.
- Run `node scripts/check-client-architecture.mjs`.

## Manual Testing Focus

- Switch active profile through a temporary debug hook or minimal selector if the full settings UI
  is not ready.
- Confirm Grid still behaves exactly like current command-card hotkeys.
- Confirm a direct custom binding changes button labels, tooltip labels, and activation without
  moving buttons.
- Confirm invalid or conflicting stored profiles fall back safely and surface diagnostics.

## Handoff Expectations

The handoff should identify the profile service API, storage keys, preset ids, validation result
shape, and any temporary UI/debug hooks added. It should also state whether Phase 2/3 can rely on
the profile service for immediate apply and save-blocking validation.
