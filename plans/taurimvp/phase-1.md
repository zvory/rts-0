# Phase 1 - Startup Server Picker

## Phase Status

- [x] Done.

## Plain-Language Summary

The desktop shell should show a startup choice before loading the game. Players should be able to
choose beta or mainline. The chosen release channel should then load in the same Tauri window with
the native cursor bridge and desktop runtime metadata available.

## Objective

Replace the current immediate local server launch with a startup server selector that supports
beta and mainline release channels.

## Scope

- Add a shell-owned startup view that appears before any game server is loaded.
- Add built-in server profiles for beta and mainline.
  - Verify exact URLs during implementation from repo deploy config or live deployment evidence.
  - Expected current defaults are `https://rts-0-zvorygin-beta.fly.dev/` for beta and
    `https://rts-0-zvorygin.fly.dev/` for mainline unless evidence says otherwise.
- Do not add a local dev profile, loopback URL option, or custom server URL entry in the MVP UI.
- Persist the last selected built-in release channel if persistence is useful, but do not create
  custom profile storage.
- Update Tauri navigation and remote capability rules so the selected beta/mainline server can load.
- Keep the desktop runtime injection and native cursor bridge available after navigation to remote
  servers.
- Preserve existing env-driven developer shortcuts such as `RTS_DESKTOP_SERVER_URL`,
  `RTS_DESKTOP_AUTOSTART`, and `RTS_DESKTOP_AUTOLOCK` only as non-MVP engineering aids, and make
  their behavior explicit.
- Add focused tests for built-in profile/defaulting behavior and runtime script metadata.

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
- Use a remote beta/mainline URL smoke check when network is available.

## Manual Testing Focus

Open the shell and confirm only beta and mainline choices appear and navigate correctly.
Confirm the native cursor toggle is still available after loading a selected server.

## Handoff Expectations

The handoff must state the final built-in beta/mainline URLs, the selected-origin navigation policy,
and whether remote pages receive the native cursor runtime bridge. It must also state that local
loopback and custom URL choices are intentionally absent from the MVP UI.
