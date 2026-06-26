# Phase 4 - Unsigned Windows Artifact

## Phase Status

- [ ] Not started.

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

- [ ] Confirm Phase 3 identified a playable source-run commit.
- [ ] Build an unsigned Windows artifact with the Windows-native toolchain.
- [ ] Generate manifest, checksum, and README.
- [ ] Assert the artifact does not include server/runtime assets.
- [ ] Install or open the artifact on the build machine.
- [ ] Mark this phase as done in this file in the implementation commit.

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
