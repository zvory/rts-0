# Phase 4 - Client Replay Viewer

## Objective

Create a dedicated client replay viewer mode that reuses rendering, camera, minimap, fog, and
audio where appropriate, but disables live gameplay command paths by construction.

## Client Work

- Add a replay mode or `ReplayViewer` class with its own lifecycle.
- Reuse:
  - `GameState`
  - `Camera`
  - `Renderer`
  - `Fog`
  - `Minimap`
  - combat/audio event handling where useful
- Do not construct normal gameplay `Input` command issuance for replay mode.
- Add replay controls for:
  - reset to start
  - rewind fixed amounts
  - speed buttons with `2.0x` active by default
  - fog perspective: all players, one player, or selected players
  - shared state feedback from `replayState`
- Treat replay snapshots as spectator-style fog snapshots, defaulting to all-player union fog but
  allowing live per-viewer perspective changes.
- Show all-player resource information from `playerResources`.
- Hide command card and give-up controls.
- Keep each viewer's camera and viewport local.

## UI Expectations

The first pass should be utilitarian and compact. It should be obvious that playback is a replay,
but the map remains the primary experience.

## Verification

- Client smoke test that replay start constructs the replay viewer and does not expose command UI.
- Browser test or script that speed buttons send replay controls and update from server state.
- Browser test or script that fog perspective controls send the selected player ids and affect only
  the local viewer.
- Teardown test by moving from match to replay to lobby without duplicate listeners or WebGL leaks.

## Player-Facing Outcome

Players get a purpose-built replay viewer instead of a frozen score overlay or a normal spectator
match with hidden controls.
