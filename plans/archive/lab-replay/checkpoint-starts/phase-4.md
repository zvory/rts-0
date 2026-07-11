# Phase 4 - Replay Artifact Schema Break

> [!WARNING]
> **POTENTIALLY STALE PHASE - DO NOT IMPLEMENT YET.**
> This phase belongs to a lab-replay subdivision that may change after `plans/archive/game-state/plan.md`
> lands. Re-evaluate it before implementation.

Status: POTENTIALLY STALE - not started. Re-evaluate after `plans/archive/game-state/plan.md` lands.

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
It is acceptable for beta match-history replay buttons or dev replay artifacts to have a short
dead zone while the schema break and new capture path land across separate PRs. The phase must still
fail with understandable unsupported-schema messages rather than panics or partial playback.

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
