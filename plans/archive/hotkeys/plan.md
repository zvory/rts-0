# Hotkeys and Unified Settings Plan

## Purpose

Deliver first-class command-card hotkeys and a unified settings surface while preserving the current
server-authoritative gameplay model. This work is client-side unless a later phase discovers a
cross-file contract change; do not change wire protocol, balance mirrors, or server persistence for
this effort. The implementation should be staged so command identity, validation, settings shell,
and editor behavior can be verified independently.

## Investigation Findings

- Command-card descriptors already exist in `client/src/hud_command_card.js`, but descriptors embed
  `GRID_HOTKEYS` directly and do not expose stable command identities separately from rendered slots.
- Runtime hotkey activation currently scans `#command-card button[data-hotkey]` in
  `client/src/input/commands.js`, so the active key is a DOM-rendered label rather than a profile
  binding resolved from command identity.
- `client/src/hud.js` uses the descriptor builder for normal rendering, but legacy private render
  paths still exist and are covered by `tests/client_contracts.mjs`; implementation phases must
  either keep them compatible or retire/update their tests deliberately.
- Audio settings are built by `buildAudioSettings()` in `client/src/bootstrap.js`, while live-match
  give-up, pointer-lock, and debug controls are toggled directly from `Match`; there is no portable
  settings panel module yet.
- The authoritative source for rendered command-card contexts can be the existing descriptor
  builder, but it needs a testable catalog/context enumeration API before profile validation can
  detect same-context conflicts without duplicating selection rules.

## Phase Summaries

Phase 0 hardens the command-card contract before adding profile behavior. It introduces stable
command identities, separates slot placement from hotkey labels, and creates a context catalog from
the descriptor system for validation. The player-facing behavior should remain Grid hotkeys exactly
as today while tests prove command identity, slot order, and fallback conflict behavior are stable.

Phase 1 adds the hotkey profile model, schema validation, local storage, presets, migration, import,
and export as a headless service. It makes Grid the default slot-derived profile and adds Classic RTS
as an immutable direct-binding preset, with custom profile payloads stored locally. The player-facing
behavior should still look unchanged except diagnostics can now report invalid, missing, unknown, or
conflicting bindings.

Phase 2 extracts the unified settings container and portable panel content. It moves audio controls
into a tabbed settings surface, keeps the gear icon as the only opener, and mounts context-specific
live-match controls such as give up, pointer-lock, and debug options only when applicable. The
player-facing result is one settings modal usable from lobby, live match, and replay without pausing
the match.

Phase 3 builds the hotkey editor inside the unified settings surface. It lets players inspect
command-card contexts, click visible command buttons to rebind command identities, clone presets,
create named custom profiles, and resolve migration/import warnings before saving. The player-facing
result is editable hotkeys whose labels and tooltips update immediately while command-card positions
stay fixed.

Phase 4 completes import/export UX, validation polish, diagnostics, and rollout hardening. It adds
file import/export flows, conflict explanations with affected contexts, smoke coverage across lobby,
match, and replay settings, and cleanup of obsolete hard-coded hotkey assumptions. The player-facing
result is a stable first-version hotkey/settings feature with no replay-specific hotkeys or global
army-selection hotkeys added.

## Phase Index

0. [Phase 0 - Command Identity and Context Contract](phase-0-command-identity-contract.md)
1. [Phase 1 - Profile Schema, Presets, and Storage](phase-1-profile-schema-storage.md)
2. [Phase 2 - Unified Settings Container](phase-2-unified-settings-container.md)
3. [Phase 3 - Hotkey Editor](phase-3-hotkey-editor.md)
4. [Phase 4 - Import Export and Rollout Hardening](phase-4-import-export-rollout.md)

## Overall Constraints

- Preserve command-card slot locations when hotkeys change; bindings affect labels, tooltips, and
  activation only.
- Bind hotkeys to stable command identity strings, not DOM buttons, entity ids, or localized labels.
- Keep Grid as the default profile where command identities inherit the key for their current
  rendered command-card slot.
- Use the command-card descriptor system as the source for same-context conflict detection; do not
  maintain a parallel handwritten conflict matrix.
- Saved normal profiles must not contain unbound known commands; unresolved bindings are allowed
  only during migration, import validation, and editing.
- Unknown imported command identities should warn and be ignored, while invalid keys and same-context
  conflicts are fatal until fixed.
- Do not add modifier customization, physical keyboard layout support, replay-specific hotkeys,
  global game hotkeys, account persistence, server persistence, or command-card layout changes.
- Keep settings panel content portable enough to move from modal to side panel later; avoid coupling
  tab content directly to the current container markup.
- Settings do not pause live matches. Give up must not appear for spectators or replay viewers.
- Follow client architecture rules: plain ES modules, dependency injection through `App`/`Match`
  where practical, `destroy()` for modules with listeners/resources, and `node scripts/check-client-architecture.mjs`
  for client module changes.

## Verification Themes

- Add descriptor/DOM contract coverage for command identities, Grid slot-derived labels, direct
  profile labels, duplicate-key conflict detection, missing/unresolved bindings, and first-visible
  runtime conflict fallback.
- Add profile service tests for schema validation, preset immutability, cloning, local-storage
  persistence, migration fallback, import replacement semantics, and export shape.
- Add settings contract or smoke coverage for lobby, live match, spectator, replay, audio controls,
  debug controls, give up visibility, and Escape behavior.
- Run `node scripts/check-client-architecture.mjs` for client module changes.
- Run targeted Node contract tests during early phases; before feature completion, run the relevant
  client suites selected by `node tests/select-suites.mjs --verify` and the smoke path.

## Implementation and Handoff Rules

Implement one phase at a time. Each phase must be committed, merged to `main`, and pushed before the
next phase begins. When a phase is complete, mark that phase document as done in the same
implementation commit for that phase.

After implementing each phase, the implementing agent must provide a handoff message for the next
agent. The handoff must summarize what changed, list verification commands and results, identify
the next phase or follow-up work, and name the core features that should be manually tested. Manual
testing notes should cover the core changed features, not an exhaustive matrix.
