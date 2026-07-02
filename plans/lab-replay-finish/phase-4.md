# Phase 4 - Retire Compatibility And Final Cleanup

Status: Done.

## Scope

Remove old compatibility paths only after the lab replay and checkpoint setup paths have proven out.
This phase is the deliberate cleanup pass for `LabScenarioV1`, replay artifact schema 2, stale docs,
and stale tests.

## Expected Touch Points

- `server/crates/protocol/src/lab_scenario.rs`
- `server/crates/sim/src/game/lab/scenario.rs`
- `server/src/lab_scenarios.rs`
- `server/crates/sim/src/game/replay.rs`
- `server/crates/sim/src/game/replay_artifact.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/lobby/room_task/replay.rs`
- `server/src/db.rs`
- `client/src/protocol.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/testing.md`
- Fixtures and tests under `tests/` and `server/crates/**/tests`

## Requirements

- Confirm the bake-in gate from `plan.md` is satisfied before deleting compatibility.
- Remove `LabScenarioV1` compatibility import/export code, protocol mirrors, stale docs, and tests.
- Remove replay schema 2 loading or replace it with a clear intentional rejection message if old
  artifacts are still expected to appear.
- Audit dev replay, self-play, crash replay, match-history, and committed fixture load paths.
- Update protocol parity and public-surface tests so the old DTOs cannot silently reappear.
- Document rollback behavior: old binaries rejecting schema 3, new binaries rejecting removed
  schema 2/lab scenario inputs, and what users should do with old artifacts.

## Keyframe Policy

Do not replace in-memory lab/replay keyframes as part of this cleanup unless there is a measured
need. Checkpoint keyframes are a possible future optimization for persisted seek points or
cross-process replay tooling, not a requirement for completing the lab replay migration.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-protocol`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-sim replay_artifact`
- `node tests/protocol_parity.mjs`
- Relevant client contract tests for lab and replay
- `git diff --check`

## Manual Testing Focus

Open one new schema 3 replay, one new lab replay, and one lab checkpoint setup. Then attempt an old
schema 2 replay and old `LabScenarioV1` import and verify they either load by an intentional
remaining policy or fail with clear compatibility messages.

## Handoff Notes

Name exactly what was deleted, what compatibility remains if any, and what old artifact behavior a
tester should expect.
