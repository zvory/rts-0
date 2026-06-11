# Phase 3 - Command Card Integration

## Objective

Make the active hotkey profile drive command-card labels, tooltips, and keyboard activation. This
phase removes hard-coded command-card hotkey behavior from the live HUD/input path.

## Scope

- Add command identity metadata to every command-card descriptor.
- Resolve each descriptor's displayed key through the active hotkey profile.
- Preserve command-card slot locations exactly as they are.
- Make Grid resolve from slot location instead of direct command bindings.
- Update input activation to resolve hotkeys through command identities/context instead of scanning
  only hard-coded `data-hotkey` values.
- Keep `data-hotkey` or an equivalent DOM attribute as an output of the resolved profile so styles
  and tests can inspect rendered keys.
- Ensure production repeat behavior still works for train and production-cancel commands.
- Ensure targeted command behavior still works for Move, Attack, setup, and abilities.
- Remove hard-coded command-card fallback behavior such as direct special cases once the profile
  covers the command identity. Today `S` is a normal hard-coded Stop fallback; after this phase,
  Stop should come from the active profile like any other command-card command.
- If a runtime conflict reaches the command card, activate the first visible matching command and
  emit a diagnostic warning.

## Likely Touch Points

- `client/src/hud.js`
- `client/src/hud_command_card.js`
- `client/src/input/commands.js`
- `client/src/input/index.js`
- `client/src/input/placement.js`
- hotkey service modules from Phase 2
- command-card descriptor tests

## Verification

- Descriptor tests proving command identities and resolved hotkeys are present for unit, worker
  build, production, research, ability, and cancel cards.
- Input tests proving active custom bindings trigger the intended command.
- Regression tests proving Grid still matches current command-card slot behavior.
- Tests proving Stop no longer depends on a separate hard-coded `S` fallback.
- `node scripts/check-client-architecture.mjs`
- Run relevant client smoke or command-card DOM coverage.

## Manual Testing Focus

In a live match, test Grid and a custom profile across worker commands, worker build menu,
production, research if available, support-weapon setup, abilities, and cancel production. Confirm
button labels change with the profile and command-card locations do not move.

## Handoff Expectations

The handoff should identify any remaining hard-coded command-card keys and explain whether they are
intentional non-goals or follow-up bugs. It should also name the exact manual command paths that
were tested.

## Player-Facing Outcome

Selected hotkey profiles now control real gameplay command-card hotkeys. Grid remains the default
and behaves like the current command-card-position setup.

