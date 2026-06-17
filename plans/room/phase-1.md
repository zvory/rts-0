# Phase 1 - Baseline Mode Matrix

## Phase Status

- [ ] Done.

## Objective

Capture current room behavior and lock down the highest-risk paths before production code starts
moving.

## Work

- Create a current-state matrix under `plans/room/` that describes every existing room-hosted path:
  normal lobby/live match, spectator, post-match replay, persisted replay room, replay branch
  staging, replay branch live match, dev self-play live watch, saved self-play replay, and dev
  scenario.
- For each path, record current choices for state source, join behavior, host/authority, command
  acceptance, clock, vision, mutation, persistence, start payload stamping, empty-room reset, and
  client-facing controls.
- Add or tighten focused tests for behavior that later phases will route through shared helpers:
  join routing, command rejection outside live play, branch seat aliasing, replay vision, dev pause
  and step, spectator projection, and match-history persistence decisions.
- Keep production movement to zero unless a harmless test helper is required.

## Expected Touch Points

- `plans/room/mode-matrix.md`
- `server/src/lobby/room_task.rs` tests
- `server/src/lobby/tests.rs`
- `server/src/lobby/replay_session.rs` tests if replay vision coverage needs tightening
- `server/src/lobby/snapshot_fanout.rs` tests if fanout coverage needs tightening

## Implementation Checklist

- [ ] Document the current mode matrix.
- [ ] Identify behavior that is protected by tests before extraction.
- [ ] Add focused characterization tests for unprotected high-risk paths where practical.
- [ ] Record behavior still requiring manual smoke testing.
- [ ] Confirm no protocol shape, gameplay rule, or visible client behavior changed.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `cargo test --manifest-path server/Cargo.toml -p rts-server match_history`
- `git diff --check`

## Manual Test Focus

Normal lobby start, spectator join before start, post-match replay prompt, persisted replay join,
replay branch staging and launch, dev self-play watch, and one dev scenario URL.

## Handoff Expectations

Name the tests that form the baseline for future phases, attach the mode matrix location, and call
out any behavior that remains manual-only before extraction.
