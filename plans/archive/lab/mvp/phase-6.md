# Phase 6 - Scenario Import and Export

## Phase Status

- [x] Done.

## Objective

Add versioned lab scenario JSON import/export so useful staged setups can be saved, reviewed,
reloaded, and shared without server-side public storage.

## Work

- Define legacy setup JSON as authoritative setup data with schema version, kind, name, map identity,
  seed, players, player state, entities, and lab metadata.
- Keep the scenario format intentionally smaller than `Snapshot`; exclude transient events,
  compact transport details, fog-filtered recipient projections, client interpolation state,
  projectile runtime state, and operation result metadata unless a concrete scenario needs them.
- Implement export from the current lab `Game` plus room lab metadata.
- Implement import validation and restore through public `Game` lab APIs. Import should remap JSON
  stable ids to current runtime ids and return an id map or summary where useful.
- Add browser import/export controls using file download/upload or copy/paste JSON. Do not write
  arbitrary files on the server.
- Optionally support bundled read-only examples through a typed store. The server must validate
  scenario names and read only from a known bundled scenario directory.
- Keep local-dev server-side save out of the MVP unless it fits cleanly behind the same typed
  `LabScenarioStore`; public production save-to-disk should remain out of scope.
- Add round-trip tests for map, players, teams, resources, upgrades, entities, ownership, and lab
  vision metadata.

## Expected Touch Points

- `server/crates/sim/src/game/lab.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/lab_scenarios.rs` or similar typed room/server helper
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/lab_client.js`
- `client/src/lab_panel.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- `tests/protocol_parity.mjs`

## Implementation Checklist

- [x] Define legacy setup JSON with schema version and stable, legible JSON fields.
- [x] Export authoritative setup state without snapshot-only projection data.
- [x] Import and validate scenario JSON with bounded size, names, ids, kinds, coordinates, players,
      teams, resources, upgrades, and entity state.
- [x] Restore a coherent lab `Game` and lab metadata through public APIs.
- [x] Return clear import errors and useful remap/result summaries.
- [x] Add browser export/download and import/upload controls.
- [x] Skip optional bundled read-only scenarios for the MVP; browser JSON import/export is the
      scenario store for this phase.
- [x] Add scenario round-trip and invalid-schema tests.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab_scenario`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_scenario`
- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `git diff --check`

If the exact filters do not exist yet, add narrowly named scenario tests and use those names.

## Manual Test Focus

Create a lab setup, export JSON, start a fresh lab, import the JSON, and confirm the map, teams,
resources, research, entities, ownership, and vision mode are restored well enough to continue
issuing real commands.

## Handoff Expectations

Document the scenario schema, import/export limits, and any fields intentionally omitted from the
MVP. State whether bundled read-only examples were added and where they live.
