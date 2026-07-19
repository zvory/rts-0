# Phase 4 - Unsigned Windows Artifact

## Phase Status

- [x] Done. Completed on 2026-07-14 (Windows x64 NSIS build and install smoke).

## Objective

Produce a repeatable unsigned Windows playtest artifact for the thin Tauri shell.

## Work

- Extend `desktop/maccursor-shell/build-unsigned.mjs` to support Windows or add a sibling Windows
  build script with the same manifest discipline.
- Prefer an NSIS setup executable for the first artifact:
  - It is the normal Windows setup executable path in Tauri.
  - It avoids MSI-specific VBSCRIPT friction for the first playtest artifact.
  - It can remain unsigned for this phase.
- Keep the artifact thin:
  - no `externalBin`
  - no `rts-server.exe`
  - no `client/`
  - no `server/assets/maps`
  - no `server/assets/lab-scenarios`
  - no match-history database or replay data
- Build with a Tauri config override if needed rather than permanently enabling all bundle targets
  for every local build.
- Include artifact metadata:
  - git SHA and dirty flag
  - shell package version
  - Windows architecture
  - Tauri CLI version
  - Rust version
  - selected installer target
  - release profile URLs
  - SHA-256 checksum for the setup executable or zip
  - contents listing for any directory artifact
- Add a README beside the artifact explaining:
  - it is unsigned and first-playtest only
  - expected Windows SmartScreen/Defender friction
  - how to install/open it
  - which release channels it can load
  - where logs live and how to retrieve them
  - how to uninstall
- Update repo docs with the build command and expected output path.

## Expected Touch Points

- `desktop/maccursor-shell/build-unsigned.mjs` or
  `desktop/maccursor-shell/build-unsigned-windows.mjs`
- `desktop/maccursor-shell/README.md`
- `desktop/maccursor-shell/src-tauri/tauri.conf.json` only if persistent Windows bundle settings are
  needed
- `desktop/maccursor-shell/src-tauri/capabilities/default.json` only if packaging exposes a
  permission/config problem
- `plans/windowstauri/phase-4.md` status update

## Implementation Checklist

- [x] Confirm Phase 3 identified a playable source-run commit.
- [x] Build an unsigned Windows artifact with the Windows-native toolchain.
- [x] Generate manifest, checksum, and README.
- [x] Assert the artifact does not include server/runtime assets.
- [x] Install or open the artifact on the build machine.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

WSL/repo checks:

```bash
node desktop/maccursor-shell/tests/startup_ui.mjs
git diff --check
```

Windows-native artifact build after Phase 0 approval:

```powershell
cd C:\Users\Alex\rts-0-control\repo\desktop\maccursor-shell
node .\build-unsigned-windows.mjs
```

If the implementation keeps one cross-platform script, use the documented Windows invocation for
that script instead.

## Manual Test Focus

Install or open the produced artifact on the build machine. Confirm the startup screen appears,
Beta opens, shell logs are available, and the installer/artifact can be removed cleanly.

## Handoff Expectations

Provide the artifact path, checksum, manifest path, Tauri output directory, and exact build command.
State explicitly that the artifact does not contain a Windows server binary or bundled game runtime
assets.

## Implementation Handoff

- Builder: `desktop/maccursor-shell/build-unsigned-windows.mjs`
- Windows-local Cargo target: `%LOCALAPPDATA%\rts-0\tauri-target-windows-release`
- Build environment: `CARGO_BUILD_JOBS=1` (the build machine has no page file)
- Validated artifact source commit: `f119496baa826eddb1d1a812db25d820f63300fa`
- Validated installer SHA-256: `4270144e1d1d14870e7b387da6fd6affb584bf84a39cfeecb645a4db765f6b75`
- Install smoke: current-user silent install succeeded, packaged startup screen rendered, Beta navigation
  completed, the local shell log recorded the packaged build id and selected profile, and silent
  uninstall removed the install directory.
- Thin-shell assertion: the manifest reported no forbidden runtime asset matches and no bundled
  external binaries or extra resources. The installer contains no Windows game server or bundled
  game runtime assets.

The release phase must rebuild from the final merged commit and publish that new checksum; the hash
above records the Phase 4 build used for the install smoke only.
