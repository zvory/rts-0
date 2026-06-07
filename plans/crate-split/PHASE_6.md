# Phase 6 - Test Selection, CI Policy, and Documentation Lock-In

Status: Planned.

Goal: turn the crate split into enforceable development policy and reliable test-selection rules.

## Scope

- Update `docs/design/architecture.md`, `docs/design/server-sim.md`, `docs/design/protocol.md`,
  `docs/design/ai.md`, and context capsules to describe the new crate boundaries.
- Add dependency-direction checks:
  - Cargo package dependency assertions;
  - `cargo tree`-based script;
  - or a small custom check that fails on forbidden imports/package edges.
- Document package-to-test-suite mapping.
- Update `tests/run-all.sh` and any commit hooks to run package-aware checks.
- Add CI/job comments explaining when a suite can be skipped and what condition invalidates that
  skip.
- Remove temporary re-exports and compatibility modules left from earlier phases.

## Test Selection Policy Draft

- `rts-contract` or `rts-protocol` changed:
  - Rust contract/protocol tests;
  - compact snapshot tests;
  - JS protocol mirror/decode tests;
  - Node integration if any top-level message shape changed.
- `rts-rules` changed:
  - rules tests;
  - sim tests that consume stats/formulas;
  - client config mirror checks when visible balance values changed;
  - balance patch notes for player-facing tuning.
- `rts-sim` changed:
  - sim package tests;
  - deterministic replay tests;
  - relevant live server integration tests for changed behavior.
- `rts-ai` changed:
  - AI package tests;
  - `RTS_FULL_AI_TESTS=1 cargo test` when strategy/profile/self-play behavior changed.
- `rts-server` changed:
  - server/lobby tests;
  - Node live-server integration/regression tests;
  - client smoke when connection/snapshot delivery changes.

## Design Notes

The point is certainty, not speed theater. A test can be skipped only when the package graph and
changed files show the exercised behavior cannot have changed. Cross-contract changes still require
end-to-end tests even if only one package changed.

## Tests

- Run the full local gate once after policy and script updates.
- Verify dependency-direction checks fail on a deliberately forbidden edge in a throwaway branch or
  local patch.
- Verify package-aware test selection maps changed files to the expected suites.

## Done

- Docs match the implemented crate graph.
- Forbidden architecture edges are checked automatically.
- Temporary migration shims are gone or explicitly tracked.
- Test-selection policy is written down and wired into local/CI scripts where practical.

