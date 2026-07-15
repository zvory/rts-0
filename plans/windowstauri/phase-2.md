# Phase 2 - Windows Source Build

## Phase Status

- [x] Completed on 2026-07-14.

## Objective

Build and run the thin Tauri shell on Windows from source. This phase proves the Windows toolchain,
paths, Tauri config, startup UI, and remote profile navigation work before packaging.

## Work

- Add a Windows developer launcher if useful, for example `desktop/maccursor-shell/run.cmd`, that
  runs only the Tauri shell crate and does not build or start the game server.
- Confirm `desktop/maccursor-shell/src-tauri/tauri.conf.json` is valid for a thin remote shell:
  - product name and identifier are the player-facing `Bewegungskrieg` values
  - `build.frontendDist` points to `../ui`
  - no `externalBin`
  - no bundled game client, maps, lab scenarios, or server resources
  - remote capabilities include only beta/mainline and developer loopback as intended
- Run a Windows-native debug/source build of `desktop/maccursor-shell/src-tauri`.
- Start the shell on Windows and verify:
  - the startup UI loads from the app-local Tauri asset
  - beta and mainline profile buttons are visible
  - selecting beta navigates to `https://rts-0-zvorygin-beta.fly.dev/`
  - selecting mainline navigates to `https://rts-0-zvorygin.fly.dev/`
  - disallowed navigation is rejected or stays within the configured allowlist
  - shell log path actions work on Windows
- Capture every command needed to reproduce the successful source build.
- Do not create a Windows source checkout unless Phase 0 explicitly stopped for and received a repo
  guidance change.

## Expected Touch Points

- `desktop/maccursor-shell/run.cmd` or equivalent developer launcher
- `desktop/maccursor-shell/README.md`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json` only if config drift is found
- `desktop/maccursor-shell/src-tauri/capabilities/default.json` only if navigation permissions need
  tightening
- `plans/windowstauri/phase-2.md` status update

## Implementation Checklist

- [x] Confirm Phase 0 granted the Windows-native shell-build command exception.
- [x] Add or document the Windows source-run command.
- [x] Build the shell crate with Windows-native Rust/MSVC.
- [x] Run the shell and navigate to beta from startup.
- [x] Confirm shell logs are written and revealable on Windows.
- [x] Document any path issue caused by the WSL portal.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

Minimum WSL checks for any repo edits:

```bash
node desktop/maccursor-shell/tests/startup_ui.mjs
git diff --check
```

Windows-native checks after Phase 0 approval:

```powershell
cd C:\Users\Alex\rts-0-control\repo\desktop\maccursor-shell
cargo test --manifest-path .\src-tauri\Cargo.toml
cargo run --manifest-path .\src-tauri\Cargo.toml
```

If `cargo run` cannot work because Tauri requires the CLI for the app asset context, use the
equivalent `cargo tauri dev` command and record that instead.

## Manual Test Focus

Open the Windows shell from source, click Beta, and confirm the lobby browser appears. Return to the
startup screen only if the shell has an explicit path for that; otherwise close/reopen the app and
test Mainline.

## Handoff Expectations

Include exact Windows commands, exact tool versions, whether the WSL portal path was acceptable, and
the first known-good source-run commit SHA. If source running fails, hand off the shortest failing
command and the first actionable error.

## Completed Handoff

- First known-good source-launcher commit: `33bd7aa305109fe0487ca0dfc3de59c157a31aae`.
- Host: Windows 11 Pro x64 with Rust/Cargo 1.97.0, Tauri CLI 2.11.3, WebView2
  150.0.4078.65, Node 24.18, and npm 11.16.
- The WSL UNC checkout worked through `cmd.exe pushd`, which temporarily mapped it to a drive.
  Cargo output remained on the Windows filesystem to avoid linker/path failures.
- The successful source run used:

  ```powershell
  $env:CARGO_TARGET_DIR = Join-Path $env:LOCALAPPDATA 'rts-0\tauri-target-windows-runtime'
  $env:CARGO_BUILD_JOBS = '2'
  cmd.exe /d /c "pushd \\wsl.localhost\Ubuntu\tmp\rts-worktrees\windows-tauri-source\desktop\maccursor-shell && run.cmd"
  ```

- The app-local startup selector rendered both Beta and Mainline. Automated Windows clicks opened
  both allowlisted profiles, and `shell.log` recorded `selected_profile`, `navigation_started`, and
  `navigation_finished` for each exact URL.
- Beta rendered the connected lobby in WebView2. Mainline also finished navigation. The fresh
  Windows runs emitted no macOS native-cursor capture request.
- **Reveal logs** opened the Windows Explorer `logs` directory and recorded
  `log_directory_revealed`. The verified log file was
  `%LOCALAPPDATA%\dev.bewegungskrieg.Bewegungskrieg\logs\shell.log`.
- `startup_ui.mjs` now locks the thin-shell config contract: no `externalBin`, no bundled resources,
  and a launcher that starts only this Tauri crate.
