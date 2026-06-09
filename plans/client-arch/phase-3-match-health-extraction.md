# Phase 3 - Match Health Extraction

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

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- `node tests/select-suites.mjs client/src/match.js client/src/match_health.js`
- Client smoke when practical.

## Safety Notes

This is deliberately non-visual. Do not move settings UI, replay controls, pointer-lock behavior, or
combat audio in this phase. If test setup requires a fake `Net`, keep it tiny and local to the test.

## Outcome

No gameplay or visual change. `Match` becomes less responsible for diagnostics bookkeeping, and
future health/status work has a narrower place to land.
