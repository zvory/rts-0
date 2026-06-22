# Tauri Retrospective

We tried shipping Bewegungskrieg through a Tauri native shell and removed it.

The goal was a small desktop wrapper around the live web client, mainly to make RTS inputs less
fragile for playtesters. The useful target was reliable edge panning and control-group hotkeys
without browser chrome or tab shortcuts fighting the game.

It did not work well enough to keep. Tauri native cursor handling produced laggy synthetic cursor
behavior, and falling back to the browser Pointer Lock API inside the shell was not meaningfully
better. The Pointer Lock browser API path is still browser-dependent and awkward enough that a
native wrapper did not justify its packaging, CI, debugging, and webview differences.

The better path is to keep the client as a normal browser game:

- Install the site as an app when a browser supports it, so the game runs without normal tab chrome.
- For playtests, run the game as the only active tab or window when control groups matter.
- Keep control groups browser-friendly: Windows browser tabs and browser fullscreen use
  `Alt+number` to save groups, while installed-app/standalone display mode accepts
  `Alt+number`, `Ctrl+number`, and `Cmd+number`.

Tauri-specific app code, build scripts, release workflows, and source detection were removed so the
supported surface is just the web client and server.

## 2026 Native Cursor Spike Note

The local `desktop/maccursor-shell` spike does not reverse that product decision. It is a disposable
macOS test shell for the native cursor plan: it starts the existing Rust server on a loopback
ephemeral port, loads the served web client at that same origin, and injects a desktop runtime flag
for later native cursor integration.

Unlike the removed Tauri attempt, this shell deliberately disables browser Pointer Lock in the
desktop path. Any later cursor success must come from the native macOS backend, not from measuring
Chromium/WKWebView Pointer Lock again.
