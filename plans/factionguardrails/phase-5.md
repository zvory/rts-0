# Phase 5 - Runtime Faction Surface Audit

## Phase Status

- [x] Done.

## Objective

Audit runtime faction acceptance, defaulting, exposure, and rejection surfaces.

## Work

- Confirm lobby validation matches the documented boundary.
- Prefer a data-shaped lifecycle policy table over broad `matches!(DEFAULT_FACTION_ID | EKAT...)`
  logic. Catalog existence alone must never grant AI, prediction, dev, replay, fixture, quickstart,
  or self-play support.
- Add or adjust tests for unknown ids, fixture-only ids, playable ids, AI defaults, replay
  validation, replay branch, dev scenarios, self-play, and prediction support.
- Add a table-driven test covering every `FactionRequestContext` by faction id, including unknown
  ids, fixture-only ids outside `TestFixture`, playable ids, AI seats, quickstart, self-play, dev
  scenarios, replay branch preservation, spectator/countdown/in-game `setFaction`, and prediction
  compatibility.
- Ensure client selector, command cards, hotkeys, and unknown-faction fallback do not inherit the
  wrong catalog accidentally.

## Expected Touch Points

- `server/src/lobby/faction_validation.rs`
- `server/src/lobby/room_task.rs`
- Replay validation paths if boundary checks differ
- `client/src/lobby*`
- `client/src/config.js`
- `client/src/hud_command_card.js`
- `client/src/hotkey_profiles.js`
- `client/src/prediction_compatibility.js`
- Faction-related tests

## Implementation Checklist

- [x] Inventory runtime surfaces that accept or default faction ids.
- [x] Add a table-driven test covering every `FactionRequestContext` by faction id.
- [x] Verify catalog existence alone never grants AI, prediction, dev, replay, quickstart,
      self-play, or fixture support.
- [x] Add focused negative and fixture tests.
- [x] Check client selector, command cards, hotkeys, and prediction compatibility.
- [x] Document lifecycle path decisions.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server faction_validation`
- `node tests/faction_integration.mjs` with a running server, if touched
- `node tests/client_contracts.mjs`
- `node tests/prediction_controller.mjs`

## Manual Test Focus

Start a local lobby, switch factions if exposed, add AI, start a match, and verify start payload and
player command cards match the boundary.

## Handoff Expectations

State which lifecycle paths accept which faction ids and which unsupported paths intentionally
default to Kriegsia.
