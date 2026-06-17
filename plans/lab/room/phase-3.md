# Phase 3 - Participants And Authority

## Phase Status

- [x] Done.

## Objective

Extract connected-player, seat-mapping, host, spectator, and command-authority logic into a
lobby-owned helper that preserves existing player ids and command behavior.

## Work

- Add a participant helper module, for example `server/src/lobby/participants.rs`, that owns or
  wraps the current `order`, `players`, host fallback, active human ids, active seat ids,
  spectator checks, and branch live seat aliases.
- Route command issuer resolution through the helper so normal live matches still issue as the
  connection's player id, replay branch live matches still issue as the original replay seat id,
  spectators stay read-only, defeated players stay blocked, replay/dev paths keep their current
  rejection behavior, and client sequence tracking remains per connection.
- Keep AI slots in `RoomTask` unless moving them is necessary and clearly behavior-neutral.
- Preserve join order, color assignment inputs, host reassignment, lobby broadcasts, ready state,
  branch staging occupant behavior, and empty-room reset.
- Do not move transport sinks out of room ownership or add cross-room shared state.

## Expected Touch Points

- `server/src/lobby/participants.rs` or similarly named lobby-local module
- `server/src/lobby/room_task.rs`
- `server/src/lobby/replay_branch.rs` only if branch aliasing needs a small adapter
- `server/src/lobby/tests.rs`

## Implementation Checklist

- [x] Extract participant read helpers before moving mutation helpers.
- [x] Route live seat lookup and player/spectator command authority through the helper.
- [x] Preserve per-connection command sequence acknowledgement behavior.
- [x] Add tests for normal player command acceptance, spectator command rejection, branch live
      seat command aliasing, defeated-player blocking, and replay command rejection.
- [x] Confirm lobby display order, host fallback, and branch staging broadcasts remain unchanged.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server command`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay_phase_ignores_gameplay_commands`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `git diff --check`

## Manual Test Focus

Normal player command issue, spectator read-only behavior, branch room seat claim and launch,
branch live command control, host leaving a lobby, and active player leaving mid-match.

## Handoff Expectations

Name the new participant helper API, remaining direct `players` or `order` accesses in
`RoomTask`, and any authority decisions still expressed as ad hoc mode checks.
