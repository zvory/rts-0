# Phase 6 - Launch Plans And Start Payloads

## Phase Status

- [x] Done.

## Objective

Extract common launch bookkeeping and per-recipient start payload composition from existing live
session starts.

## Work

- Add a launch helper, for example `server/src/lobby/launch.rs`, that packages shared start
  bookkeeping without owning the `Game` long term.
- Keep normal `start_match`, replay branch `start_branch_live`, and dev scenario
  `start_dev_session` rules intact, including map loading, seed selection, quickstart resources,
  AI controller setup, branch seat mapping, dev scenario drivers, match-history metadata, drain
  tracking, structured logs, and prediction flags.
- Extract only the common pieces: recipient list construction, `StartPayload` stamping, prediction
  build/version choice, spectator flag choice, pending snapshot clearing, match metadata recording
  inputs, and common send loops.
- Do not add a `payload.lab` field or any new start payload variant in this plan.
- Keep failure handling and host error messages identical for map and scenario bootstrap failures.

## Expected Touch Points

- `server/src/lobby/launch.rs` or similarly named lobby-local module
- `server/src/lobby/room_task.rs`
- `server/src/lobby/live_tick.rs` only if launch metadata currently feeds tick logging
- `server/src/lobby/tests.rs`
- `server/crates/protocol/src/lib.rs` should not change
- `client/src/protocol.js` should not change

## Implementation Checklist

- [ ] Add tests around normal start payload stamping for active players and spectators.
- [ ] Add tests around branch live start payload stamping and original-seat ids.
- [ ] Add tests around dev start payload stamping if current coverage is thin.
- [ ] Extract shared recipient/payload send logic without changing `StartPayload` shape.
- [ ] Confirm prediction build/version flags are unchanged for active players, spectators, branch
      seats, and replay/dev viewers.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server start`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch_launch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/protocol_parity.mjs`
- `git diff --check`

## Manual Test Focus

Normal active player start, normal spectator start, branch live launch from staging, dev scenario
start, and client prediction being enabled only for controllable active seats.

## Handoff Expectations

Describe the launch helper, name all start paths that use it, and explicitly state that
`StartPayload` and client protocol mirrors were unchanged unless a separately approved change was
made.
