# Phase 5 - Replay And Room-Time Split

Status: done.

## Goal

Move replay viewer and replay room-time event handling into `server/src/lobby/room_task/replay.rs`
while preserving replay playback, vision, seek, and prompt behavior.

## Scope

- Move replay viewer joins, dedicated replay room joins, replay join prompts, replay session access,
  replay start payload stamping and sends, replay room-time state sends, observer analysis sends,
  replay snapshot fanout calls, replay ticks, replay speed/step/seek controls, replay vision
  controls, and replay return-to-lobby behavior.
- Continue using `replay_session.rs` for artifact validation, replay `Game` rebuild, command cursor,
  keyframes, seek cooldown, speed clamping, and per-viewer vision storage.
- Keep root `handle_event` dispatching replay events to moved `pub(super)` methods.
- Keep post-match transition and match-end capture in lifecycle code until Phase 7 unless a tiny
  replay helper is needed to send replay starts.
- Update replay tests only for module path changes or split helper imports.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/replay.rs`
- Replay-focused room-task tests
- `plans/roomsplit/phase-5.md`

## Constraints

- Do not change replay artifact validation, replay command playback, keyframe policy, seek cooldown,
  speed limits, per-viewer replay vision, or replay start capabilities.
- Do not change replay-room prompt/confirm semantics or return-to-lobby behavior for dedicated,
  persisted, saved artifact, or post-match replay rooms.
- Do not change fog behavior; replay snapshots must continue to use the existing projection helpers.
- If any replay wire message construction is more than a mechanical move, read
  `docs/context/protocol.md` and run protocol parity.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server seek`
- `node scripts/check-lobby-architecture.mjs`
- `node tests/protocol_parity.mjs` if replay message construction changes beyond pure movement
- `git diff --check`

## Manual Testing Focus

Manually check persisted replay room join prompt and confirm, post-match replay transition, replay
pause/speed/step, seek-to and seek-back controls, replay vision selection, observer analysis after
seek, and return-to-lobby behavior.

## Handoff

After implementation, mark this phase done and summarize the replay handler map, commands run,
manual replay checks performed or still needed, and any replay lifecycle behavior intentionally left
for Phase 7.
