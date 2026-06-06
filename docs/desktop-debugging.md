# Desktop / Tauri Debugging

The desktop app is a thin Tauri shell around the live web client. On macOS it uses
WKWebView, not Chrome, so WebGL, canvas, audio, and inspector behavior can differ from the
regular browser even when the hosted URL is identical.

## Interactive Debug Run

```bash
scripts/run-desktop-debug.sh
```

This launches `cargo tauri dev` with:

- `RTS_DESKTOP_URL=https://rts-0-zvorygin.fly.dev/?rtsDebug=1`
- `RTS_TAURI_OPEN_DEVTOOLS=1`

Override the URL when you need a local server or another deployed build:

```bash
RTS_DESKTOP_URL="http://127.0.0.1:3000/?rtsDebug=1" scripts/run-desktop-debug.sh
```

If automatic inspector opening does not work, try the platform shortcut from the focused app
window. Tauri documents Web Inspector access through right-click Inspect or `Cmd+Option+I` on
macOS, with the inspector implemented by Safari/WebKit on macOS.

## Debuggable Bundle

```bash
scripts/build-desktop-debug.sh
```

This creates a debug bundle and enables the Tauri `devtools` feature for that build. Keep this for
internal debugging only; normal release builds should continue using `scripts/build-desktop.sh`.

## Client Timing Marks

Add `?rtsDebug=1` to the client URL, or run this once in Web Inspector:

```js
localStorage.setItem("rts.debug", "1");
location.reload();
```

The client then writes `[rts-debug]` console rows and keeps the latest lifecycle entries in
`window.__rtsDebug.marks`. High-volume heartbeat and snapshot traffic is counted in
`window.__rtsDebug.counts` instead of being appended to the timeline.

Useful rows for lobby-to-match problems:

- `client.send.start`: the Start Match click sent the start request.
- `server.recv.start`: the server replied with the match start payload.
- `app.onStart.begin`: the client started handling that payload.
- `match.renderer`, `match.staticMap`, `match.minimap`, `match.input`: synchronous client setup
  phases. Large durations here mean the WebView main thread is busy after the server already
  answered.

Quick summary:

```js
__rtsDebug.summary();
```

Filter the timeline:

```js
__rtsDebug.table(/start|match\./);
```

Inspect high-volume counters:

```js
__rtsDebug.counts;
__rtsDebug.last["server.recv.snapshot"];
```

## Triage Pattern

1. If `client.send.start` to `server.recv.start` is slow, inspect server/Fly logs and
   `docs/perf-tracing.md`.
2. If `server.recv.start` appears quickly but `app.onStart.end` is late, inspect the WebView
   profile for synchronous client work.
3. If Chrome is instant but Tauri is slow in `match.staticMap` or renderer setup, treat it as a
   WKWebView/WebGL path first. Compare a local Chrome run with the same `?rtsDebug=1` URL.
4. If no marks appear after a click, inspect the Console for CSP, module-load, or WebSocket
   errors before debugging gameplay code.

## Sources

Tauri v2's debug documentation says devtools are enabled for development/debug builds, can be opened
programmatically with `WebviewWindow::open_devtools`, and require the `devtools` Cargo feature for
production-style builds. Tauri's webview reference also notes macOS uses WebKit through WKWebView.
