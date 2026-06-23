# Phase 2 - Thin Shell Runtime Boundary

## Phase Status

- [ ] Planned.

## Plain-Language Summary

The shipped app should be a thin shell around whichever website the player selects. It should not
bundle or spawn `rts-server`, `client/`, maps, lab scenarios, or any other game runtime assets. A
local dev option is just a URL to a separately running repo server.

## Objective

Make the shell's runtime boundary explicit: the app owns windowing, server selection, native cursor
support, and logs, while the selected website owns game content and WebSocket service.

## Scope

- Remove or gate the current automatic `cargo run` server launch from the shippable path.
- Ensure the startup selector always navigates to a chosen URL rather than spawning a game server.
- Preserve developer shortcuts only when they mean "open this URL"; avoid hidden server launch in
  packaged app behavior.
- Keep the app bundle free of `rts-server`, `client/`, maps, lab scenarios, match-history database
  files, and source-tree assets.
- Update any docs that describe the shell as starting the existing Rust server.
- Keep local dev usage explicit: start the repo server separately, then choose the local URL profile
  or enter its URL as custom.
- Add tests that prove URL selection does not build a local server process command for shipped
  profiles.

## Expected Touch Points

- `desktop/maccursor-shell/src-tauri/src/main.rs`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json`
- `desktop/maccursor-shell/src-tauri/Cargo.toml`
- `desktop/maccursor-shell/src-tauri/build.rs`
- `desktop/maccursor-shell/README.md`

Avoid touching:

- deployment scripts for Fly
- server gameplay behavior
- server asset-path handling

## Verification

- Run `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml`.
- Inspect the built app bundle or configured resources and confirm it does not include server/client
  game assets.
- Launch local-dev URL mode against a separately running repo server if available.

## Manual Testing Focus

Open the shell, choose beta/mainline/custom/local URL profiles, and confirm no local server process
is launched by the app.

## Handoff Expectations

The handoff must state how the app proves it is thin, how local dev mode is used, and whether any
server-spawning dev shortcut remains outside the shipped path.
