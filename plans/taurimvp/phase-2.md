# Phase 2 - Packaged Local Runtime

## Phase Status

- [ ] Planned.

## Plain-Language Summary

Local mode should not require a repo checkout or Cargo on the tester's machine. The app bundle
should contain an optimized `rts-server` binary plus the static client and map assets it needs. Dev
runs can keep using Cargo, but packaged runs must use bundled resources.

## Objective

Turn local mode into a distributable app-bundle runtime while preserving the source-tree dev loop.

## Scope

- Build or copy an optimized `rts-server` binary into the Tauri app bundle for the host macOS
  architecture.
- Bundle `client/` and `server/assets/maps/` as app resources.
- Update local server launch to detect packaged vs source-tree mode.
  - Packaged mode launches the bundled server binary.
  - Source-tree dev mode may keep using `cargo run`.
- Set `RTS_CLIENT_DIR`, `RTS_MAPS_DIR`, `RTS_ADDR=127.0.0.1:0`, and `RTS_DESKTOP_SHELL` correctly in
  packaged local mode.
- Enable Tauri bundling for the app target without adding signing or notarization.
- Keep remote beta/mainline/custom modes from starting a local server.
- Make server shutdown on app quit/window close reliable in packaged mode.
- Add tests for packaged resource path resolution and process command selection where practical.

## Expected Touch Points

- `desktop/maccursor-shell/src-tauri/src/main.rs`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json`
- `desktop/maccursor-shell/src-tauri/Cargo.toml`
- `desktop/maccursor-shell/src-tauri/build.rs`
- New packaging/resource helper scripts under `desktop/maccursor-shell/` or `scripts/`
- `desktop/maccursor-shell/README.md`

Avoid touching:

- deployment scripts for Fly unless a build command needs to share a helper
- server gameplay behavior
- match-history defaults beyond preserving the existing env gate

## Verification

- Run `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml`.
- Run the new package-local build helper far enough to prove resources are copied into the expected
  bundle/resource locations.
- Launch local mode from the packaged app on the development Mac if available.

## Manual Testing Focus

Open the packaged app, choose local mode, reach the lobby, then quit and confirm the bundled server
process exits.

## Handoff Expectations

The handoff must state the generated app/resource layout, whether the artifact is Apple Silicon
only or universal, how packaged mode locates the server/client/maps, and any remaining packaging
limitations.
