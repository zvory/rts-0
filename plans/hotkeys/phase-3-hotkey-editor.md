# Phase 3 - Hotkey Editor

Status: Not Started

## Goal

Build the first-version hotkey editor in the unified settings surface. Players should be able to
select command-card contexts, inspect the rendered command card, click a command to rebind its
command identity, and save only valid complete custom profiles. The editor should use the same
descriptor/context system that the HUD uses.

## Scope

- Add a Hotkeys tab that lists units, buildings, production cards, research/upgrade entries, and
  other command-card contexts from the Phase 0 catalog.
- Render a command-card preview for the selected context.
- Start rebinding by clicking a visible command-card command.
- Normalize allowed single-key bindings and reject unsupported keys.
- Show missing, unresolved, unknown, invalid, and same-context conflict warnings with affected
  contexts.
- Support creating a custom profile from a preset or from scratch.
- Support profile name and description editing for custom profiles.
- Apply selected profile changes immediately across lobby and match contexts after valid save.

## Expected Touch Points

- Settings modules from Phase 2
- Hotkey profile service from Phase 1
- Command-card/context catalog from Phase 0
- `client/src/hud_command_card.js`
- `client/src/hud.js`
- `client/src/input/commands.js`
- `client/styles.css`
- New focused tests for editor behavior
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs`

## Design Notes

- The editor preview should not duplicate command-card selection rules. It should request
  descriptors from the same builder/catalog used by the live HUD.
- Command-card button locations must remain fixed while rebinding.
- If a profile being edited becomes invalid, keep it as an editing draft and block saving as the
  active normal profile until resolved.
- Multiple commands may share a key only when the context catalog proves they cannot appear in the
  same rendered command card.
- Search by command name is optional and should not displace required validation or conflict UI.
- Reset can stay simple: create a new custom profile from a preset or from scratch.

## Verification

- Add tests for click-to-rebind, valid save, invalid save blocking, conflict messages, unresolved
  binding display, and immediate apply after profile selection.
- Add tests that same-key bindings are allowed only across mutually exclusive contexts.
- Add tests that labels/tooltips update without moving slots.
- Run `node tests/hud_command_card.mjs`.
- Run targeted `node tests/client_contracts.mjs`.
- Run `node scripts/check-client-architecture.mjs`.
- Run client smoke coverage for opening settings and using at least one changed hotkey.

## Manual Testing Focus

- Clone Grid and Classic RTS into custom profiles.
- Rebind Move, Attack, Stop, worker Build, a train command, a research command, and an ability.
- Confirm command-card positions do not change after rebinding.
- Confirm conflicts explain which command-card contexts are affected.
- Confirm a saved profile applies immediately in lobby-created matches and live matches.

## Handoff Expectations

The handoff should list editor capabilities, known UX limitations, validation gaps if any, and the
manual contexts tested. It should tell Phase 4 what import/export and rollout polish remains.
