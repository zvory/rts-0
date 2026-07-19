# Phase 0 - Windows Workstation Gate

## Phase Status

- [x] Done. Completed on 2026-07-14.

## Objective

Prepare and verify the Windows-native environment needed to build a Windows Tauri shell. This phase
does not change gameplay behavior and should not attempt to fix shell code.

## Key Decision

The repo guidance currently forbids native Windows repo commands, including `cargo`, `node`, `npm`,
and `git`. A Windows Tauri artifact eventually requires Windows-native `cargo` and the MSVC toolchain,
so this phase must get an explicit, narrow user-approved exception before running Windows-native
build commands in the repo.

The exception should be limited to:

- Windows-native `cargo`, `rustc`, and `cargo tauri` for `desktop/maccursor-shell/src-tauri`.
- Windows-native `node` for `desktop/maccursor-shell` build or startup UI scripts.
- No Windows-native server build unless the user separately authorizes a future bundled-server plan.
- No Windows-native `git`; keep git operations in WSL.

## Work

- Record current Windows state:
  - `Get-Command rustc,cargo,node,npm,git,winget`
  - WebView2 install location under `C:\Program Files (x86)\Microsoft\EdgeWebView\Application`
  - Windows version and architecture
  - whether Visual Studio Build Tools are installed
- Install or verify prerequisites:
  - Microsoft Visual Studio Build Tools 2022 with the "Desktop development with C++" workload.
  - Rust through official `rustup` on Windows, with the `x86_64-pc-windows-msvc` target.
  - Tauri CLI through `cargo install tauri-cli --version "^2.0.0" --locked`.
  - Node LTS for local build scripts and startup UI tests.
  - Git for Windows only if needed by build metadata scripts; do not use it for repo mutation.
  - WebView2 Runtime. The investigation found WebView2 already installed on this machine.
- Suggested `winget` ids verified during investigation:
  - `Microsoft.VisualStudio.2022.BuildTools`
  - `OpenJS.NodeJS.LTS`
  - `Git.Git`
- Use the official Rust installer path if `winget` cannot locate a current Rust package id.
- Decide repo access:
  - First try the existing WSL portal path `C:\Users\Alex\rts-0-control\repo` for Windows-native
    shell builds.
  - If Windows Cargo or Node cannot build reliably from the WSL portal or UNC target, stop and ask
    the user to update repo guidance before creating any Windows checkout or copying the repo into
    `C:`.
  - If a Windows-local build staging directory is needed only for generated artifacts or Cargo
    target output, document it and keep it outside the repo source tree.
- Capture the exact verified versions in the phase handoff.

## Expected Touch Points

- Usually none for product code.
- Optional docs/check script only if the phase discovers a stable, useful prerequisite checker.
- This phase document should be marked done only after the workstation is actually ready or the
  blocker is documented.

## Verification

Run the following in Windows PowerShell after setup:

```powershell
Get-Command rustc,cargo,node,npm,git,winget
rustc --version
cargo --version
node --version
npm --version
cargo tauri --version
```

Run the following through WSL to preserve repo command discipline:

```powershell
C:\Users\Alex\rts-0-control\bin\rts.cmd "git status --short --branch"
```

## Manual Test Focus

No gameplay test is required. Confirm only that the Windows-native shell toolchain exists and that
the next phase has a permitted command path.

## Handoff Expectations

State whether the user granted the narrow Windows-native shell-build exception. Include the installed
tool versions, WebView2 version/location, the path that future phases should use for Windows-native
Tauri commands, and any path/access blocker.

## Phase Handoff

- The user explicitly authorized getting the Windows Tauri client working, including Windows-native
  shell build commands. Git operations remain in WSL and no Windows server build is in scope.
- Verified Windows 11 Pro x64 with Visual Studio Build Tools 2022 and the MSVC x64 tools, WebView2
  Runtime 150.0.4078.65, Rust/Cargo 1.97.0, Tauri CLI 2.11.3, Node 24.18.0, npm 11.16.0, and Git.
- Use `C:\Users\Alex\rts-0-control\repo` for Windows-native shell source commands. The portal is
  viable, but Windows and WSL must not share the shell crate's default `target` directory.
- Set `CARGO_TARGET_DIR` to `%LOCALAPPDATA%\rts-0\tauri-target-windows` for Windows builds.
- This host has no active page file. Default-parallel Cargo failed with Windows error 1455; use
  `CARGO_BUILD_JOBS=1` or `2` until a page file is enabled.
- A two-job native build compiled the Tauri dependency graph and reached the project build script.
  The next product blocker is `src-tauri/icons/icon.ico` being absent; Phase 1/2 should add the
  Windows icon before expecting a complete native build.
