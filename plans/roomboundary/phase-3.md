# Phase 3 - Snapshot Fanout And Observer Delivery

## Phase Status

- [ ] Not implemented.

## Objective

Centralize room-local snapshot fanout while preserving each projection mode's visibility contract.

## Work

- Extract fanout plumbing for live players, live spectators, replay viewers, branch-live players,
  and dev watch clients.
- Centralize compact snapshot creation, `SnapshotNetStatus`, slow tick accounting, head-of-line
  accounting, and perf snapshot records.
- Keep projection mode selection explicit so normal gameplay never receives full-world data.
- Share observer analysis delivery logic for live spectators and replay viewers where safe.

## Expected Touch Points

- `server/src/lobby/room_task.rs`
- New `server/src/lobby/snapshot_fanout.rs` or `server/src/lobby/fanout.rs`
- `server/src/lobby/snapshots.rs`

## Implementation Checklist

- [ ] Inventory all current snapshot send paths and projection modes.
- [ ] Extract a fanout helper with explicit projection choices.
- [ ] Preserve perf and net-status metadata.
- [ ] Add or update focused tests for spectator/replay/branch/dev visibility.
- [ ] Run verification and record exact results in the handoff.

## Verification

- Focused `cargo test --manifest-path server/Cargo.toml -p rts-server` tests for spectator,
  replay, branch, and fanout paths
- `cargo run --manifest-path server/Cargo.toml -p rts-archcheck -- check-sim-architecture`
- `node tests/protocol_parity.mjs` if snapshot message construction changes

## Manual Test Focus

Human fog versus spectator union fog, replay single-player/all-player vision, branch-live mapped
seat snapshots, and dev self-play full-world watch.

## Handoff Expectations

Identify every caller mode and the exact projection path it now uses.
