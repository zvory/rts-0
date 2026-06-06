# Phase 2 — Native cursor confine / hide

Replace the browser Pointer Lock API path in `client/src/input/` with calls into a
Tauri-provided IPC command. The browser path stays as a fallback when the app is
opened in a normal browser (so non-Tauri playtest via the URL still works).

## Why not the browser Pointer Lock API

User preference: use native window APIs where available. Practical reasons that
agree with the preference:

- WKWebView (macOS) has historically had pointer-lock quirks around fullscreen
  transitions and Escape handling that we do not want to debug per OS update.
- The browser API delivers raw movement deltas but also hides the cursor at the
  same position it was locked — for edge-pan RTS UX we'd rather the cursor stay
  pinned at the screen edge so the camera keeps panning.

## What "native cursor handling" means here

Tauri v2's `Window` API exposes:

- `set_cursor_grab(bool)` — confines the cursor to the window bounds.
- `set_cursor_visible(bool)` — hides or shows the OS cursor.

These two together give us: cursor confined to the window, hidden when in
pan-lock mode, visible when not. They do **not** give raw deltas — the JS side
still reads `mousemove` events from the webview and computes deltas itself, which
is what `input/camera_controls.js` already does.

For edge-pan camera (the actual RTS use case) we do not need infinite cursor
movement. The cursor sitting at the window edge is the desired state. This makes
the native path strictly simpler than the browser Pointer Lock API.

## IPC surface

Expose two Tauri commands from `desktop/src-tauri/src/main.rs`:

```rust
#[tauri::command] fn cursor_grab(window: tauri::Window, grab: bool) -> Result<(), String>
#[tauri::command] fn cursor_visible(window: tauri::Window, visible: bool) -> Result<(), String>
```

Register them in `tauri::Builder::default().invoke_handler(...)`.

## Client-side changes

A new module `client/src/input/native_cursor.js`:

- Detect Tauri at runtime: `const isTauri = !!window.__TAURI_INTERNALS__;` (the v2
  global). Do **not** import `@tauri-apps/api` — keep the client free of any build
  step. Use the global `window.__TAURI__.core.invoke` if exposed, otherwise fall
  through to a no-op + browser fallback.
- Export `enterCursorLock()` and `exitCursorLock()` that:
  - In Tauri: `invoke('cursor_grab', { grab: true })` + `invoke('cursor_visible', { visible: false })`.
  - In a normal browser: call the existing browser Pointer Lock path.

Refactor `client/src/input/index.js` (`enterPointerLock` / `exitPointerLock`,
around lines 220–270) and `camera_controls.js` to route through the new module.
Keep the JS-side delta math identical — only the "how we hide and confine" step
changes.

## Escape behavior

Browser Pointer Lock auto-releases on Escape. Native cursor grab does not.
Replicate the contract in JS: when in cursor-lock mode, a keydown Escape (already
handled in `camera_controls.js:10`) calls `exitCursorLock()`. Verify on both
macOS and Windows that Escape still reaches the webview while cursor is grabbed —
it should, since grab does not capture keys.

## Capability allowlist

Tauri v2 capability JSON must allow the two commands for the main window. Nothing
else gets added in this phase — keep the IPC surface minimal.

## Verification

1. In a normal Chrome/Safari browser: pointer-lock toggle still uses the browser
   API and behaves exactly as before.
2. In the Tauri app: pointer-lock toggle hides the OS cursor, confines it to the
   window, and edge-pan works while the cursor is at the edge.
3. Escape releases cursor in both environments.
4. Switching to another app via Cmd-Tab while cursor is grabbed releases it
   cleanly (Tauri should do this; verify, and add an `on_window_event` blur
   handler in `main.rs` that releases grab if not).

## Exit criteria

- The "Lock cursor pan" toggle in the settings menu works in the Tauri app
  without ever invoking the browser Pointer Lock API.
- No regression in browser mode.
- Alt-Tab / Cmd-Tab away never leaves the cursor stuck.

## Risks

- Tauri's `set_cursor_grab` on macOS historically required the window to be the
  key window. Verify after fullscreen transitions (Phase 3) that grab survives
  the transition or is re-applied on the appropriate event.
- The existing `_pointerLockCursor` synthetic cursor in `input/index.js:181-255`
  was drawn because the OS cursor was hidden by Pointer Lock at the lock origin.
  With native hide it may still be useful (gives the player a visible aim point);
  decide whether to keep it. Default: keep it, unchanged.

## Out of scope

Fullscreen. Audio. Anything Windows-only.
