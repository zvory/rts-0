# Phase 1 - Startup Server Picker

## Phase Status

- [ ] Planned.

## Plain-Language Summary

The desktop shell should show a startup choice before loading the game. Players should be able to
choose beta, mainline, local bundled server, or a custom server URL. The chosen server should then
load in the same Tauri window with the native cursor bridge and desktop runtime metadata available.

## Objective

Replace the current immediate local server launch with a startup server selector that supports
remote beta/mainline/custom servers and local mode.

## Scope

- Add a shell-owned startup view that appears before any game server is loaded.
- Add built-in server profiles for beta and mainline.
  - Verify exact URLs during implementation from repo deploy config or live deployment evidence.
  - Expected current defaults are `https://rts-0-zvorygin-beta.fly.dev/` for beta and
    `https://rts-0-zvorygin.fly.dev/` for mainline unless evidence says otherwise.
- Add a local profile that starts the local/bundled server only after the player chooses it.
- Add custom URL entry, validation, and persistence in the user's app config directory.
- Allow remote `https://` URLs and loopback `http://127.0.0.1:*` / `http://localhost:*` URLs.
- Update Tauri navigation and remote capability rules so the selected remote server can load.
- Keep the desktop runtime injection and native cursor bridge available after navigation to remote
  servers.
- Preserve existing env-driven developer shortcuts such as `RTS_DESKTOP_SERVER_URL`,
  `RTS_DESKTOP_AUTOSTART`, and `RTS_DESKTOP_AUTOLOCK`, but make their behavior explicit.
- Add focused tests for URL normalization, profile persistence/defaulting, and runtime script
  metadata.

## Expected Touch Points

- `desktop/maccursor-shell/src-tauri/src/main.rs`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json`
- `desktop/maccursor-shell/src-tauri/capabilities/default.json`
- New startup UI files under `desktop/maccursor-shell/`
- `desktop/maccursor-shell/README.md`
- Tauri crate tests and any small JS harness needed for startup UI behavior

Avoid touching:

- server simulation rules
- protocol DTOs
- balance/config mirrors
- normal browser client startup unless a small desktop-runtime seam is required

## Verification

- Run `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml`.
- Run `node scripts/check-client-architecture.mjs` if any `client/src/` files change.
- Use a local dev run to verify the startup selector can navigate to a loopback server URL.
- Use a remote beta/mainline URL smoke check when network is available.

## Manual Testing Focus

Open the shell and confirm beta, mainline, custom, and local choices appear and navigate correctly.
Confirm the native cursor toggle is still available after loading a selected server.

## Handoff Expectations

The handoff must state the final built-in beta/mainline URLs, where custom profiles are stored, what
navigation policy was chosen for custom URLs, and whether remote pages receive the native cursor
runtime bridge.
