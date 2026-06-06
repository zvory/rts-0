# Phase 5 — GitHub Actions Windows build

Produce an unsigned Windows `.exe` (NSIS) on every push to `main` and on tagged
releases. macOS builds stay local for now — the playtester macOS audience is the
author plus a handful of trusted people, and a local `cargo tauri build` is one
command.

## Workflow file

`.github/workflows/tauri-windows.yml`:

- Trigger: `push` to `main` for `desktop/**` paths, plus `workflow_dispatch`,
  plus `release` published events.
- Runner: `windows-latest`.
- Steps:
  1. `actions/checkout@v4`.
  2. Install Rust stable via `dtolnay/rust-toolchain@stable`.
  3. Cache `~/.cargo` and `desktop/src-tauri/target` via `actions/cache@v4`.
  4. `tauri-apps/tauri-action@v0` with `projectPath: desktop`, no signing keys.
  5. Upload `desktop/src-tauri/target/release/bundle/nsis/*.exe` as a workflow
     artifact. On `release` events, also attach to the GitHub Release.

## What we deliberately skip

- macOS in CI: not needed for v1, and signing-less macOS artifacts produced by
  CI carry a different quarantine attribute story than locally built ones. Avoid
  the complexity.
- Code signing (Windows or macOS).
- WiX `.msi`. NSIS `.exe` only — one artifact format keeps the workflow simple.
- Auto-update artifact signing for the Tauri updater plugin. Not enabled.

## Release flow

1. Bump `version` in `tauri.conf.json`.
2. Tag `desktop-v0.X.Y`.
3. Create a GitHub Release from the tag. CI attaches the `.exe`.
4. Build the macOS `.dmg` locally with `cargo tauri build` and upload it to the
   same release manually.
5. Release notes include the two workaround one-liners (Gatekeeper xattr,
   SmartScreen "More info → Run anyway").

## Verification

1. Push a no-op change under `desktop/`; confirm the workflow runs and produces
   an artifact.
2. Download the `.exe`, run on a Windows machine, dismiss SmartScreen, confirm
   the app opens into the live lobby and plays a match.
3. Confirm cursor lock and fullscreen behave on Windows the same way they did
   on macOS in Phase 2/3.

## Exit criteria

- Tagged release produces a downloadable unsigned `.exe`.
- A first-time Windows playtester can install and play with documented one-time
  SmartScreen bypass.

## Risks

- WebView2 must be present on the target Windows machine. It is preinstalled on
  Windows 11 and most updated Windows 10. The NSIS installer can include the
  WebView2 bootstrapper as a runtime dependency — enable that in the bundle
  config to cover older Windows 10 boxes.
- The Tauri CLI version on the runner must match the one used locally for
  macOS, or bundle differences will produce surprises. Pin via a small action
  step that installs `tauri-cli` with `--version X.Y.Z`.

## Out of scope

macOS CI. Code signing. Auto-update.
