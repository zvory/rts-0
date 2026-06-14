# Phase 3B - AI and Prediction Fail-Closed Policy

Status: Done.

## Objective

Make unsupported faction behavior explicit for AI seats and client prediction before any
non-Kriegsia gameplay is available.

## Scope

- Keep AI seats Kriegsia-only. Clients must not be able to create an AI as `ekat`,
  `phase2_empty_fixture`, or any unknown faction.
- Keep `addAi` team-only in the Phase 3 protocol. If a hand-built or future client sends extra
  faction fields, lobby validation must ignore or reject the unsupported request without creating a
  non-Kriegsia AI seat.
- Emit a clear host-visible notice for unsupported AI faction requests if such requests become
  representable. Existing invalid `addAi(teamId)` behavior may remain silent where no faction was
  requested.
- Disable prediction only when the **local player** is on an unsupported faction. Unsupported
  remote opponents do not by themselves disable local prediction.
- Make the prediction disable reason explicit in client diagnostics, using a stable reason such as
  `unsupported-local-faction`.

## Architecture Direction

Prediction compatibility should be decided before `SimWasmPredictionAdapter` initializes. Prefer
one source of truth in `predictionCompatibility(startInfo)`, with `GameState` exposing normalized
player faction identity and the adapter receiving already-approved start info. Server metadata may
continue to advertise prediction build/version only for supported live active players; the client
must still defensively check the local player's faction from `startInfo.players`.

## Expected Touch Points

- `server/src/lobby/`
- `server/crates/protocol/src/lib.rs` and protocol docs only if the wire shape changes
- `client/src/match.js`
- `client/src/state.js`
- `client/src/sim_wasm_adapter.js` only if needed to prevent initialization
- Focused AI and prediction tests

## Verification

- AI integration test proving AI seats default to Kriegsia and cannot be assigned unsupported
  factions.
- Client contract or prediction test proving a local unsupported faction records
  `unsupported-local-faction` and does not initialize WASM prediction.
- Client test proving a Kriegsia local player does not lose prediction solely because another start
  player has an unsupported faction.

## Manual Testing Focus

Start a normal match with prediction enabled and confirm current prediction behavior remains
available for Kriegsia.
