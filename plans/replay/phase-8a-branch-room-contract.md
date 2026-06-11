# Phase 8A - Branch Room Contract

## Objective

Define the server and protocol contract for creating a new practice branch room from an existing
replay viewer session, without promoting the replay to live play yet.

## Server Work

- Add an explicit branch-room creation path from `Phase::ReplayViewer`.
- Branch from the replay session's current authoritative server tick, not a client-supplied visual
  estimate.
- Clone or rebuild the replay `Game` to the branch tick and store it as frozen branch seed state.
- Create a new unguessable branch room id, separate from the source replay room.
- Move all connected replay viewers into the branch room.
- Keep the source replay artifact immutable. The branch must not mutate the artifact or source
  replay command log.
- Require the same replay compatibility validation used by replay playback before branch creation.
- Reject branch requests outside replay rooms.
- Reject branch creation from replays that include AI seats for the first implementation, or mark
  those seats unsupported with a clear error.

## Protocol Work

- Add a client message for requesting a branch from the current replay tick.
- Add a server message that announces branch-room creation and carries:
  - branch room id
  - source replay tick
  - original seats in replay order
  - whether each seat is claimable
- Decide whether the move to the branch room is expressed as a redirect-style message or an
  internal room transfer. Prefer an explicit message so the client lifecycle is easy to audit.
- Update Rust and JS protocol mirrors and `docs/design/protocol.md` together.

## Verification

- Unit test branch creation is rejected outside `Phase::ReplayViewer`.
- Unit test branch creation keeps the source replay artifact and replay session intact.
- Unit test current replay tick is captured exactly when the request is accepted.
- Unit test incompatible or unsupported replays fail without creating a branch room.
- Protocol mirror test for all new message tags and fields.

## Player-Facing Outcome

A replay viewer can request a branch and the server can create a separate branch room from the
current replay tick, but players cannot claim seats or start the branch yet.
