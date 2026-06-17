# Phase 5 - Projection And Fanout Policy

## Phase Status

- [ ] Done.

## Objective

Extract snapshot projection and fanout choices into a shared policy while preserving all current
fog and visibility behavior.

## Work

- Add a lobby-owned projection policy, for example `server/src/lobby/projection.rs`, that can answer
  what snapshot projection each connected recipient should receive for live, spectator, replay,
  branch live, dev scenario paths, and any neutral saved-artifact replay inspection path preserved
  by Phase 0.
- Preserve normal player fog through `Game::snapshot_for_player`, live spectator union vision,
  branch live original-seat mapping, replay per-viewer vision, observer analysis delivery, dev
  full-world snapshots, compact snapshot fanout, and net-status/perf accounting.
- Route `live_tick.rs`, `snapshot_fanout.rs`, replay fanout, and dev fanout through the shared
  projection names where practical.
- Keep projection decisions server-authoritative; do not trust client-selected vision without the
  existing replay validation.
- Do not widen spectators into omniscient viewers except where current dev or replay behavior
  already does so.

## Expected Touch Points

- `server/src/lobby/projection.rs` or similarly named lobby-local module
- `server/src/lobby/snapshot_fanout.rs`
- `server/src/lobby/live_tick.rs`
- `server/src/lobby/replay_session.rs`
- `server/src/lobby/room_task.rs`
- `server/src/lobby/tests.rs`

## Implementation Checklist

- [ ] Add focused tests for projection classification before moving fanout code.
- [ ] Preserve per-recipient player snapshots for active players.
- [ ] Preserve live spectator visible-player list semantics, including branch live aliases.
- [ ] Preserve replay per-viewer vision validation and clamping.
- [ ] Preserve dev full-world snapshot behavior and observer analysis delivery.
- [ ] Run verification and record exact results in the handoff.

## Verification

- `cargo test --manifest-path server/Cargo.toml -p rts-server replay_vision`
- `cargo test --manifest-path server/Cargo.toml -p rts-server spectator`
- `cargo test --manifest-path server/Cargo.toml -p rts-server branch`
- `cargo test --manifest-path server/Cargo.toml -p rts-server dev`
- `node tests/protocol_parity.mjs` if any protocol-facing snapshot assumptions are touched
- `git diff --check`

## Manual Test Focus

Player fog in a normal match, spectator view in a live match, replay vision switching, branch live
spectator/player views, saved artifact replay inspection if it exists, dev scenario full-world
view, and observer-analysis overlays.

## Handoff Expectations

Name the projection policy API, list every fanout path migrated to it, and explicitly state whether
any fog-sensitive behavior remains manual-only.
