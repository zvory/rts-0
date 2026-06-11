# Phase 3 - Match Health Extraction

## Phase Status

- [x] Implemented.

## Objective

Move network-health bookkeeping out of `Match` into a small, non-visual collaborator. This reduces
`Match` size and risk without touching UI layout, Pixi rendering, input behavior, or gameplay.

## Work

- Add `client/src/match_health.js`.
- Move these responsibilities from `Match` into `MatchHealth`:
  - latency sample tracking
  - snapshot jitter tracking
  - server tick/lag status ingestion
  - issue active/count state
  - status badge publication payload creation
- Keep `StatusBadge` unchanged.
- Keep the `Match` frame loop shape unchanged except for delegating to `MatchHealth`.
- Suggested API:
  - `constructor({ net, statusBadge, snapshotMs })`
  - `noteSnapshotArrival(now, documentHidden)`
  - `applyServerNetStatus(status)`
  - `refreshLatency()`
  - `publish()`
  - `metrics()`
- Add focused tests in `tests/client_contracts.mjs` or a new dependency-free test file for:
  - latency issue threshold
  - jitter window behavior
  - slow tick/head-of-line server status propagation
  - status payload shape

## Implementation Segments

Mark each segment complete as it lands:

- [x] Add `MatchHealth` with the agreed constructor and public methods.
- [x] Move latency, jitter, server-status, and issue-state bookkeeping out of `Match`.
- [x] Keep `StatusBadge` and visible status payload shape unchanged.
- [x] Add focused dependency-free tests for the health thresholds and payloads.
- [x] Run verification and record whether client smoke was run or skipped.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- `node tests/select-suites.mjs client/src/match.js client/src/match_health.js`
- Client smoke when practical.

Completed verification:

- `node tests/client_contracts.mjs` - passed.
- `node scripts/check-client-architecture.mjs` - passed.
- `node tests/select-suites.mjs client/src/match.js client/src/match_health.js` - selected
  `client-architecture`, `js-protocol-contracts`, `node-minimap-input-contracts`, `client-smoke`.
- `tests/run-all.sh --no-rust` - passed, including client smoke.

## Manual Test Prompt

At handoff, ask the user to do this quick browser check:

> Manual testing requested, 5-10 minutes:
> 1. Start or join a match and leave it running for about one minute.
> 2. Watch the status badge and confirm it still appears, updates, and clears normally.
> 3. Switch tabs briefly, return to the match, and confirm the badge does not get stuck in a stale
>    warning state.
> 4. Report any missing badge text, persistent warning after recovery, or console error.

## Handoff Expectations

In the final handoff, include the completed segment checklist, exact verification output summary,
and the filled manual testing prompt above. Tell the next agent to start Phase 4 only after this
phase is committed, merged to `main`, and pushed.

## Safety Notes

This is deliberately non-visual. Do not move settings UI, replay controls, pointer-lock behavior, or
combat audio in this phase. If test setup requires a fake `Net`, keep it tiny and local to the test.

## Outcome

No gameplay or visual change. `Match` becomes less responsible for diagnostics bookkeeping, and
future health/status work has a narrower place to land.
