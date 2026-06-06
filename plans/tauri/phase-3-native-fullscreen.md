# Phase 3 — Native fullscreen

Add a fullscreen toggle that calls Tauri's `Window::set_fullscreen` instead of
`document.documentElement.requestFullscreen()`. Same fallback pattern as Phase 2:
in a normal browser, fall through to the Fullscreen Web API.

## Why native

- Avoid the browser fullscreen permission/notification banner that WKWebView
  shows on first entry.
- Avoid the Web Fullscreen API's interaction with Escape, which conflicts with
  the cursor-lock Escape contract from Phase 2.
- On macOS, native fullscreen integrates with Mission Control / Spaces correctly;
  the Web API's fullscreen does not.

## IPC surface

One command:

```rust
#[tauri::command] fn set_fullscreen(window: tauri::Window, on: bool) -> Result<(), String>
```

Plus a query so the client can render the toggle state correctly after the user
uses an OS-level shortcut (macOS green button, F11 on Windows):

```rust
#[tauri::command] fn is_fullscreen(window: tauri::Window) -> Result<bool, String>
```

Emit a `fullscreen-changed` event from `on_window_event` when the OS reports a
fullscreen transition so the client UI can refresh without polling.

## Client-side changes

New module `client/src/native_window.js` exporting `setFullscreen(bool)` and
`subscribeFullscreenChange(cb)`. Detect Tauri the same way as Phase 2.

Add a fullscreen button next to the existing pointer-lock toggle in the settings
menu (`client/index.html:124`). Bind to `F11` as a global hotkey in `match.js`.

In normal browser mode the same module falls back to `requestFullscreen()` /
`exitFullscreen()` plus a `fullscreenchange` event listener — keeps non-Tauri
playtest URLs working.

## Verification

1. Tauri app: F11 toggles native fullscreen, the menu button reflects state.
2. macOS green button entering fullscreen fires the change event and the menu
   button updates without manual interaction.
3. Cursor lock (Phase 2) survives a fullscreen transition or is cleanly
   re-applied. Document whichever behavior we settle on.
4. Browser: F11 still works via the Web API.

## Exit criteria

- Fullscreen on/off from inside the app via F11 and the settings menu.
- OS-level fullscreen toggles stay in sync with the in-app indicator.
- No interaction bugs with the Phase 2 cursor lock.

## Risks

- On Windows, F11 in some webview configurations is intercepted by the webview
  itself. If so, swap to `Alt+Enter` or handle the key from the Rust side via a
  global accelerator.
- Multi-monitor: native fullscreen attaches to the monitor the window is on.
  Acceptable; no per-monitor selection UI in v1.

## Out of scope

Borderless-windowed mode. Per-monitor pick. Resolution scaling controls.
