# Phase 7 - Match Shell Collaborators

Status: planned.

## Goal

Extract small collaborators from `client/src/match.js` while keeping `Match` as the explicit
composition shell and preserving lifecycle ordering.

## Scope

- Read `docs/context/client-ui.md` and the `Match` app-shell section of
  `plans/hotspots/responsibility-map.md`.
- Extract one or more small collaborators for net-report/ping management, combat audio event
  handling, or settings action wiring.
- Keep prediction, frame loop, room-time, lab/replay wiring, and teardown ordering in `Match` unless
  a very small local helper can preserve the exact lifecycle.
- Use dependency injection for collaborators rather than broad cross-area imports.
- Update `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if new match helper paths
  are not grouped under `client-match-shell`.

## Touch Points

- `client/src/match.js`
- new match-local helper modules
- match/frame/health/prediction/observer-analysis/teardown contract tests
- `scripts/hotspot-analysis.mjs` and `plans/hotspots/group-map.md` if grouping needs new helper paths
- `plans/hotspotcleanup/phase-7.md`

## Constraints

- Do not change `Match` construction, destruction, listener teardown, GPU/resource teardown, frame
  ordering, prediction adapter lifecycle, room-time handling, lab controls, replay controls, or live
  match transport behavior.
- Do not make helpers reach into `Net`, `ReplayViewer`, HUD, renderer, or lab internals unless those
  dependencies are injected through the shell.
- Do not change player-visible pause, give-up, settings, combat audio, or observer-analysis behavior.

## Verification

- `node tests/client_contracts.mjs` or targeted match-shell contracts plus the stable runner
- `node scripts/check-client-architecture.mjs`
- Browser smoke when helper movement touches lifecycle, frame loop, or teardown behavior
- `git diff --check`

## Manual Testing Focus

Manually start and leave a live match, replay, and lab session. Check pointer lock, settings
controls, pause/give-up controls, combat audio, net report status, observer-analysis controls, and
rematch teardown.

## Handoff

After implementation, mark this phase done and summarize the collaborators, unchanged lifecycle
ordering, commands run, manual checks, and any app-shell clusters left in `match.js` because they are
ordering-sensitive.
