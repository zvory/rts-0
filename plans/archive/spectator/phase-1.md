# Phase 1 - Late Spectator Admission

## Phase Status

- [x] Done.

## Objective

Allow a user to join a normal in-progress match from the lobby browser as a read-only spectator.
The implementation should make the browser action available, accept only spectator late joins on the
server, send the late spectator a live `start` payload, and then let the existing live snapshot
fanout deliver union-fog spectator snapshots.

## Work

- Update the room join policy for normal live matches:
  - replace the blanket normal mid-match rejection with spectator-only admission;
  - keep non-spectator joins rejected with a clear error;
  - keep match-countdown joins rejected so a row cannot be joined while start is already committed;
  - keep duplicate joins rejected;
  - keep replay, replay-branch, lab, and dev-watch join paths governed by their existing policies.
- Add or reuse a helper that inserts a connected human as a spectator:
  - color `#6f8fa8`;
  - `ready: true` or other state that cannot block the live match;
  - no human team assignment;
  - no human faction assignment;
  - no command issuer, pause authority, or active seat mapping.
- Send the accepted late spectator a live start payload:
  - build from the current `Game::start_payload()` so the payload tick matches current live state;
  - set `payload_player_id` to the joining connection id;
  - set `spectator: true`;
  - disable prediction build/version;
  - use live spectator room capabilities and diagnostic capabilities;
  - clear any pending stale snapshot if the connection sink needs it.
- Ensure subsequent live snapshots include the late spectator:
  - rely on `Participants::spectator_visible_player_ids` and `ProjectionPolicy::live_snapshot_for`
    for union-fog spectator projection;
  - verify `SnapshotFanout` produces spectator net status without prediction ACK metadata;
  - keep observer analysis available to the late spectator when live spectator diagnostics advertise
    it.
- Update the lobby browser:
  - make `joinState: "inGame"` map to a spectator join intent;
  - show an action label such as `Spectate` or `Join as spectator` while preserving the status label
    `In match`;
  - keep `starting` and stale/unknown rows disabled;
  - keep the existing HTTP preflight, and make stale preflight failures refresh the list rather than
    wedging the UI.
- Update docs:
  - `docs/design/protocol.md` for `join` spectator semantics after live match start;
  - `docs/design/server-sim.md` for normal live spectator attach behavior;
  - `docs/design/client-ui.md` for lobby-browser in-progress row behavior if needed.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/participants.rs` if any helper shape needs adjustment
- `server/src/lobby/launch.rs` if start-recipient stamping needs a small reuse helper
- `client/src/lobby_browser_view.js`
- `client/src/lobby.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/design/client-ui.md`
- `tests/client_contracts.mjs`
- `tests/lobby_browser_integration.mjs`
- `tests/regression.mjs`
- focused Rust tests in `server/src/lobby/room_task.rs` and possibly `session_policy.rs`

## Implementation Checklist

- [x] Browser in-game rows are joinable only as spectators.
- [x] Active late joins remain rejected and leave the socket able to join another room.
- [x] Late spectator joins are accepted for normal live matches.
- [x] Late spectators receive a read-only live start payload.
- [x] Late spectators receive union-fog snapshots and observer analysis when advertised.
- [x] Countdown rows remain non-joinable.
- [x] Protocol, server-sim, and client-ui docs are updated.
- [x] Focused Rust and JS coverage is added or updated.
- [x] Verification is run and recorded.
- [x] This phase file is marked done in the implementation commit.

## Verification

- Executor verification:
  - `cargo test --manifest-path server/Cargo.toml -p rts-server late_spectator -- --nocapture`
  - `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy -- --nocapture`
  - `node tests/client_contracts.mjs`
  - `node --check tests/lobby_browser_integration.mjs`
  - `node scripts/check-client-architecture.mjs`
  - `node scripts/check-docs-health.mjs`
  - `git diff --check`
  - Local live `node tests/lobby_browser_integration.mjs` was not run because this executor sandbox
    rejected binding a private server on `127.0.0.1:18081` with `Operation not permitted`.
- `cargo test --manifest-path server/Cargo.toml -p rts-server late_spectator -- --nocapture`
- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy -- --nocapture` if
  session-policy join naming changes
- `node tests/client_contracts.mjs`
- `node tests/lobby_browser_integration.mjs` with a running server
- `node tests/regression.mjs` with a running server if the mid-match rejection regression is updated
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

If a focused Rust filter matches zero tests, run the exact test names added in this phase before
counting verification as passed.

## Manual Test Focus

Run a local player-vs-AI or two-player match, then open a second browser session at the lobby
browser. Confirm the running room is visible, the row action joins as a spectator, the spectator
screen loads without prediction or command UI, and the spectator sees both sides through union fog.
Confirm a stale click during countdown or a non-spectator direct join is rejected cleanly and the
socket can still join another room.

## Handoff Expectations

Report the final browser action label, exact late-join server helper shape, and whether any
`SessionPolicy` join enum name changed. Include the focused verification commands and note any
manual-test results. Call out any remaining gaps for Phase 2, especially notice routing, paused-match
behavior, and whether the newly joined spectator is excluded from the join notice.
