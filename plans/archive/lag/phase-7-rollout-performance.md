# Phase 7 - Rollout, Performance Budgets, and Removal of Legacy Delay Paths

Status: Done. Prediction remains default-enabled for compatible live active-player sessions, but
now has an explicit server/client compatibility gate, richer per-session diagnostics, and a
main-thread replay budget fallback to tracking-only mode.

## Objective

Ship local prediction broadly once correctness and performance gates are stable, then simplify
older delay-compensation paths that are no longer needed.

## Rollout Work

- Keep prediction behind a runtime flag until all phase gates pass:
  - disabled
  - tracking only
  - movement prediction
  - command UI prediction
  - default enabled
- Add a server/client compatibility check so mismatched builds disable prediction automatically
  rather than running unsafe reconciliation.
- Add player/session metrics:
  - pending command count
  - acknowledged command latency
  - correction distance
  - correction frequency
  - prediction disable reasons
  - WASM tick time
  - WASM memory footprint
- Add a developer overlay or log export for prediction metrics.

## Performance Budgets

Set explicit budgets before enabling prediction by default:

- WASM startup time
- WASM binary size
- local prediction tick time at representative entity counts
- memory usage after 5, 15, and 30 minutes
- main-thread frame impact
- maximum reconciliation replay ticks per frame

If prediction runs on the main thread at first, add a hard budget and a fallback to tracking-only
mode when replay work exceeds that budget. Consider a Worker only after the single-thread path is
correct and measured.

## Cleanup Work

- Remove or reduce any artificial client-side command delay that existed only to hide network echo.
- Revisit interpolation delay constants after prediction is default.
- Keep snapshot interpolation for non-owned entities and authoritative corrections.
- Keep latest-only snapshot coalescing on the server; prediction must tolerate it.

## Verification

- Full test suite:
  - `cargo test`
  - protocol mirror tests
  - architecture checks
  - Node regression tests
  - server integration tests
  - client smoke tests
  - WASM parity tests
- Long-running browser soak:
  - 30 minute predicted match
  - repeated rematches to prove teardown releases WASM and listeners
  - memory budget assertion
- Performance harness with representative matches and explicit pass/fail thresholds.
- Rollback test proving prediction can be disabled mid-session and the client returns to
  authoritative snapshots without corrupting selection, camera, fog, or HUD state.

## Manual Testing Focus

Run a normal match and at least one artificial-latency match with prediction enabled by default.
Manual testing should focus on command feel, visible correction quality, frame pacing, and whether
turning the prediction flag off still provides a reliable fallback.

## Handoff Expectations

At handoff, include the final rollout flag state, performance measurements against the stated
budgets, and the list of legacy delay paths removed or intentionally retained. Name any remaining
prediction caveats that should be watched in playtests after rollout.

## Player-Facing Outcome

Prediction becomes the normal live-match experience. Owned commands respond immediately while
remote authority, fog correctness, and replay determinism remain intact.

## Implementation Notes

- Live active-player `start` payloads now include `predictionBuildId` and `predictionVersion`.
  Clients disable prediction automatically on version/build mismatch, while spectators and replays
  continue to omit prediction compatibility metadata.
- Client net reports now include pending command count, issue-to-sim-ack latency, correction
  distance/frequency, disable counts, WASM tick/replay work, and WASM memory footprint. The server
  logs notable prediction reports alongside existing network/render reports.
- The single-thread WASM path has a 4 ms replay-work budget. If measured replay work exceeds it,
  visual prediction falls back to tracking-only command sequencing without corrupting selection,
  fog, camera, HUD, or authoritative snapshots.
- `window.__rtsPredictionDebug` now exports compatibility, controller, and WASM diagnostics for
  developer inspection/log export.
- Legacy interpolation and snapshot coalescing paths were intentionally retained. This phase did
  not remove artificial command delay because no prediction-only command delay path was found in
  the current client flow.
