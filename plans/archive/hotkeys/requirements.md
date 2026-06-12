# Hotkeys and Unified Settings Requirements

## Purpose

Players need first-class, editable command-card hotkeys instead of hard-coded key labels scattered
through the HUD and input code. Hotkey profiles must be importable, exportable, serializable, and
validated against a schema. The same settings surface should be reachable from lobby, live match,
and replay screens, with context-specific sections such as give up or debug controls appearing only
where they make sense.

## Product Requirements

- Hotkeys bind to command identities, not DOM buttons.
- Command-card button locations do not change when a hotkey changes.
- The default profile is Grid: command identities inherit the key for their current command-card
  slot, so when command-card layout changes, Grid automatically follows the new locations.
- Other profiles bind directly to command identities.
- Presets are immutable templates and can be cloned into custom profiles.
- Launch presets:
  - Grid, the default command-card-position layout.
  - Classic RTS, a direct command-identity profile using familiar RTS-style keys.
  - Custom profiles stored locally in the browser.
- Custom profiles have a name and description.
- Imported profiles are stored as new named custom profiles.
- Exported files contain hotkeys only, plus metadata.
- Imports replace the profile payload being imported into; they do not merge individual bindings.
- Hotkeys cannot be left unbound in normal saved profiles.
- Validation must warn about invalid, unknown, missing, or conflicting bindings.
- It must be impossible through the normal UI to save two visible command-card commands with the
  same key in the same context. If a conflict somehow reaches runtime, the first visible command
  wins and a validation/diagnostic warning should be available.
- Multiple commands may use the same key only when they cannot appear together in the same rendered
  command-card context.
- Modifier customization is out of scope for the first version.
- Physical keyboard layout handling is out of scope for the first version.
- Command-card hotkey bindings drive button labels and tooltip labels.
- Replay-specific hotkeys are out of scope for the first version.

## Command Context Model

The conflict model should be based on rendered command-card contexts, not broad entity categories.
This matters because mixed selections can expose commands from different unit types together, such
as support-weapon setup and artillery/scout abilities. If two command identities can be visible in
the same command-card render, they conflict when assigned the same key.

Shared basic unit commands are global within the command-card system:

- Move is one command identity shared by all units that can move.
- Attack is one command identity shared by all units or rally-capable production buildings that can
  attack/attack-move.
- Stop is one command identity shared by all units that can stop.

Those shared commands are configured once and cannot conflict with any other command that can be
visible beside them.

For dynamic mixed selections, the authoritative conflict source is the same command-card descriptor
builder the HUD uses. The editor should ask the descriptor system what command identities may appear
together, rather than duplicating selection rules in a separate hand-written table. If a mixed
selection has only one slot available for multiple possible commands, the command-card descriptor
rules decide which command appears; only the actually visible command participates in that rendered
context.

## New Command Fallback

When a new command identity appears in a custom profile:

1. Use the command's command-card slot key if that key is available in every context where the
   command appears.
2. Otherwise use the first letter of the command label if it is available in every context where the
   command appears.
3. Otherwise leave it unresolved for migration and force the profile editor/import validator to
   resolve it before the profile can be saved as valid.

The product rule is still that saved player profiles cannot have unbound hotkeys. The unresolved
state exists only during migration, import validation, and editing.

## Import and Export Schema

Exported hotkey profiles are JSON. The file is intended to be machine-validated, but users may edit
it manually if they understand the schema.

Required top-level shape:

```json
{
  "schemaVersion": 1,
  "profileId": "custom-example",
  "name": "Example Custom",
  "description": "Optional player-facing description.",
  "createdWithBuild": "dev-or-build-id",
  "basePreset": "grid",
  "bindings": {
    "unit.move": "M",
    "unit.attack": "A",
    "unit.stop": "S"
  }
}
```

Schema rules:

- `schemaVersion` is required.
- `profileId`, `name`, and `description` are metadata and may be rewritten on import to avoid local
  collisions.
- `createdWithBuild` is informational.
- `basePreset` records the preset the custom profile was cloned from.
- `bindings` maps stable command identity strings to normalized key names.
- Unknown command identities are ignored with a warning, not fatal.
- Missing known command identities are filled by the new-command fallback where possible.
- Invalid keys are fatal until changed.
- Same-context duplicate keys are fatal until changed.

## Unified Settings Requirements

The settings container and the settings panel content should be separate concepts. The first
implementation can use a modal with tabs, but the panel content should be portable enough to move to
a side panel later.

The settings surface is opened by the gear icon only. No settings hotkey is required until there is
a safe choice that avoids browser and OS conflicts.

Settings do not pause the match.

Initial tabs:

- Game
- Hotkeys
- Audio
- Debug, only when debug controls are available

The live-match settings title area should include the give up action. Give up should not appear for
spectators or replay viewers.

Debug settings may include controls such as movement waypoint overlays when those controls are
available in the current mode.

Audio settings should move into the unified settings panel without changing the underlying audio
volume behavior.

## Hotkey Editor Requirements

- The editor shows a list of units, buildings, production cards, research/upgrade entries, or other
  command-card contexts.
- Selecting an item displays its command card.
- Clicking a command-card command starts rebinding for that command identity.
- The editor may show unresolved bindings while editing or importing, but a saved valid profile must
  bind every known command.
- Search by command name is desirable but not required for the first version.
- Reset behavior can be simple: users may start a new custom profile from a preset or from scratch.
  Per-category and per-binding reset controls are not required.
- The editor must show conflict warnings with enough context for the player to understand which
  command-card contexts are affected.

## Storage Requirements

- Profiles and active-profile selection are stored in browser local storage for the first version.
- The selected profile applies immediately across lobby and match contexts.
- There is no account or server persistence requirement in the first version.

## Non-Goals

- Do not add replay hotkeys in this effort.
- Do not add global game hotkeys such as select idle worker or select all army in this effort.
- Do not add modifier-key customization in this effort.
- Do not add server/database persistence in this effort.
- Do not change command-card locations as part of hotkey editing.
- Do not add physical keyboard-layout support in this effort.

