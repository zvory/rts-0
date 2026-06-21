# Phase 1 - Registry Disposal Primitive

Status: not started

## Goal

Add a small, race-safe way for a room task to tell the lobby registry that the room is empty and may
be removed. This phase should not change public lobby behavior yet; it only builds and tests the
registry cleanup primitive.

## Scope

- Add a lifecycle/disposal signal from `RoomTask` back to `Lobby`.
- Give each room handle a stable identity token, generation number, or equivalent channel match so
  cleanup removes only the exact room instance that requested disposal.
- Teach the registry to drop the matching `rooms` entry and thereby close the room task's event
  channel.
- Keep room creation, join, summary collection, drain, replay room creation, and branch room
  creation behavior otherwise unchanged.
- Add focused tests for successful removal, stale removal ignored after a newer room with the same
  name exists, and room task shutdown after registry removal.

## Touch Points

- `server/src/lobby/mod.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/tests.rs`
- Possibly `scripts/check-lobby-architecture.mjs` only if the new lifecycle helper needs an
  allowlist or module-boundary assertion.

## Out Of Scope

- Do not change `POST /api/lobbies` semantics in this phase.
- Do not change empty-room reset behavior in this phase.
- Do not add client-visible lobby browser changes in this phase.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lobby`
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture` if any
  room-task or lobby-module boundary changes affect architecture checks.
- `git diff --check`

## Manual Testing Focus

No broad manual testing is expected for this internal primitive. If a local server is already
running for verification, create and join a normal lobby once to confirm the ordinary join path still
works.

## Handoff

After this phase, report the lifecycle primitive name, the stale-cleanup guard used, the focused
tests added, and whether any module-boundary checker was updated. Tell the next agent to wire the
primitive into public normal lobby cleanup without adding a reclaim-on-duplicate path.
