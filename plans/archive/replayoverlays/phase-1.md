# Phase 1 - Replay Overlay Shell

Status: Done.

## Objective

Create the client-side replay analysis overlay framework without adding real analysis metrics yet.
The shell should provide a stable tab surface, preserve the selected tab and collapsed/visible state
across replay seeks, and cleanly mount only for replay viewers.

## Scope

- Add a focused replay analysis UI module under `client/src/`, preferably in the app-shell or UI
  area depending on the final dependency shape.
- Mount it from `Match` only when `replayViewer` is true and the start payload contains `replay`.
- Add a stable DOM container in the game screen or create/destroy the container from the module.
- Provide initial tabs as disabled or placeholder descriptors:
  - Army value
  - Production
  - Units
  - Units lost
  - Resources lost
- Store selected tab and visible/collapsed state outside the per-`Match` instance. Acceptable
  options are an `App`-owned service passed into `Match`, local storage, or both.
- Ensure replay seeks preserve overlay preference state when `App.onStart()` destroys and rebuilds
  the replay viewer.
- Keep controls read-only and non-blocking for camera controls except where buttons are clicked.
- Add `destroy()` and call it from `Match.destroy()` and any freeze/branch staging path that keeps
  a replay frame in the background.

## Expected Touch Points

- `client/index.html` or module-created DOM under `#game-screen`
- `client/styles.css`
- `client/src/app.js`
- `client/src/match.js`
- new `client/src/replay_analysis_overlay.js` or similarly named focused module
- `docs/design/client-ui.md` if a new exported module contract is introduced
- `scripts/check-client-architecture.mjs` only if the final import direction needs an explicit
  allowlist entry with a reason

## Verification

- Run the client architecture check:

```bash
node scripts/check-client-architecture.mjs
```

- Add focused JS/DOM coverage if there is an existing lightweight client contract test that can
  instantiate the overlay without a live server.
- If no suitable DOM test exists, document that gap in the handoff and rely on Phase 2 browser
  smoke coverage once the overlay displays real data.

## Manual Testing Focus

Open a replay, switch tabs, collapse or show the overlay, seek backward and via the timeline, and
confirm the same overlay state returns after the replay viewer rebuilds. Confirm normal live
matches and lobby screens do not show replay analysis controls.

## Handoff Expectations

The handoff must state where overlay state is stored, how it survives seek-triggered `start`
messages, and which lifecycle paths call `destroy()`. It should also name any tabs that are
placeholders and tell the next agent where to plug in the Phase 2 army-value data.

## Player-Facing Outcome

Replay viewers gain a stable analysis panel shell, but no new strategic information is displayed
yet.
