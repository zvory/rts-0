# Phase 4 - Replay Artifact Schema Break

Status: Not started.

## Scope

Replace the replay artifact schema with a checkpoint-backed shape:

```text
ReplayArtifact
  schemaVersion
  start: GameCheckpoint
  actions: ReplayAction[]
```

Old replay artifacts should be rejected with clear errors. Do not build migration or dual-read
support unless a later product decision reverses the compatibility stance.

## Expected Touch Points

- `server/crates/sim/src/game/replay.rs`
- `server/src/lobby/replay_session.rs`
- `server/src/lobby/dev_replay.rs`
- `server/src/db.rs`
- `docs/design/protocol.md`

## Verification

- Run focused replay artifact validation tests.
- Run a dev replay launch test for a newly generated checkpoint-backed artifact.
- Add a test that an old schema is rejected clearly.

## Manual Testing Focus

Open a newly generated replay artifact and verify old artifacts fail with an understandable message.

## Handoff

The handoff must state exactly what old replay behavior was intentionally broken.
