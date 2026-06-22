# Phase 3 - Lobby Control Split

Status: not started.

## Goal

Move ordinary lobby roster and control handlers into `server/src/lobby/room_task/lobby.rs` while
keeping the root event dispatcher readable and stable.

## Scope

- Move public lobby summary construction, normal join/leave handling, readiness, start request
  admission, host reassignment, team assignment, faction assignment, AI seat management, quickstart,
  map selection, spectator toggles, lobby broadcasts, and related roster helpers.
- Keep mode-specific joins for replay rooms, branch rooms, lab rooms, dev watch, and live spectators
  out of this phase unless a tiny normal-lobby helper must call them unchanged.
- Leave `RoomTask::handle_event` in the root file and have it call `pub(super)` methods from
  `lobby.rs`.
- Keep `session_policy.rs`, `participants.rs`, and `faction_validation.rs` as collaborators rather
  than duplicating their policy logic.
- Update tests only for module visibility or fixture path changes caused by the split.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/lobby.rs`
- `server/src/lobby/room_task/types.rs` if roster type visibility needs adjustment
- Lobby-focused room-task tests
- `plans/roomsplit/phase-3.md`

## Constraints

- Do not change who may join, ready, start, spectate, move teams, choose factions, add/remove AI, or
  select maps.
- Do not change public lobby browser filtering or summary fields.
- Do not change default team assignment, default faction assignment, AI colors, host fallback, or
  full-lobby spectator behavior.
- Moved methods should be `pub(super)` only when called from `handle_event` or another sibling module.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo test --manifest-path server/Cargo.toml -p rts-server faction`
- `node tests/protocol_parity.mjs` only if lobby message construction changes
- `node scripts/check-lobby-architecture.mjs`
- `git diff --check`

## Manual Testing Focus

Manually check creating a normal room, joining as multiple humans, ready/start behavior, spectator
toggle, team/faction changes, AI add/remove/profile changes, quickstart, map selection, and the
public lobby browser row.

## Handoff

After implementation, mark this phase done and summarize the lobby handler map, commands run, any
lobby behavior deliberately left in the root file, and which live/replay/lab/branch joins the next
phases still need to move.
