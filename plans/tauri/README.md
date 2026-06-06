# Tauri Native Client Plan

Ship the existing web client as an unsigned Tauri desktop app for early playtesters.
macOS first (local build), Windows second (GitHub Actions). The server stays remotely
hosted on fly.io; the desktop app is a thin webview shell that loads the live client
from a single hardcoded URL.

## Goals

- Native desktop launch that opens the live client.
- Native cursor confinement / hide (not the browser Pointer Lock API).
- Native fullscreen toggle.
- Reproducible Windows build out of GitHub Actions.
- Unsigned everything; document the macOS quarantine and Windows SmartScreen workarounds.

## Non-goals

- Code signing, notarization, auto-update.
- Offline play. The app requires network at launch.
- Bundling the client. The webview loads it from the server.
- Shipping the map editor or any `/dev/*` route in the native shell.

## Constraints assumed

- Server is reachable at `https://rts-0-zvorygin.fly.dev/` and will later move to
  `https://bewegungskrieg.net/`. The default URL lives in **one** constant so it can
  be flipped before a playtest build without touching anything else.
- No Apple Developer ID, no Windows EV cert. Gatekeeper / SmartScreen warnings are
  acceptable and documented.
- Tauri v2 (current as of 2026).

## Phases

- [Phase 0 — Scaffold and macOS smoke build](phase-0-scaffold.md)
- [Phase 1 — Point at the live server](phase-1-remote-load.md)
- [Phase 2 — Native cursor confine / hide](phase-2-native-cursor.md)
- [Phase 3 — Native fullscreen](phase-3-native-fullscreen.md)
- [Phase 4 — Audio gesture prompt](phase-4-audio-gesture.md)
- [Phase 5 — GitHub Actions Windows build](phase-5-windows-ci.md)
- [Phase 6 — Vendor PixiJS](phase-6-vendor-pixi.md)

## What success looks like

A playtester downloads a `.dmg` or `.exe` from a GitHub Release, dismisses the OS
"unidentified developer" dialog once with a documented one-liner, launches the app,
and is in the lobby on the live server within ten seconds. Cursor confine and
fullscreen work without the browser's Pointer Lock prompt. The shell never needs to
be rebuilt for ordinary client changes — only for shell-level changes (server URL,
window flags, native API surface).
