# macOS Tauri Desktop Shell Spike

This is the Phase 2 shell for `plans/maccursor/phase-2.md`. It hosts the
current served RTS client in a macOS-first Tauri WebView and keeps the normal
same-origin `/ws` URL model intact.

Run it from this directory on macOS:

```bash
./run.sh
```

The shell starts the existing Rust server with:

- `RTS_ADDR=127.0.0.1:0` so the OS chooses a free loopback-only port.
- `RTS_CLIENT_DIR=<repo>/client` so static client files are found from the
  shell-launched process.
- `RTS_MAPS_DIR=<repo>/server/assets/maps` so the HTTP map catalog uses the
  same source-tree maps during the spike.

The shell reads the server log line that contains `open http://...`, opens that
exact URL in the Tauri window, and keeps WebSocket derivation based on
`window.location`. Use `RTS_DESKTOP_SERVER_URL=http://127.0.0.1:<port>/ ./run.sh`
to point the shell at a server you started yourself instead of spawning one.

The WebView injects `window.__RTS_DESKTOP_RUNTIME` before the client scripts run:

```js
{
  shell: "tauri",
  platform: "macos",
  nativeCursorBackend: false,
  pointerLockDisabled: true,
  serverMode: "spawned" | "external"
}
```

Pointer Lock is deliberately disabled inside this shell. Phase 2 must not claim
cursor success from Chromium/WKWebView Pointer Lock; Phase 3 should use
`window.__RTS_DESKTOP_RUNTIME.nativeCursorBackend === false` as the flag to
replace with a real native cursor backend.

Manual check:

1. Run `./run.sh`.
2. Confirm the lobby loads in the desktop window.
3. Create a local lobby and start a one-player sandbox or AI match.
4. Confirm the WebSocket connects from the shell-loaded origin.
5. Confirm the Pointer Lock toggle fails in the shell, then run `./runserver`
   and confirm ordinary browser play still works.
6. Quit the shell and confirm the spawned server process exits.
