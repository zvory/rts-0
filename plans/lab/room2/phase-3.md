# Phase 3 - Room-Controlled Time Contract

## Phase Status

- [x] Done.

## Objective

Replace replay/dev-watch-shaped time control with a neutral room-controlled-time contract. Backwards
compatibility is not required, so remove old replay/dev wire names in the same coordinated change
instead of keeping fallback aliases.

## Work

- Define the room time capability in `SessionPolicy`: fixed realtime ticking or room-controlled time,
  plus supported operations such as pause, speed, step, and seek for the current state source.
- Rename server protocol messages and DTOs away from replay/dev-specific time names. Candidate shapes:
  `setRoomTimeSpeed`, `stepRoomTime`, `seekRoomTimeTo`, and `roomTimeState`; choose final names during
  implementation and update every mirror in the same phase.
- Replace `setReplaySpeed`, `stepDevTick`, `seekReplay`, `seekReplayTo`, and `replayState` uses where
  they are generic room time behavior. Do not preserve old message builders or fallback handlers.
- Keep product-specific behavior intact: replay seek/keyframe behavior remains replay-only, dev
  scenario one-tick step remains dev-only, and lab timeline controls remain out of scope unless they
  already exist when this phase begins.
- Update `tick_control.rs` so scheduled actions and permission checks consume clock capability and
  allowed operations rather than replay/dev identity.
- Update `ReplaySession` and dev scenario room handling to produce and consume the new time state.
- Update browser controls and network wrappers to send the new messages and display the new state.
- Update `docs/design/protocol.md`, protocol parity tests, and client contract tests.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/tick_control.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/replay_session.rs`
- `client/src/protocol.js`
- `client/src/net.js`
- `client/src/replay_controls.js`
- `client/src/match.js`
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- focused protocol, replay, dev, and client contract tests
- `plans/lab/room2/phase-3.md`

## Implementation Checklist

- [x] Choose final neutral room-time protocol names and remove old replay/dev time names.
- [x] Mirror the protocol changes across Rust DTOs, server adapters, JavaScript builders/decoders,
      and docs.
- [x] Route server replay and dev scenario timing through clock capability and allowed operations.
- [x] Update client controls to use room-time state instead of replay/dev mode checks where possible.
- [x] Add or update focused tests for replay pause/speed/seek and dev pause/step.
- [x] Mark this phase as done in this file.

## Verification

- `node tests/protocol_parity.mjs`
- `node tests/client_contracts.mjs`
- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo test --manifest-path server/Cargo.toml -p rts-server tick`
- `git diff --check`

## Manual Test Focus

Open a replay and verify pause, speed, seek, timeline display, vision controls, and branch creation
still work. Open one `/dev/scenario` flow and verify pause, speed, and one-tick step still work.

## Handoff Expectations

Name the final room-time wire tags and client class/module names, list every old replay/dev time name
removed, and call out any remaining replay-specific references that are truly replay state-source
behavior.
