# Phase 3 - Lab Protocol and Room Operations

## Phase Status

- [ ] Not started.

## Objective

Wire typed lab operations through mirrored protocol messages and the room task, with explicit
operator authorization, result reporting, operation logging, issue-as commands, and per-viewer
vision policy.

## Work

- Add a single top-level client lab envelope with bounded `requestId` and typed lab op payloads.
- Add `labState` and `labResult` server messages. `labState` is room/control metadata; normal
  world state still travels through `snapshot`.
- Mirror all protocol changes across Rust protocol, server protocol, JavaScript protocol builders,
  protocol parity checks, and `docs/design/protocol.md`.
- Add `RoomEvent::Lab` or an equivalent request/reply path that routes lab messages only to lab
  rooms and ignores or rejects them elsewhere.
- Enforce one operator per lab room for the MVP. Non-operator viewers may receive snapshots and lab
  state but cannot mutate or issue commands.
- Store accepted privileged operations in an append-only room-local op log with tick, request id,
  operator id, op kind, and result metadata.
- Wire `Game::apply_lab_op` for privileged setup operations.
- Wire `Game::issue_lab_command_as` for real gameplay commands as one owning player. Reject
  mixed-owner gameplay orders unless a later client flow partitions them intentionally.
- Implement lab vision operations as room policy: all, team, and selected-team union. Translate
  team choices to current player ids server-side.
- Add room tests for authorization, result delivery, rejected operations, normal-room isolation,
  viewer read-only behavior, issue-as routing, and vision projection.

## Expected Touch Points

- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `server/src/main.rs`
- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/participants.rs`
- `server/src/lobby/tests.rs`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `tests/protocol_parity.mjs`

## Implementation Checklist

- [ ] Add mirrored lab envelope, op, state, result, error, and vision DTOs.
- [ ] Add JavaScript protocol builders for every lab message.
- [ ] Add room event handling with payload bounds and request id validation.
- [ ] Authorize only the lab operator for mutations and issue-as commands.
- [ ] Broadcast or target `labState` changes predictably after accepted operations.
- [ ] Return `labResult` for accepted and rejected requests.
- [ ] Log accepted privileged operations in room-local order.
- [ ] Route lab snapshots through explicit lab vision policy.
- [ ] Confirm lab operations are ignored or rejected outside lab rooms.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server command`
- `cargo test --manifest-path server/Cargo.toml -p rts-server projection`
- `node tests/protocol_parity.mjs`
- `node scripts/check-lobby-architecture.mjs`
- `git diff --check`

## Manual Test Focus

Use a temporary or minimal client/dev harness if available to send lab ops against a lab room:
switch vision, attempt a rejected non-operator mutation, issue a command as one owner, and confirm a
normal room ignores lab messages.

## Handoff Expectations

Document the final lab wire shapes, the lab authorization model, the op-log record shape, and the
vision behavior. State which client-facing controls still need to be built in Phase 4 and Phase 5.
