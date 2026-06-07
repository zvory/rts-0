# Phase 2 — Aggressive browser Pointer Lock

Status note: Tauri native cursor handling was tested and rejected. The desktop
app must use the browser Pointer Lock API path, even in Tauri, because the Tauri
cursor bridge produced laggy synthetic cursor behavior on macOS WKWebView.

## Policy

- Do not expose Tauri cursor IPC commands.
- Do not call window cursor APIs for RTS camera pan.
- Keep the client free of Tauri API imports and JS build steps.
- Use browser Pointer Lock aggressively in both browser and desktop shells.
- Support WebKit-prefixed Pointer Lock APIs because macOS Tauri uses WKWebView.

## Verification

1. In Chrome/Safari: pointer-lock toggle uses browser Pointer Lock.
2. In the Tauri app: entering a match and clicking or pressing a gameplay key
   repeatedly requests browser Pointer Lock until it sticks.
3. Escape releases Pointer Lock; the next non-text-entry gameplay gesture can
   reacquire it.
4. Cmd-Tab away never leaves an app-level cursor trap behind, because none exists.
