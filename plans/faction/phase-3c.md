# Phase 3C - Command Identity and Per-Faction Hotkeys

Status: Designed, not implemented.

## Objective

Define stable command identities and hotkey storage before faction-specific command cards expand.
Kriegsia and Ekaterina must not collide even if they both use the same hotkey labels or command
families.

## Scope

- Keep global tactical commands un-namespaced for now, including `unit.move`, `unit.attack`,
  `unit.stop`, support-weapon setup/teardown, and worker menu navigation.
- Keep global production control commands un-namespaced for now, including production cancel.
- Namespace faction-specific build, train, research, and ability command ids by faction id when
  they refer to a faction catalog action. The intended shape is:
  - `kriegsia.build.<kind>`
  - `kriegsia.train.<kind>`
  - `kriegsia.research.<upgrade>`
  - `kriegsia.ability.<ability>`
- Do not create Ekaterina command cards in this phase. Reserve the namespace so later Ekaterina
  commands can use `ekaterina.*` ids without colliding with Kriegsia.
- Store custom/direct hotkey bindings per faction. Grid mode remains global because it follows
  rendered command-card slot position rather than command id.
- Migrate existing custom profiles from old Kriegsia ids such as `build.city_centre` and
  `train.rifleman` into the Kriegsia binding set. Do not apply those custom bindings to Ekaterina.

## Hotkey Behavior

Profiles should support:

- One active global mode selection, such as grid versus direct/classic.
- Per-faction custom/direct binding maps.
- Unknown or unavailable faction command ids preserved on import/export when they are structurally
  valid, but inactive unless that faction's command catalog is active.
- Active gameplay only arms commands legal for the local player's current faction.

## Expected Touch Points

- `client/src/hud_command_card.js`
- `client/src/hotkey_profiles.js`
- `client/src/hotkey_editor.js`
- Client command-card and hotkey tests
- `docs/design/client-ui.md`

## Verification

- Existing Kriegsia command-card tests pass with namespaced build/train/research/ability ids.
- Hotkey tests prove old custom profile ids migrate into Kriegsia bindings.
- Hotkey tests prove Kriegsia custom bindings do not apply to Ekaterina.
- Hotkey import/export tests prove unavailable faction commands can be preserved but not armed.

## Manual Testing Focus

Open hotkey settings, customize a Kriegsia command, export/import the profile, and confirm ordinary
Kriegsia command-card hotkeys still work.
