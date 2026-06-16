# Phase 1 - Baseline Characterization

## Phase Status

- [ ] Not implemented.

## Objective

Capture current room-task responsibilities and protect high-risk lifecycle behavior before moving
production code.

## Work

- Inventory `room_task.rs` responsibilities and identify replay, branch, fanout, AI, observer, and
  lifecycle coverage gaps.
- Add or tighten focused tests for replay viewer joins, replay seek state, branch staging, live
  observer analysis delivery, and tick/fanout equivalence where practical.
- Update docs only if they are stale about the current `Game` seam or room ownership model.

## Expected Touch Points

- `server/src/lobby/room_task.rs` tests
- `docs/design/server-sim.md` only if stale
- `plans/roomboundary/*`

## Implementation Checklist

- [ ] Inventory current responsibilities and coverage.
- [ ] Add focused baseline tests for replay paths.
- [ ] Add focused baseline tests for branch and observer paths where practical.
- [ ] Record behavior still requiring manual smoke testing.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `node tests/protocol_parity.mjs` if protocol docs or message assumptions are touched
- `git diff --check`

## Manual Test Focus

Normal lobby start, spectator start, post-match replay prompt, persisted replay join, and replay
seek controls.

## Handoff Expectations

List the baseline tests future phases must keep green and name any room behavior that remains only
manually covered.
