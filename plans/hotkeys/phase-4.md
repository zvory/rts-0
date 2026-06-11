# Phase 4 - Hotkey Editor, Import, and Export

## Objective

Build the player-facing hotkey editor in the unified settings modal and complete import/export and
custom profile management.

## Scope

- Add Hotkeys tab UI for:
  - active profile selection
  - cloning immutable presets into custom profiles
  - creating a custom profile from scratch
  - naming and describing custom profiles
  - importing a profile as a new named custom profile
  - exporting the active/custom profile JSON
- Show a list of command-card contexts such as units, buildings, production cards, research, and
  ability contexts.
- Selecting a context displays its command card.
- Clicking a command-card command starts rebinding for that command identity.
- Show unresolved bindings during import/editing, but block saving until every known command is
  bound and conflicts are fixed.
- Show conflict warnings that name the affected command-card contexts and command labels.
- Keep search by command name out of the required first version, but leave room for it in layout and
  data flow.
- Do not add per-binding or per-category reset controls. Users can start from a preset or scratch.

## Likely Touch Points

- settings UI modules from Phase 1
- hotkey service modules from Phase 2
- command-card descriptor preview helpers from Phase 3
- `client/styles.css`
- targeted DOM/contract tests for the Hotkeys tab

## Verification

- DOM tests for profile selection, clone, create from scratch, rebind, validation warnings, import,
  and export.
- Tests proving invalid imports do not become active valid profiles.
- Tests proving imported valid profiles are stored as new named custom profiles.
- Tests proving profile switching updates rendered command-card hotkeys without a page reload.
- `node scripts/check-client-architecture.mjs`
- Browser smoke covering lobby and match settings interactions.

## Manual Testing Focus

Create a custom profile from Grid, rebind several commands, trigger a deliberate conflict and fix
it, export the profile, reload, import it under a new name, and confirm a live match uses the chosen
profile immediately. Also confirm invalid imports show actionable errors without corrupting the
current active profile.

## Handoff Expectations

The handoff should include sample exported JSON, identify any editor affordances intentionally left
for later, and list the profile-management paths manually tested.

## Player-Facing Outcome

Players can create, edit, import, export, and select command-card hotkey profiles from the unified
settings menu.

