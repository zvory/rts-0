# Phase 3 - Unified Client Overlay

Status: Planned.

## Objective

Mount and polish one observer analysis overlay for both replay viewers and live spectators. The
overlay should no longer present itself as replay-only in user-visible text, storage names where
migration is easy, or module contracts.

## Scope

- Mount the analysis overlay when the start payload is replay playback or live spectator mode.
- Keep command UI hidden for spectators as it is today, and ensure the observer overlay does not
  interfere with camera, minimap, or settings controls.
- Rename or wrap `ReplayAnalysisOverlay` into an observer-oriented module if Phase 1 did not already
  do so.
- Update visible labels, aria labels, empty states, and preference storage names where practical.
  If local-storage migration is needed, keep old replay preference keys readable.
- Verify `destroy()` and branch staging freeze paths remove the overlay cleanly.
- Add or update client contract coverage for replay and live spectator mounting decisions.
- Add a smoke or manual browser check for a live spectator seeing the overlay during an active game.

## Expected Touch Points

- `client/src/app.js`
- `client/src/match.js`
- `client/src/replay_analysis_overlay.js` or renamed observer overlay module
- `client/styles.css`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs` if browser coverage is extended
- `docs/design/client-ui.md` if exported module names/contracts change

## Verification

Run focused client checks:

```bash
node tests/client_contracts.mjs
node scripts/check-client-architecture.mjs
```

If browser smoke coverage is added or touched, run the relevant smoke path through the existing
test harness with a running server.

## Manual Testing Focus

Open a replay and confirm the overlay still works across tab switches, collapse/show, and seeks.
Start a live game with a spectator and confirm the same overlay appears, updates, and can be hidden
or collapsed. Confirm active players still do not see the overlay and can issue commands normally.

## Handoff Expectations

The handoff must describe the final client module names, preference-storage compatibility, and the
manual replay/live spectator checks performed. It should also list any remaining cleanup that was
intentionally left out, such as CSS class aliases kept for compatibility.

## Player-Facing Outcome

Replay viewers and live spectators get the same observer analysis panel for army value, production,
unit counts, losses, and resources-lost tabs.

