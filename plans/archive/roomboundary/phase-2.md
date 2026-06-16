# Phase 2 - Replay Runtime Extraction

## Phase Status

- [x] Done.

## Objective

Move replay session state and replay playback logic out of `room_task.rs` into a lobby-local module.

## Work

- Extract `ReplaySession`, replay keyframes, replay vision validation, seek logic, command cursor
  advancement, replay state payload construction, and start payload helpers.
- Keep room-owned decisions in `RoomTask`: phase transitions, membership, connection sends, and
  broadcast timing.
- Move or adapt tests so replay behavior remains well covered after private structs move.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- New `server/src/lobby/replay_session.rs` or `server/src/lobby/replay_runtime.rs`
- `server/src/lobby/mod.rs`
- Replay-related tests

## Implementation Checklist

- [x] Extract replay runtime types and methods.
- [x] Keep room-owned send/broadcast decisions in `RoomTask`.
- [x] Move focused replay tests with the extracted module.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo fmt --manifest-path server/Cargo.toml --check`
- `git diff --check`

## Manual Test Focus

Replay playback, pause/speed, seek, per-viewer vision, observer analysis after seek, and return from
replay if applicable.

## Handoff Expectations

Call out any replay code still embedded in `RoomTask` and why it remains room-owned.
