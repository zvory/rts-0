# Phase 3 - Per-Operator Lab Vision

## Phase Status

- [x] Done.

## Objective

Make lab vision per operator while keeping the current all-operators collaborator model.

## Work

- Replace `LabSession`'s single shared `vision_mode` with recipient-specific vision state plus a
  default vision for future joiners.
- Keep `LabStartRole::Operator` for every direct lab joiner. Do not add permissions, read-only UI,
  locks, auth, invitations, or cursor/presence systems.
- Change `LabClientOp::SetVision` handling so it updates only the requesting operator's vision after
  validating the selected teams against the current `Game`.
- Make `LabStartMetadata` and `LabState` continue to carry `vision`, but ensure the value is the
  recipient's own vision. Broadcasts may still go to every operator, but each payload must be stamped
  for that recipient.
- Change lab snapshot fanout so each recipient's `LabVisionMode` drives that recipient's projection.
  Full-world projection, one-team fog, and team-union fog must remain server-side and authoritative.
- Define import/export semantics without widening legacy setup JSON: exporting uses the requesting
  operator's current vision, and importing applies scenario vision to the requester plus the default
  vision for future joiners without overwriting other connected operators.
- Preserve scenario dirty state, operation counts, result routing, and operation log attribution.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/projection.rs` if a helper is needed for per-recipient lab projection
- `server/src/lobby/snapshot_fanout.rs` if payload stamping needs to carry recipient-specific
  projection metadata
- `server/crates/contract/src/lib.rs` only if comments or metadata docs need clarification
- `server/crates/protocol/src/lib.rs` only if protocol comments/tests need clarification
- `client/src/lab_client.js`
- `client/src/lab_panel.js`
- `tests/client_contracts/lab_contracts.mjs`
- `tests/protocol_parity.mjs` if protocol fixtures or docs change
- `docs/design/protocol.md`
- `docs/design/client-ui.md`
- `docs/design/server-sim.md`

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server lab`
- `node tests/client_contracts.mjs`
- `node tests/protocol_parity.mjs` if protocol or contract files change
- `node scripts/check-client-architecture.mjs` if client module wiring changes
- `git diff --check`

If the Rust `lab` filter runs zero tests, choose the nearest explicit room-task test filter that
covers per-recipient lab state and projection.

## Manual Test Focus

Open the same lab room in two browser sessions. Set one operator to full-world vision and the other
to one team's vision, then confirm each browser keeps its own projection while both can still spawn,
edit, and issue commands.

## Handoff Expectations

State the final import/export vision semantics, whether any protocol shape changed, and whether the
next phase can safely add shared room-time controls without revisiting lab vision state.
