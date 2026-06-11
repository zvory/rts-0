# Phase 1 - Unified Settings Shell

## Objective

Create the unified settings modal and move existing settings controls into portable settings
sections. This phase should not change command-card hotkey behavior yet.

## Scope

- Add a settings shell that can be opened from the gear icon in lobby, live match, and replay
  screens.
- Keep settings opening gear-only; do not add a settings hotkey.
- Implement tab support for:
  - Game
  - Hotkeys, initially a placeholder explaining that custom hotkeys are not active yet
  - Audio
  - Debug, only when debug controls are available
- Move existing audio controls into the Audio tab while preserving current volume persistence and
  unlock behavior.
- Move current pointer-lock/game controls into the Game tab where applicable.
- Move movement waypoint debug control into the Debug tab when available.
- Show give up in the live-match settings title area for active players only.
- Keep give up hidden for spectators and replay viewers.
- Keep settings non-pausing.
- Separate the settings container from panel content so later work can move the content into another
  container type without rewriting settings sections.

## Likely Touch Points

- `client/index.html`
- `client/styles.css`
- `client/src/bootstrap.js`
- `client/src/app.js`
- `client/src/match.js`
- new focused settings UI modules under `client/src/ui/` or another client area consistent with
  existing architecture
- `docs/design/client-ui.md` if exported module contracts change meaningfully

## Verification

- `node scripts/check-client-architecture.mjs`
- Existing relevant JS tests selected by `node tests/select-suites.mjs --verify`
- Browser smoke test if the changed shell affects match/lobby navigation.

## Manual Testing Focus

Open settings from lobby, a live match, and a replay. Confirm audio controls still work, live match
give up is visible only for active players, spectator/replay views do not show give up, and debug
controls appear only when available.

## Handoff Expectations

The handoff should describe the settings shell API, where settings sections are registered, and how
later phases should add the Hotkeys panel. It should also list any old settings DOM ids that remain
for compatibility.

## Player-Facing Outcome

Players get one settings modal across lobby, match, and replay. Existing audio/game/debug controls
are easier to find, but hotkeys are not editable yet.

