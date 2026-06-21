# Phase 1 - Collaborative Lab Authority

## Phase Status

- [ ] Not started.

## Objective

Allow multiple connections in the same lab room to operate as collaborators. The server remains the
only authority: every accepted operation still flows through `RoomTask`, `LabClientOp`, and public
`Game` lab APIs.

## Work

- Replace the single-operator authorization check with role-based lab authorization.
- Decide the first implementation policy: every direct `/lab` joiner becomes
  `LabStartRole::Operator`; keep `ReadOnly` available in the protocol for future explicit viewer
  modes if it remains useful.
- Preserve the original room creator or primary operator only as metadata if needed; do not use it
  as the sole mutation authority.
- Ensure `LabSession::role_for` and join handling make reconnect/late-join behavior deterministic.
- Record the actual requesting connection id in every accepted lab operation log entry, including
  issue-as commands and ordinary lab mutations.
- Keep import/export, vision changes, spawn/delete/move/set-owner/resource/research operations, and
  issue-as command routing room-local and bounded.
- Update room-task tests that currently assert a later lab joiner is read-only.
- Add a test proving two lab connections can both mutate or issue commands in the same room, with
  operation log entries attributed to the correct requester.
- Keep normal rooms rejecting lab requests and keep non-lab room behavior unchanged.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs` if role language or policy helpers need clarification
- `server/crates/contract/src/lib.rs` only if lab role/start metadata shape changes
- `server/crates/protocol/src/lib.rs` only if lab role/start metadata shape changes
- `server/src/protocol.rs` only if protocol adapters change
- `client/src/protocol.js` only if protocol mirrors change
- `docs/design/protocol.md`
- `docs/design/server-sim.md` if room ownership semantics change
- `tests/protocol_parity.mjs` if protocol mirrors change

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server quickstart`
- `node tests/protocol_parity.mjs` if protocol or contract shapes change
- `git diff --check`

If the Rust filter runs zero tests, choose the nearest explicit room-task test filter that covers
the changed lab authorization path and report it in the handoff.

## Manual Test Focus

No browser manual test is required for this server-authority phase unless the implementation changes
the start payload shape. If a manual check is cheap, open two `/lab?room=<same>` sessions and
confirm both receive lab starts.

## Handoff Expectations

State the exact collaborator role policy, whether `operatorId` semantics changed, which operations
were proven from a second connection, and whether the next phase needs any client protocol updates.
