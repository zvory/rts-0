# Phase 4 - Lab Pause, Speed, And Step

## Phase Status

- [x] Done.

## Objective

Add shared lab pause, resume, speed, and one-tick step controls through the neutral room-time
capability system.

## Work

- Add a lab room-time source to the room policy model instead of treating labs as replay playback or
  fixed live matches.
- Advertise lab room-time capabilities for set-speed, pause, and step. Do not advertise seek or
  timeline yet.
- Store lab room-time state in room-owned lab/session state, including current speed, paused state,
  and last controller where useful for `roomTimeState`.
- Route scheduled lab ticks through the same authoritative live-game tick and snapshot fanout path
  used by current labs. If this requires refactoring `RoomTask::on_tick`, extract a neutral helper
  rather than duplicating live tick code.
- Implement `setRoomTimeSpeed` for labs using existing clamping and pause semantics. `0` pauses,
  positive values resume at the clamped speed, and any direct lab operator may control it.
- Implement `stepRoomTime` for paused labs only. One step advances exactly one authoritative lab tick
  and broadcasts fresh snapshots plus `roomTimeState`.
- Keep normal matches, replay playback, replay branch, and dev scenario time controls unchanged.

## Expected Touch Points

- `server/src/lobby/session_policy.rs`
- `server/src/lobby/tick_control.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/launch.rs`
- `server/crates/contract/src/lib.rs` only if comments/tests around room-time capabilities change
- `server/crates/protocol/src/lib.rs` only if protocol docs/tests change
- `client/src/room_capabilities.js`
- `client/src/replay_controls.js` or an extracted neutral room-time controls module
- `client/src/app.js` / `client/src/match.js` if room-time control ownership needs clearer injection
- `tests/client_contracts/match_replay_contracts.mjs`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server room_time`
- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs` if protocol or contract files change
- `node scripts/check-client-architecture.mjs` if client room-time modules are extracted or rewired
- `git diff --check`

If `room_time` or `lab` filters run zero tests, use the nearest explicit tests covering
`TickControl`, session policy capabilities, and lab tick behavior.

## Manual Test Focus

Open a lab with two browser sessions. Pause the lab from one browser, confirm both browsers stop
advancing, step one tick, resume at normal speed, then change speed and confirm both browsers observe
the shared time behavior.

## Handoff Expectations

State the new lab room-time capabilities, name the helper or path that routes lab ticks through live
simulation, and call out any UI rough edges that Phase 7 should clean up.
