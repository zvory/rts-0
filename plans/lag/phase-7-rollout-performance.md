# Phase 7 - Rollout, Performance Budgets, and Removal of Legacy Delay Paths

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

## Player-Facing Outcome

Prediction becomes the normal live-match experience. Owned commands respond immediately while
remote authority, fog correctness, and replay determinism remain intact.
