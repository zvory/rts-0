# Phase 5 - Replay Branch Module

## Phase Status

- [x] Done.

## Objective

Isolate replay branch staging and launch policy in a lobby-local module.

## Work

- Move branch staging state, seat claim/release policy, branch staging message construction, and
  branch launch preparation out of `room_task.rs`.
- Keep membership, host identity, connection sends, countdown policy, and final phase transitions in
  `RoomTask`.
- Preserve original replay player mapping for branch-live commands, snapshots, and outcomes.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- New `server/src/lobby/replay_branch.rs`
- `server/src/lobby/mod.rs`
- Branch-related tests

## Implementation Checklist

- [x] Extract branch staging state and policy.
- [x] Preserve room-owned membership and send decisions.
- [x] Add or update mapped-seat branch tests.
- [x] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `node tests/protocol_parity.mjs` if branch message construction changes
- `git diff --check`

## Manual Test Focus

Create a branch from replay, claim all seats, launch the branch, verify spectators stay spectators,
and verify active players control original seats.

## Handoff Expectations

Document what remains in `RoomTask` because it is transport or room membership policy.
