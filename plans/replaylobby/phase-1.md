# Phase 1 - Server Replay Lobby Contract

Status: Done.

## Goal

Create the server-side contract for persisted replay rooms to wait in a spectator-only lobby before
playback starts.

## Scope

- Change match-history replay launch so created replay rooms remain in a staging/lobby phase until
  the host sends `start`.
- Make replay staging rooms visible through `/api/lobbies` with safe metadata and a replay-specific
  room kind or equivalent explicit marker.
- Accept joins as spectators only; ignore or reject active seats, ready toggles, team changes, AI
  changes, faction changes, and map selection for replay staging rooms.
- Let the host start playback immediately when at least one spectator is present and the server is
  not blocking new sessions for drain.
- Preserve current `ReplayViewer` playback behavior after the host starts, including later spectator
  attach, room-time controls, vision selection, observer analysis, return-to-lobby detach, and
  branch-from-tick.
- Update `docs/design/protocol.md` and `docs/design/match-history.md` for the new launch and lobby
  behavior.

## Expected Touch Points

- `server/src/main.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/room_task/lobby.rs`
- `server/src/lobby/room_task/replay.rs`
- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js` only if the lobby wire shape changes
- `docs/design/protocol.md`
- `docs/design/match-history.md`
- Server lobby/replay tests under `server/src/lobby/**`

## Verification

- Focused Rust lobby/replay tests for replay staging summary, spectator-only joins, host start, and
  post-start playback.
- `node tests/protocol_parity.mjs` if any protocol vocabulary or message shape changes.

## Manual Testing Focus

Use a match-history replay launch endpoint, confirm the returned room appears in `/api/lobbies`, and
confirm two WebSocket clients can join as spectators before the host starts playback.

## Handoff Expectations

State the exact replay lobby marker added to HTTP or WebSocket payloads, what controls the server
ignores in replay staging, and which client files Phase 2 must update to consume the new contract.
