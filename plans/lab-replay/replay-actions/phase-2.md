# Phase 2 - Player Command Timeline Executor

Status: Not started.

## Scope

Apply normal player commands through the new `ReplayAction` executor. Newly captured match replays
should play through the checkpoint-backed artifact and action timeline with behavior equivalent to
the pre-refactor command replay. Keep seek, room time, fog perspectives, and viewer controls shared.

## Expected Touch Points

- `server/src/lobby/replay_session.rs`
- `server/src/lobby/room_task/replay.rs`
- Match replay capture code
- Replay tests

## Verification

- Run focused replay session tests.
- Run the checkpoint/replay characterization harness on at least one newly generated match replay.

## Manual Testing Focus

Open a match replay, seek forward and backward, and inspect both player fog perspectives.

## Handoff

The handoff must identify any replay viewer behavior that changed or was intentionally preserved.
