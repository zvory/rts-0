# Phase 1 - Live Match Pause

## Phase Status

- [ ] Not started.

## Objective

Add live-match pause and unpause controls. Each active player may initiate pause at most three times
per match, and any active player may unpause the current pause. The paused state must be
server-authoritative and visible to every live match recipient through a centered Game Paused overlay.

## Work

- Add live pause protocol messages:
  - client-to-server `pauseGame`;
  - client-to-server `unpauseGame`;
  - server-to-client reliable `livePauseState`.
- Define a compact `LivePauseState` contract, with fields such as:
  - `paused: bool`;
  - `pausedBy?: u32`;
  - `pausesRemaining?: u8` for the receiving active player;
  - optional `pauseLimit: u8` if the UI needs to present the fixed match limit;
  - enough state for spectators to render the overlay without granting them controls.
- Extend room capability metadata without conflating live pause with room-time controls:
  - prefer a new nested capability such as `matchControls.pause`;
  - alternatively extend `commands` with live-pause-specific booleans if that fits the current
    `RoomCapabilities` shape better;
  - update `SessionPolicy::start_capabilities` so normal live players advertise pause capability and
    spectators/replays/dev/lab do not;
  - document any deliberate branch-live behavior.
- Route messages through the existing WebSocket path:
  - add Rust `ClientMessage` variants and protocol contract tags;
  - add JS `C` tags and `msg.pauseGame()` / `msg.unpauseGame()` builders;
  - add `Net.pauseGame()` and `Net.unpauseGame()` helpers;
  - add `RoomEvent` variants and `main.rs` forwarding.
- Add room-owned live pause state to `RoomTask`:
  - `live_paused`;
  - `live_paused_by`;
  - per-seat or per-connection pause counters with a fixed limit of three successful pauses;
  - reset fields on `prepare_live_match_launch`, live match completion, replay transition, and empty
    room reset as appropriate.
- Implement authority and state transitions:
  - accept `pauseGame` only from active live players with pauses remaining and while not already
    paused;
  - decrement the initiating player's remaining count only when a pause request changes the room
    from unpaused to paused;
  - accept `unpauseGame` from any active live player while paused;
  - ignore or reliably reject invalid requests without panicking;
  - broadcast `livePauseState` to all connected match recipients whenever state changes and send the
    current state after match start.
- Stop live simulation work while paused:
  - in `RoomTask::on_tick`, before constructing `LiveTickDriver`, skip the live simulation branch if
    `live_paused` is true;
  - do not call AI thinking, `Game::tick`, command-ack consumption, defeat checks, or live snapshot
    fanout as part of a paused scheduled tick;
  - keep the room event loop alive so pings, reports, disconnects, Give up, and Unpause still work.
- Add client UI:
  - parse the new capability in `client/src/room_capabilities.js`;
  - add a gear-menu Pause action beside Give up, visible to active live players while unpaused and
    enabled only when the local player has remaining pauses;
  - do not show a confirmation before sending `pauseGame`;
  - add a `LivePauseOverlay` or similarly small match-owned DOM helper mounted under
    `#game-screen`;
  - render centered `Game Paused` text and an Unpause button for active live players;
  - allow spectators to see the paused overlay without an enabled unpause control unless the server
    explicitly says they are active;
  - update `Match.destroy()` teardown and settings remount behavior.
- Update docs:
  - `docs/design/protocol.md` for new messages, capability metadata, and `livePauseState`;
  - `docs/design/server-sim.md` for `RoomTask` live pause ownership and tick skipping;
  - `docs/design/client-ui.md` for the settings action and overlay module.

## Expected Touch Points

- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/main.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/room_task.rs`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/room_capabilities.js`
- `client/src/settings_panels.js`
- `client/src/match.js`
- possible new `client/src/live_pause_overlay.js`
- `client/styles.css`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `tests/client_contracts.mjs`
- `tests/protocol_parity.mjs`
- focused Rust tests near `server/src/lobby/session_policy.rs` and `server/src/lobby/room_task.rs`

## Implementation Checklist

- [ ] Add protocol DTOs, tags, builders, and parity coverage.
- [ ] Add room capability metadata for live pause controls.
- [ ] Add server-owned live pause state, counters, authorization, reset paths, and reliable fanout.
- [ ] Skip live simulation ticks while paused without blocking the room event loop.
- [ ] Add gear-menu Pause action with no confirmation.
- [ ] Add centered Game Paused overlay with active-player Unpause button.
- [ ] Update protocol, server-sim, and client-ui docs.
- [ ] Add focused Rust and JS tests.
- [ ] Run verification and record exact results.
- [ ] Mark this phase as done in this file.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs`
- focused Rust tests for `session_policy` pause capabilities
- focused Rust tests for `RoomTask` pause authorization, three-pause limit, any-player unpause, and
  no tick advance while paused
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If the focused Rust filters match zero tests, run the concrete test names added in this phase before
counting verification as passed.

## Manual Test Focus

Run a local two-player or player-vs-AI match. Confirm an active player can pause from the gear menu
without a confirmation, all connected recipients see the centered Game Paused overlay, another active
player can unpause from the overlay, and the initiating player's available pauses decreases only on
successful pause starts. Confirm the fourth pause attempt from the same player is unavailable or
ignored, Give up still works while paused, and normal simulation resumes after unpause.

## Handoff Expectations

Report the final protocol field names, capability shape, and exact pause-state reset points. State
whether branch-live matches were included or deliberately excluded. Include the focused verification
commands, any manual-test result, and any remaining risks around disconnects, spectators, or paused
command receipts.
