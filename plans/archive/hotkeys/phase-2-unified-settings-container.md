# Phase 2 - Unified Settings Container

Status: Not Started

## Goal

Extract settings into a reusable container with portable tab content before adding the hotkey
editor. The settings surface should be reachable by the gear icon from lobby, live match, and replay
screens while preserving existing audio behavior and live-match controls. Settings must not pause
the match.

## Scope

- Create a settings container module that owns opening, closing, tabs, focus behavior, and teardown.
- Move audio settings out of `bootstrap.js` into portable panel content without changing volume or
  unlock behavior.
- Add initial tabs: `Game`, `Hotkeys`, `Audio`, and conditional `Debug`.
- Mount context-specific content for lobby, live match, spectator, and replay.
- Keep give up in the live-match settings title/action area only for non-spectator live players.
- Preserve pointer-lock and debug movement waypoint controls where available.
- Keep the gear icon as the only opener; do not add a settings hotkey.

## Expected Touch Points

- `client/index.html`
- `client/styles.css`
- `client/src/app.js`
- `client/src/match.js`
- `client/src/lobby.js`
- `client/src/bootstrap.js`
- New settings modules under `client/src/`
- `tests/client_contracts.mjs`
- `tests/client_smoke.mjs`

## Design Notes

- Settings container and panel content should be separate. Panel content should receive
  collaborators through dependency injection rather than importing `Match`, `Lobby`, or global DOM
  handles directly.
- Existing `#settings-button` can remain pinned, but the current `#settings-menu` markup should
  become a mount point rather than the owner of all settings content.
- Escape should close settings before gameplay cancel behavior clears selection, matching current
  smoke coverage.
- Give up should remain blocked for spectators and replay viewers. Replay viewers can show settings
  and audio, but no replay-specific hotkeys are part of this effort.
- If a module adds DOM listeners, timers, or subscriptions, implement `destroy()` and call it from
  the owning lifecycle.

## Verification

- Add contract coverage for tab visibility and context-specific controls.
- Preserve or update smoke coverage for gear-open settings and Escape behavior.
- Verify audio sliders still call the same `Audio` methods and unlock status updates still work.
- Run `node scripts/check-client-architecture.mjs`.
- Run targeted `node tests/client_contracts.mjs`.
- Run relevant client smoke coverage.

## Manual Testing Focus

- Open settings from lobby, live match, spectator match, and replay.
- Confirm audio controls work in each context.
- Confirm live non-spectator players see give up and spectators/replay viewers do not.
- Confirm debug controls appear only when movement waypoint diagnostics are available.
- Confirm the match keeps running while settings are open.

## Handoff Expectations

The handoff should describe the settings container API, the panel-content registration model, and
the context object shape available to the Hotkeys tab. It should call out any remaining pinned DOM
ids that Phase 3 must preserve.
