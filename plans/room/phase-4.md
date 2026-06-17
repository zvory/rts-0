# Phase 4 - Clock And Tick Control

## Phase Status

- [ ] Done.

## Objective

Extract room clock and tick-control decisions into a neutral helper while keeping the room task as
the only event/tick owner.

## Work

- Add a lobby-local tick-control helper, for example `server/src/lobby/tick_control.rs`, that names
  realtime ticking, speed-controlled ticking, paused playback, stepped dev ticks, branch staging,
  countdown, and lobby no-op behavior.
- Preserve `current_tick_interval()` behavior exactly, including replay speed, dev watch pause,
  replay pause, branch staging interval behavior, and test interval overrides.
- Keep `RoomTask::run` and the Tokio interval in room ownership; this phase should only decide
  what the next tick should do and which speed multiplier applies.
- Route `SetReplaySpeed`, `StepDevTick`, replay seek pause interactions, dev watch pause, and
  countdown finishing through named tick-control methods only where doing so is narrow and
  behavior-equivalent.
- Do not rename client messages or add generic room-control protocol.

## Expected Touch Points

- `server/src/lobby/tick_control.rs` or similarly named lobby-local module
- `server/src/lobby/room_task.rs`
- `server/src/lobby/replay_session.rs` if a small adapter is needed for replay speed reads
- `server/src/lobby/tests.rs`

## Implementation Checklist

- [ ] Add tests for current tick interval decisions across normal, replay, paused replay, dev
      watch, paused dev watch, branch staging, and test interval override.
- [ ] Extract speed and tick-action decisions behind a small helper.
- [ ] Preserve countdown start timing and aborted countdown behavior.
- [ ] Preserve dev `StepDevTick` behavior and replay pause behavior.
- [ ] Confirm no tick path gains locks, blocking I/O, or sim ownership changes.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server tick`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo test --manifest-path server/Cargo.toml -p rts-server countdown`
- `git diff --check`

## Manual Test Focus

Normal match countdown start, live match ticking, replay speed controls, replay pause/seek, dev
watch pause/resume, and dev single-step.

## Handoff Expectations

Describe the tick-control helper shape, list which timing decisions remain in `RoomTask`, and state
whether Phase 5 can depend on stable per-mode tick classification.
