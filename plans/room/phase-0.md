# Phase 0 - Retire Dev Self-Play Watch

## Phase Status

- [ ] Done.

## Objective

Remove the live `/dev/selfplay` watch path before the room refactor starts, while preserving the
AI self-play harness and leaving dev scenarios alone.

## Work

- Remove the live `/dev/selfplay` entry point and `watchSelfplay` live auto-join behavior. A normal
  lobby with AI players plus spectator clients should be the replacement for watching AI versus AI.
- Remove the hidden live self-play room mode, including `RoomMode::DevSelfPlay::Live`,
  `DevSelfPlayConfig::Live`, `LiveSelfPlay` ownership from `RoomTask`, live self-play join/start
  branches, and live self-play restart-on-one-alive behavior.
- Preserve `server/crates/ai/src/selfplay` and self-play replay artifact generation. Those are
  still useful for automated AI tests and diagnostics.
- If browser inspection of saved self-play artifacts is still needed, migrate it to a neutral
  replay-artifact entry point that uses the shared replay viewer path. Do not keep a
  `DevSelfPlay::Replay` mode just to load files from `target/selfplay-artifacts` or
  `target/selfplay-failures`.
- Remove or update UI links, bootstrap parsing, docs, tests, and diagnostic messages that point
  users at `/dev/selfplay`.
- Leave `/dev/scenario`, `/dev/scenarios`, `watchScenario`, `RoomMode::DevScenario`,
  `DevScenarioDriver`, `ReplayState` pause/step controls for dev scenarios, and tri-state dev
  scenario harness usage unchanged.

## Expected Touch Points

- `server/src/main.rs`
- `client/src/bootstrap.js`
- `client/index.html`
- `server/src/lobby/dev_replay.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/tests.rs`
- `server/crates/ai/src/selfplay/tests.rs` only for browser-inspection URL updates
- `tests/client_contracts.mjs`
- `docs/context/testing.md`
- `docs/design/architecture.md`
- `docs/design/server-architecture-walkthrough.md`
- `docs/design/server-sim.md`

## Implementation Checklist

- [ ] Remove live `/dev/selfplay` routing and client bootstrap support.
- [ ] Remove `DevSelfPlay` live room mode and live self-play room-task branches.
- [ ] Preserve AI self-play automated tests and artifact writing.
- [ ] Provide a neutral saved-artifact replay inspection path or explicitly confirm that browser
      artifact inspection is no longer required.
- [ ] Update docs and tests that mention `/dev/selfplay`.
- [ ] Confirm `/dev/scenario` and `watchScenario` behavior is unchanged.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server dev_selfplay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev_scenario`
- `cargo test --manifest-path server/Cargo.toml -p rts-ai selfplay`
- `node tests/client_contracts.mjs`
- `node scripts/check-wiki.mjs`
- `git diff --check`

If a filtered command matches zero tests, add or use exact test names before counting it as
verification.

## Manual Test Focus

Create a normal lobby with AI players and two spectator browser clients to replace live self-play
watching. Open the saved-artifact replay inspection path if Phase 0 preserves one. Open one
`/dev/scenario` URL and confirm pause, step, speed, and initial full-world view still work.

## Handoff Expectations

Name the replacement for `/dev/selfplay?replay=<artifact>` if one remains, list every removed
self-play room entry point, and explicitly state that dev scenarios were not migrated in this
phase.
