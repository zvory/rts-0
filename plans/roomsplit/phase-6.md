# Phase 6 - Live And Branch Handler Split

Status: not started.

## Goal

Move live-match and replay-branch room-owned handlers into dedicated child modules while preserving
command authority, spectator visibility, branch seat mapping, and existing live tick helpers.

## Scope

- Move live command handling, command receipts, live pause state sends, pause/unpause limits,
  give-up, late live spectator attach, late spectator notices, live start payload glue, live tick
  entrypoint calls, defeat notices, active-seat lookup helpers, and live command issuer helpers into
  `room_task/live.rs` where practical.
- Move branch-staging joins, branch-live attaches, branch seat claim/release handlers, branch start
  requests, branch announcements, branch staging message construction/broadcast calls, and branch
  live launch glue into `room_task/branch.rs`.
- Continue using `live_tick.rs` for the actual live simulation tick driver and `replay_branch.rs` for
  reusable branch staging state and launch preparation.
- Keep root `handle_event` as the event map and root `RoomTask::run` as the only Tokio owner.
- Update live and branch tests only for module path changes or support helper imports.

## Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/room_task/live.rs`
- `server/src/lobby/room_task/branch.rs`
- Live/branch-focused room-task tests
- `plans/roomsplit/phase-6.md`

## Constraints

- Do not change command issuer resolution, command ack sequencing, defeated-player command rejection,
  live pause authorization, pause limits, give-up outcome behavior, or late spectator read-only
  payloads.
- Do not change branch original-seat mapping for commands, snapshots, scores, or give-up outcomes.
- Do not change branch staging claim/release exclusivity, all-seat readiness, host fallback, or
  unsupported recorded-faction rejection.
- Do not move `live_tick.rs` logic back into the room task or into `Game`.
- If branch or live start payload construction changes beyond pure movement, run protocol parity.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server command`
- `cargo test --manifest-path server/Cargo.toml -p rts-server pause`
- `cargo test --manifest-path server/Cargo.toml -p rts-server spectator`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `node scripts/check-lobby-architecture.mjs`
- `node tests/protocol_parity.mjs` if live or branch message construction changes beyond pure movement
- `git diff --check`

## Manual Testing Focus

Manually check normal live match start, live commands from active players, defeated-player command
rejection, pause/unpause, give-up, active and spectator start payloads, late spectator join, branch
room seat claiming, branch launch, and branch live control of original seats.

## Handoff

After implementation, mark this phase done and summarize the live/branch handler map, commands run,
manual checks performed or still needed, any authority/fog checks inspected, and any lifecycle code
left for Phase 7.
