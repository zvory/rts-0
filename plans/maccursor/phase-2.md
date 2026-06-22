# Phase 2 - Desktop Shell Thin Slice

## Phase Status

- [ ] Planned.

## Plain-Language Summary

Create the thinnest macOS desktop shell that launches the existing game server and opens the current
client. This phase is not allowed to claim cursor success; it only proves the app can run through a
desktop shell without changing gameplay. Pointer Lock must stay out of the native-capture test path
so later phases do not accidentally measure Chromium/WKWebView behavior again.

## Objective

Host the current RTS client in a macOS-first Tauri shell and preserve the existing same-origin
server/client model enough for a local spike.

## Scope

- Add a Tauri shell for local macOS spike usage.
- Launch the existing `rts-server` on `127.0.0.1:<free-port>` or document a reliable dev command
  that starts the server before the shell.
- Load the served client URL in the shell.
- Keep the existing `/ws` derivation working from `window.location`.
- Add a server asset-path override if needed so a packaged or shell-launched server can find
  `client/` and `server/assets/maps/` outside the source-tree compile-time assumptions.
- Keep the native cursor backend disabled by default until Phase 3 wires it into the game.
- Add a clear desktop runtime flag visible to the client, such as a Tauri-injected capability, but
  do not use it to bypass normal web behavior outside the desktop shell.

## Non-Negotiable Latency Rules

- Do not use browser Pointer Lock as the desktop-shell success path.
- Do not add a frame-coalesced cursor transport in this phase.
- Do not add synthetic DOM mousemove flooding.

## Expected Touch Points

- New Tauri desktop shell files under a clearly named directory.
- Server startup helper code or docs for the shell.
- Possible `server/src/main.rs` asset-path override, kept narrow and covered by tests if changed.
- `client/src/bootstrap.js` or a small app-shell/platform helper only if the client needs to detect
  the desktop runtime.
- `docs/tauri-retrospective.md` only if the implementation needs to clarify how this spike differs
  from the removed Tauri attempt.

Avoid touching:

- simulation crates
- protocol DTOs
- balance/config mirrors
- gameplay command handling

## Verification

- Run the shell and load the lobby.
- Join or create a local room.
- Start a one-player sandbox or AI match.
- Confirm the WebSocket connects from the shell-loaded origin.
- Confirm ordinary browser play still works from `./runserver`.
- If server asset path handling changed, add focused coverage for default source-tree paths and the
  override path.
- Run:
  - `node scripts/check-client-architecture.mjs` if any `client/src/` file changes
  - focused server tests if `server/src/main.rs` path handling changes

## Manual Testing Focus

Open the shell, reach the lobby, start a sandbox, pan/zoom/select with ordinary mouse behavior, then
quit the shell. Confirm the spawned server exits or is clearly documented as still running.

## Handoff Expectations

The handoff must state how to run the desktop shell, how the server is started, what port/address
model is used, whether ordinary browser play still works, and what Phase 3 should use as the native
cursor capability flag.
