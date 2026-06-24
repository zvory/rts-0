# Phase 3 - Native Cursor Backend Integration

## Phase Status

- [x] Done.

## Plain-Language Summary

Connect the macOS native cursor capture path to the actual in-match input system. The visible cursor
must move from native input immediately, either through a native overlay or a direct event-time
visual update, not through the normal game frame loop. JS should receive the native input stream as
fast as the WebView allows, but if JS is slower than native input, the handoff must say that plainly
instead of hiding it behind batching.

## Objective

Replace the desktop shell's tested cursor-lock path with a native macOS backend while preserving the
existing browser Pointer Lock path for ordinary web play.

## Scope

- Extend `client/src/input/cursor_lock.js` or add an injected desktop cursor collaborator so the
  input module can choose a native desktop backend when available.
- Keep the public `Input` behavior stable for selection, right-click commands, HUD routing, minimap
  routing, placement, targeted commands, and Escape-to-unlock.
- Use native capture enter/exit for the desktop path.
- Update the visible cursor immediately on native movement.
  - Prefer a native overlay cursor if JS/WebView visual updates are measurably slower.
  - If using a DOM cursor for the first integration, update it directly from the native input event
    handler and prove it is not flushed from `Input.update()` or `requestAnimationFrame`.
- Feed native movement into the existing virtual cursor coordinates without intentional coalescing.
- Route native pointer down/up/right-click/wheel into the existing input/router contracts with the
  same coordinates as the native cursor visual.
- Add diagnostics that can show native events received, JS events processed, dropped/backlogged
  events if any, and the active cursor backend.
- Preserve browser Pointer Lock tests and add native-backend contract tests using a fake desktop
  bridge.

## Non-Negotiable Latency Rules

- Do not intentionally accumulate deltas.
- Do not process one native movement per animation frame.
- Do not make cursor movement wait for Pixi render, match health reporting, snapshot apply, or
  `camera.update`.
- Do not silently drop native events to keep JS quiet. If events are dropped because the bridge
  cannot keep up, expose the count in diagnostics and call it out in the handoff.
- Do not claim success unless visible cursor movement is responsive under the test load from Phase 1
  and a real match from Phase 4.

## Expected Touch Points

- `client/src/input/cursor_lock.js`
- `client/src/input/index.js`
- `client/src/input/camera_controls.js`
- `client/src/match.js`
- `client/src/settings_panels.js`
- Tauri native shell/backend files from Phase 2
- `tests/client_contracts.mjs`
- `tests/input_context_menu_contracts.mjs` or `tests/minimap_input_contracts.mjs` if routing changes
- `docs/design/client-ui.md` if the input contract changes

Avoid touching:

- `server/crates/protocol/src/lib.rs`
- `client/src/protocol.js`
- server simulation code
- balance files

## Verification

- Add unit/contract tests proving native backend support detection does not affect normal browser
  Pointer Lock support.
- Add tests proving native movement updates virtual cursor coordinates immediately through the native
  input handler, not through `Input.update()`.
- Add tests proving Escape, blur, and destroy release native capture.
- Add tests proving HUD/minimap router coordinates match native cursor coordinates.
- Run:
  - `node tests/client_contracts.mjs`
  - `node tests/input_context_menu_contracts.mjs` if pointer routing changes
  - `node tests/minimap_input_contracts.mjs` if minimap routing changes
  - `node scripts/check-client-architecture.mjs`

## Manual Testing Focus

In a local desktop-shell match, lock native cursor mode, move the cursor over terrain/HUD/minimap,
right-click move units, box-select, edge-pan, wheel zoom, and press Escape. Watch specifically for
cursor delay, cursor/HUD coordinate mismatch, stuck capture after blur, and differences from normal
browser controls.

## Handoff Expectations

The handoff must state whether the active visual cursor is native or DOM, whether any movement is
batched or dropped, what diagnostics expose latency/backlog, which browser Pointer Lock behavior was
preserved, and what Phase 4 should test in a real match.
