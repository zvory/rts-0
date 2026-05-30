# Self-Play Spectator Plan

Goal: provide the smallest useful way to watch scripted self-play matches in the browser with zero
fog, while avoiding long-term protocol and architecture damage.

## Constraints

- Prefer a local-dev-only path over a general spectator feature.
- Reuse existing self-play and replay machinery instead of introducing a second simulation mode.
- Avoid changing normal lobby semantics unless there is no smaller clean seam.
- Avoid broad protocol expansion. A narrow dev-only bootstrap is preferable to permanent new
  public gameplay concepts.
- Keep the authoritative `Game` seam intact. New code should compose around
  `start_payload()`, `snapshot_for()`, `enqueue()`, `tick()`, and replay artifacts rather than
  reaching into private systems.

## Minimal Working Implementation

### 1. Dev-Only Watch Mode Bootstrap

- Add a local-only server route or query-param-controlled entry path for "watch self-play".
- Hitting that path should create or attach to a dedicated dev room that:
  - spawns two scripted players,
  - starts immediately,
  - opens the normal match client view without lobby setup.
- Keep this isolated from ordinary multiplayer joins and room discovery.

Why this shape:
- It solves the immediate debugging problem.
- It avoids introducing a permanent spectator role into the normal product flow.

### 2. Feed the Match From Existing Self-Play Scripts

- Reuse the existing scripted self-play command source from `server/src/game/selfplay.rs`.
- Do not create a second copy of the scripts for browser viewing.
- Run those scripted commands through the same `Game::enqueue()` path as tests.

Why this shape:
- If the browser viewer and the failing tests do not share the same scripts and command path, the
  tool will drift and become misleading.

### 3. No-Fog Snapshot Path For Dev Watching

- Add a narrow server-side way to build a full-world snapshot for the watch client.
- Keep the existing fog-filtered `snapshot_for(player)` semantics unchanged for normal players.
- Scope the no-fog path to the dev self-play watch flow only.

Why this shape:
- This is smaller and safer than redefining fog semantics globally.
- It avoids design damage to the existing "human clients only receive fog-filtered data" rule.

### 4. Minimal Client Surface

- Reuse the existing match renderer.
- Add only enough client state to show that the session is:
  - local dev,
  - self-play,
  - no fog.
- Defer timeline controls, scrubbing, save/load UI, and generalized replay browser work.

Why this shape:
- The renderer already knows how to draw a running match. The missing piece is a clean way to feed
  it a watchable state stream.

### 5. Failure Artifact Hookup

- First version may watch a fresh scripted run only.
- Immediately after that works, allow loading a failure artifact or replay log from
  `target/selfplay-failures/` through the same watch flow.
- Prefer command-log replay over bespoke serialized full-state playback.

Why this shape:
- Fresh live viewing is the fastest path to value.
- Artifact playback is the next step that turns test failures into directly inspectable sessions.

## Explicit Non-Goals For V1

- No general-purpose replay browser.
- No permanent lobby spectator feature.
- No production-mode fog bypass.
- No new command authority for spectators.
- No attempt to let the browser drive the self-play scripts.
- No timeline scrubbing unless the replay path proves clean enough to add it cheaply.

## Suggested Implementation Order

1. Add a dev-only bootstrap that launches a scripted match and drops the browser into the game.
2. Add the no-fog snapshot/view path for that bootstrap only.
3. Wire the scripted self-play command source into the live room tick loop.
4. Verify the browser can watch both sides progress end-to-end.
5. Add replay-artifact loading only after the live watch path is stable.

## Acceptance Criteria

- Visiting the local dev watch entry point opens a running match without lobby interaction.
- Two scripted sides issue their normal scripted commands and the match visibly progresses.
- The browser sees the full map with zero fog.
- Normal multiplayer joins still receive fog-filtered snapshots and unchanged lobby behavior.
- The implementation reuses existing self-play/replay logic rather than cloning it.

## Risks To Watch

- Mixing the test harness too directly into `lobby.rs` could create a second game-control path.
- A permanent spectator role added too early could force protocol and UI changes far beyond the
  debugging need.
- If the viewer consumes different scripts than the tests, debugging value collapses.
