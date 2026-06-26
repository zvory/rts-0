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

## 2026 Tauri MVP Shell Gate

The later thin-shell MVP changed the experiment from a local server wrapper into an unsigned macOS
app that loads the live Beta or Mainline website and owns only startup selection, logs, and the
native cursor bridge. It does not bundle or start `rts-server`, client assets, maps, lab scenarios,
or match-history data.

The final manual gate passed on 2026-06-23 using artifact
`maccursor-shell-v0.1.0-252fc8f35a0d-arm64` at git SHA
`252fc8f35a0d1a798229782dcdcffc9666a3ab18`. Beta, Mainline, Lab launch, local shell logs, and the
native cursor gameplay path were accepted for playtesting on macOS 15.7.7 on an Apple M1 Pro
MacBook Pro. The recommendation is to ship this unsigned macOS MVP artifact to playtesters while
keeping signing, notarization, auto-update, cross-platform support, and broader release automation
out of scope.

A follow-up packaging pass keeps the internal `desktop/maccursor-shell` source/crate name but ships
the public app bundle and artifact names as Bewegungskrieg. The unsigned playtest distribution
should prefer a universal DMG so Apple Silicon and Intel Mac playtesters use one download.
