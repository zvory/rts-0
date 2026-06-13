# Phase 2 - Live Spectator Analysis Delivery

Status: Planned.

## Objective

Send the shared observer analysis payload to live spectator connections during normal in-game
ticks. Active players must not receive this payload in live matches.

## Scope

- Add a live in-game server delivery path beside the existing spectator snapshot fanout.
- Compute observer analysis once per tick only when at least one eligible live spectator exists.
- Send the payload only to connections whose room player state is `spectator: true`.
- Preserve replay playback, replay seek, replay vision, and replay branch behavior.
- Decide whether live delivery should be every tick or throttled; document the choice in
  `docs/design/protocol.md` if it affects semantics.
- Add focused server tests proving live spectators receive analysis and active players do not.
- Keep branch live rooms in mind: claimed branch seats are active-player views; unclaimed
  spectators are observer views.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/crates/sim/src/game/analysis.rs` if the public helper was renamed in Phase 1
- `docs/design/protocol.md`
- `tests/server_integration.mjs` if live end-to-end coverage is useful
- Focused room-task Rust tests near existing spectator and replay analysis tests

## Verification

Run focused server and protocol checks:

```bash
cd server && cargo test live_spectator
node tests/protocol_parity.mjs
```

If an end-to-end Node assertion is added, start the server on the test port and run:

```bash
node tests/server_integration.mjs
```

## Manual Testing Focus

Start a live match with two active players and one spectator. Confirm the spectator connection
receives observer analysis updates while active players do not receive the analysis message. Confirm
the spectator still receives the normal union-fog snapshot and that the match remains playable.

## Handoff Expectations

The handoff must describe the live delivery cadence, where the active-player exclusion is enforced,
and which tests prove the privacy boundary. It should tell Phase 3 whether the client can rely on
the same message shape for live spectators and replay viewers.

## Player-Facing Outcome

Live spectators have server-authored analysis data available, but the client overlay may still need
Phase 3 before it is visible in the UI.

