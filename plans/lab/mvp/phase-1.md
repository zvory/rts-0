# Phase 1 - Lab Room Skeleton

## Phase Status

- [ ] Not started.

## Objective

Create a minimal lab room mode that launches a real `Game`, sends a normal `start` payload with lab
metadata, and renders through the existing match screen with server-owned full-world projection.

## Work

- Add `RoomMode::Lab(LabRoomConfig)` and lab classification to `session_policy.rs` using explicit
  lab state source, authority, vision, mutation, persistence, and start-payload choices.
- Add `LabSession` room-owned state for the operator id, viewer roles, dirty flag, operation log
  placeholder, and default vision mode. Keep it in lobby/room ownership, not in `Game`.
- Add a bounded lab route and join target, for example `/lab` with safe query parameters that map
  to an internal lab room id. Hide lab rooms from any normal lobby-browser or public normal-room
  listing.
- Launch a real `Game` from a selected map and a default two-team player template. Use public
  `Game` constructors only; do not reach into sim internals.
- Add `StartPayload.lab` metadata in the shared contract and mirror it through Rust protocol,
  server protocol, JavaScript protocol, and `docs/design/protocol.md`.
- Send lab start payloads through the shared launch helper. Operator prediction should be disabled
  until issue-as command semantics are explicitly wired.
- Route lab snapshots through the shared projection helper with an explicit full-world lab
  projection. Do not broaden normal spectators or replay viewers to full vision.
- Add focused server tests proving lab room join/launch, start payload stamping, full-world lab
  projection, empty-room reset, and non-lab behavior preservation.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/session_policy.rs`
- `server/src/lobby/projection.rs`
- `server/src/lobby/launch.rs`
- `server/src/lobby/mod.rs`
- `server/src/main.rs`
- `server/crates/contract/src/lib.rs`
- `server/crates/protocol/src/lib.rs`
- `server/src/protocol.rs`
- `client/src/protocol.js`
- `client/src/bootstrap.js`
- `client/src/app.js`
- `docs/design/protocol.md`
- `docs/design/server-sim.md`
- `docs/context/server-sim.md`
- `tests/protocol_parity.mjs`

## Implementation Checklist

- [ ] Add lab room config and session state without adding privileged mutations.
- [ ] Add lab session policy values and tests for policy classification.
- [ ] Add lab route/join parsing with bounded names and map selection.
- [ ] Start a real `Game` with a default two-team template and selected map.
- [ ] Add mirrored `StartPayload.lab` metadata and update protocol docs.
- [ ] Send lab viewers through shared launch and projection helpers.
- [ ] Confirm lab rooms do not write match history or expose themselves as normal rooms.
- [ ] Keep `/dev/scenario`, replay, replay branch, and normal lobby behavior unchanged.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `cargo test --manifest-path server/Cargo.toml -p rts-server session_policy`
- `cargo test --manifest-path server/Cargo.toml -p rts-server projection`
- `node tests/protocol_parity.mjs`
- `node scripts/check-lobby-architecture.mjs`
- `git diff --check`

If `lab` matches zero tests, add narrowly named lab tests before counting it as verification.

## Manual Test Focus

Open `/lab`, confirm it joins a lab room, renders a real map through the normal match view, shows
all expected units/resources, and leaves/reset cleanly when the browser disconnects. Also smoke a
normal lobby start and one replay/dev scenario URL to check the new mode did not steal existing
routes.

## Handoff Expectations

Name the lab route and room naming scheme, the `StartPayload.lab` shape, and every room primitive
used by the skeleton. State clearly that privileged lab operations are still absent and that Phase 2
should add the public `Game` lab API before room/client mutation wiring.
