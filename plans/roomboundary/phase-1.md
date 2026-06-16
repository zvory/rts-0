# Phase 1 - Baseline Characterization

## Phase Status

- [x] Done.

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

- [x] Inventory current responsibilities and coverage.
- [x] Add focused baseline tests for replay paths.
- [x] Add focused baseline tests for branch and observer paths where practical.
- [x] Record behavior still requiring manual smoke testing.
- [x] Run verification and record exact results in the handoff.

## Baseline Inventory

Current `server/src/lobby/room_task.rs` responsibilities:

- Room event loop ownership: joins, leaves, lobby readiness, countdowns, role/team/faction changes,
  room drain notices, and empty-room reset.
- Live match runtime: match creation, AI command enqueue, simulation ticks, panic replay capture,
  per-recipient snapshot fanout, observer-analysis delivery to live spectators, defeat/game-over
  handling, post-match replay transition, and match-history dispatch.
- Replay runtime: artifact validation, replay `Game` rebuild, command cursor, keyframes,
  seek/rate-limit state, per-viewer replay vision, replay snapshot fanout, playback state, and
  observer-analysis delivery.
- Replay branch runtime: branch seed creation, staging-room join/host/seat state, seat claim and
  release policy, branch launch payloads, original-seat command/snapshot mapping, branch game-over
  handling, and branch-room empty reset.
- Dev watch/scenario runtime: saved self-play replay loading, live self-play ticking, scenario
  stepping, full-world dev snapshots, and dev pause/speed state.

Focused room-task coverage now includes:

- Replay artifact validation, replay keyframe rebuild/seek behavior, per-viewer replay fog, replay
  speed/seek clamping, replay command rejection, replay join prompt/confirmed join, initial replay
  observer-analysis delivery, and first replay snapshot fanout after confirmed join.
- Post-match replay transition to tick 0, replay viewer return behavior, persisted replay
  multi-viewer return behavior, and saved self-play replay reuse of the shared replay viewer.
- Replay branch request rejection/success, branch seed preservation, source replay integrity, branch
  announcement broadcast, direct branch-room join staging initialization, seat claim/release/host
  behavior, branch launch payloads, original-seat command/snapshot mapping, branch faction rejection,
  branch give-up resolution, and empty branch-room reset.
- Live spectator observer-analysis delivery while active players do not receive observer-analysis
  messages.

Behavior still requiring manual smoke testing:

- Normal lobby match start from the browser, including active player and spectator start payloads.
- Post-match replay prompt/confirm flow in the browser and replay seek controls.
- Persisted replay room join from a replay link, including replay vision selection UI.
- Replay branch creation from a replay viewer, branch-room seat claiming, branch launch, and branch
  live play from the browser.

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
