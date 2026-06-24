# Phase 1 - Scenario Catalog and Selection

## Phase Status

- [x] Done.

## Objective

Replace hardcoded bundled lab scenario selection with a catalog that can grow as scenario JSON files
are added to the repo.

## Work

- Define the catalog source of truth for bundled lab scenarios. Prefer a small manifest next to
  `server/assets/lab-scenarios/*.json` with stable id, title, description, tags, map name, player
  count, and JSON filename.
- Preserve the existing `lategame` scenario id and default lab behavior unless the implementation
  explicitly chooses a clearer default with matching docs.
- Replace or wrap the current `LabScenarioPreset` enum path so new catalog entries do not require a
  hand-written Rust enum variant for each scenario.
- Add a server-side catalog loader that validates manifest entries, filename/id safety, duplicate
  ids, JSON parseability, and `LabScenarioV1` restore compatibility.
- Expose catalog entries to the browser through a bounded HTTP endpoint or existing launch metadata.
  Keep the full scenario JSON out of the listing unless a phase proves it is needed.
- Add a lab entry UI for choosing an existing scenario or blank setup before starting the room.
- Keep route compatibility for `scenario=lategame` and `scenario=blank`.
- Document the catalog contract in the relevant design docs if the phase creates a new source of
  truth or route/API shape.

## Expected Touch Points

- `server/assets/lab-scenarios/`
- `server/src/lobby/room_task/types.rs`
- `server/src/lobby/room_task/lab.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/main.rs` if adding an HTTP catalog endpoint
- `client/src/bootstrap.js`
- `client/src/app.js`
- `client/src/lab_panel.js` or a small lab catalog UI helper
- `client/src/protocol.js` only if a wire/start-payload shape changes
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `docs/design/protocol.md` if protocol/HTTP DTOs are documented there
- `tests/client_contracts/lab_contracts.mjs`

## Verification

- Focused Rust tests for catalog loading, duplicate/rejected ids, `scenario=lategame`, and
  `scenario=blank`.
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim lab_scenario`
- `node tests/client_contracts/lab_contracts.mjs`
- `node tests/protocol_parity.mjs` if any protocol or start payload shape changes.
- `git diff --check`

## Manual Test Focus

Open `/lab`, choose the existing lategame scenario, confirm it restores correctly, then start a
blank lab and confirm normal setup tools still work.

## Handoff Expectations

Describe the catalog file format, how a future scenario is made selectable, whether `lategame`
remained the default, and the next UI/validation work Phase 2 should pick up.
