# Phase 6 - Lab Timeline Seek And Rebuild

## Phase Status

- [ ] Pending.

## Objective

Enable shared lab timeline seek by rebuilding authoritative lab state from room-local keyframes and
recorded lab timeline entries.

## Work

- Advertise lab room-time relative seek, absolute seek, and timeline capabilities only after the
  server rebuild path is implemented and covered.
- Implement relative and absolute seek for labs through existing `seekRoomTime` and
  `seekRoomTimeTo` messages. Do not add lab-specific seek operations to `LabClientOp`.
- Validate that the requester is a connected lab operator before applying seek. Since every direct
  lab joiner is currently an operator, this should remain a role check rather than a permission
  matrix.
- Restore the nearest lab keyframe at or before the target tick, replay recorded lab operations and
  issue-as commands in order, and tick the authoritative game forward until the target tick.
- Re-send lab `start` payloads or another clear reset signal if needed so clients discard stale
  assumptions after a rebuild. Then broadcast `roomTimeState`, recipient-specific lab state, and fresh
  snapshots.
- Preserve per-operator lab vision across seeks. Seeking changes the world time, not each
  collaborator's chosen projection.
- Truncate future timeline entries and keyframes when a new lab operation or issue-as command is
  accepted after seeking into the past. Do not add branch, redo, or undo UI in this plan.
- Rate-limit or reject excessive seek requests using replay seek as the model, and return clear
  errors without panicking or blocking the room task indefinitely.

## Expected Touch Points

- `server/src/lobby/lab_timeline.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/tick_control.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/live_tick.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab_timeline`
- `cargo test --manifest-path server/Cargo.toml -p rts-server room_time`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/protocol_parity.mjs` if capability shape or docs change
- `git diff --check`

Tests should prove at least: seeking restores entity state, accepted lab operations replay in order,
issue-as commands replay through normal command validation, future history truncates after a past
seek plus new operation, and invalid/stale ids return structured errors instead of panicking.

## Manual Test Focus

Open a lab, spawn units, issue movement or attack commands, pause, seek backward, and confirm the
world returns to the earlier state for all connected operators. After seeking backward, spawn or move
something new and confirm the old future cannot be reached through the timeline.

## Handoff Expectations

Summarize seek semantics in player-facing language, list any retained rebuild limits, and state
whether the UI in Phase 7 can rely entirely on `RoomTimeCapabilities` and `roomTimeState`.
