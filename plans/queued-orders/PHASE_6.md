# Phase 6 - Hardening, Replay, and Documentation Audit

Goal: close edge cases after all queued order types are implemented.

## Scope

- Determinism:
  - Verify command logs reproduce queued order behavior.
  - Avoid hash-order-dependent queue promotion.
  - Keep entity-id ordering deterministic where promotion scans multiple entities.
- Hardening:
  - Fuzz or regression-test oversized queued commands, non-finite coordinates, repeated ids, stale
    ids, dead targets, depleted resources, and destroyed construction sites.
  - Confirm queue caps bound memory and per-tick promotion work.
- Documentation:
  - Update `docs/design/protocol.md` wire protocol tables and
    `docs/design/server-sim.md` `Game`/order contracts.
  - Document completion semantics for every queued order kind.
  - Update client interaction notes for Shift-click behavior and queued markers.
- UX polish:
  - Check marker readability for dense selections.
  - Ensure rally markers and queued unit markers do not visually conflict.
  - Confirm Esc, Stop, selection changes, rematch teardown, and replay viewing clean up marker state.

## Tests

- Server simulation tests for every order completion path.
- Regression tests for invalid queued commands and caps.
- Replay tests covering queued moves, worker build/gather handoff, mixed attacks, and rallies.
- Client smoke coverage for Shift-click marker rendering where practical.

## Done

- Queued orders are documented as a stable gameplay contract.
- Replays remain deterministic.
- Invalid queued input cannot panic `Game::tick()` or grow unbounded state.
- Player-facing behavior is covered in patch notes for the implementation PRs or commits.
