# Phase 0 - Command Identity and Context Contract

Status: Complete

## Goal

Create the hard contract that future phases depend on: command-card descriptors must expose stable
command identities, rendered slot indexes, and resolved hotkey labels without changing current Grid
behavior. This phase should be mostly invisible to players. Its success criterion is that the
current command card still works exactly as before while tests can reason about command identities
and rendered contexts independently from DOM buttons.

## Scope

- Add stable command identity strings to every command-card descriptor.
- Keep slot placement independent from hotkey resolution.
- Preserve Grid hotkeys as the default by resolving the key from the rendered slot index.
- Add a descriptor/context catalog API that can enumerate representative rendered command-card
  contexts for validation.
- Add or update tests for descriptor identity, slot order, Grid fallback labels, and same-context
  duplicate detection inputs.
- Decide whether legacy private HUD render methods in `client/src/hud.js` remain supported or are
  retired; update tests intentionally either way.

## Expected Touch Points

- `client/src/hud_command_card.js`
- `client/src/hud.js`
- `client/src/input/commands.js`
- New focused hotkey/command-card helper modules under `client/src/` if needed
- `tests/hud_command_card.mjs`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs` if DOM `data-hotkey` assumptions change

## Design Notes

- Suggested identity examples: `unit.move`, `unit.attack`, `unit.stop`, `worker.buildMenu`,
  `worker.return`, `build.<kind>`, `train.<kind>`, `research.<upgrade>`, `ability.<ability>`,
  `unit.setupSupportWeapon`, and `production.cancel.<buildingKind>`.
- Descriptors should retain a rendered slot index or equivalent so Grid can follow layout changes
  automatically.
- Runtime activation may still use rendered buttons in this phase, but the button data should be
  derived from descriptor identity plus the active resolver, not hard-coded per descriptor.
- If a runtime conflict reaches a rendered card, activation should select the first visible matching
  command in DOM/render order and expose enough diagnostic information for later settings UI.
- The context catalog does not need every possible entity id combination. It needs representative
  rendered command-card contexts sufficient for validation, including mixed selections where
  support-weapon setup and abilities can appear beside shared move/attack/stop commands.

## Verification

- Run `node tests/hud_command_card.mjs`.
- Run targeted `node tests/client_contracts.mjs` coverage for command cards and input hotkeys.
- Run `node scripts/check-client-architecture.mjs`.
- If DOM hotkey attributes or smoke assumptions change, run the relevant client smoke path.

## Manual Testing Focus

- Select workers, army units, mixed support weapons, production buildings, and research buildings.
- Confirm the visible Grid labels are unchanged.
- Confirm Q/W/E/A/S/D/Z/X/C still activate the same visible commands as before.
- Confirm command-card positions do not move.

## Handoff Expectations

The handoff should list the stable command identities introduced, describe the context catalog API,
and call out any legacy HUD render methods or tests that were retired or kept. It should tell the
next agent whether Phase 1 can treat the descriptor contract as stable for profile validation.

## Handoff

- Stable command identities now live on rendered descriptors as `commandId`: `unit.move`,
  `unit.attack`, `unit.stop`, `worker.buildMenu`, `worker.return`, `build.<kind>`,
  `train.<kind>`, `research.<upgrade>`, `ability.<ability>`, `unit.setupSupportWeapon`, and
  `production.cancel.<buildingKind>`.
- Rendered descriptors expose `slotIndex` and resolve the visible Grid label from that slot through
  `gridHotkeyForSlot()`. Descriptor construction no longer hard-codes per-button hotkey labels.
- `buildCommandCardContextCatalog()` enumerates representative rendered cards for empty, worker,
  worker-build, mixed support/ability, production, and research contexts. `duplicateCommandIdsForCard()`
  and `commandCardActivationCandidates()` provide validation inputs for future profile checks.
- Runtime activation still uses DOM buttons by `data-hotkey` and therefore preserves current Grid
  behavior. Buttons rendered through the descriptor path now also expose `data-command-id` and
  `data-slot-index`; activation diagnostics return those values when present.
- Legacy private HUD render helpers remain supported because existing tests still exercise them.
  They are not the new command identity contract surface; Phase 1 should build profile validation
  against `hud_command_card.js` descriptors and the catalog API.
