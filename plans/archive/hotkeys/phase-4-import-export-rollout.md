# Phase 4 - Import Export and Rollout Hardening

Status: Implemented

## Goal

Complete the first-version feature by adding player-facing import/export flows, diagnostics polish,
and broad verification. This phase should remove obsolete hard-coded hotkey assumptions and make the
feature safe to leave enabled by default. It should not expand scope into replay-specific or global
game hotkeys.

## Scope

- Add export flow that writes hotkeys-only JSON plus metadata.
- Add import flow that validates JSON, rewrites local metadata as needed, and stores imported
  payloads as named custom profiles.
- Ensure imports replace the target profile payload rather than merging individual bindings.
- Add conflict and validation diagnostics that can be inspected when runtime receives an impossible
  duplicate.
- Update smoke and contract tests that assumed Grid-only `data-hotkey` labels.
- Polish keyboard capture, focus handling, and accessibility for rebinding and settings tabs.
- Update relevant documentation if implementation changes public client module contracts.

## Expected Touch Points

- Hotkey profile service and editor modules
- Settings modules
- `client/src/hud_command_card.js`
- `client/src/hud.js`
- `client/src/input/commands.js`
- `client/index.html`
- `client/styles.css`
- `docs/design/client-ui.md` if exported client module contracts change
- `docs/context/client-ui.md` if section pointers shift
- `tests/client_contracts.mjs`
- `tests/hud_command_card.mjs`
- `tests/client_smoke.mjs`

## Design Notes

- Exported files should contain `schemaVersion`, `profileId`, `name`, `description`,
  `createdWithBuild`, `basePreset`, and `bindings`.
- `profileId`, `name`, and `description` may be rewritten on import to avoid local collisions.
- `createdWithBuild` is informational only.
- Unknown command identities should be reported as warnings and ignored.
- Invalid keys and same-context conflicts are fatal until changed.
- Runtime duplicate fallback should remain first-visible command wins, but diagnostics should make
  the invalid profile easy to find and repair.

## Verification

- Add tests for export shape, import metadata rewrite, replacement semantics, unknown command
  warnings, fatal invalid keys, and fatal same-context conflicts.
- Run `node tests/hud_command_card.mjs`.
- Run `node tests/client_contracts.mjs`.
- Run `node scripts/check-client-architecture.mjs`.
- Run `node tests/select-suites.mjs --verify`.
- Run relevant client smoke and live Node suites selected for client changes.

## Manual Testing Focus

- Export a custom profile, import it under a new name, activate it, and verify command labels and
  activation.
- Import a profile with an unknown command and confirm the warning is non-fatal.
- Import a profile with an invalid key or same-context conflict and confirm save/activation is
  blocked until fixed.
- Test settings from lobby, live match, spectator, and replay after import/export changes.
- Confirm no replay-specific hotkeys or global army hotkeys were added.

## Handoff Expectations

The final handoff should summarize the complete player-facing feature, list verification commands
and results, provide factual patch-note bullets, and name any follow-up work intentionally left out
of the first version. It should also state whether all phase documents have been marked done in
their implementation commits.
