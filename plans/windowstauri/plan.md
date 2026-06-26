# Windows Tauri Shell Plan

## Purpose

Ship an unsigned Windows Tauri shell for first playtesters. The core deliverable is a thin desktop
shell that launches on Windows, shows the existing release-channel startup screen, loads beta or
mainline over HTTPS, and lets players complete normal RTS input flows in the WebView. This plan does
not build, bundle, or ship the Rust game server for Windows.

## Current Evidence

- The source of truth is still the WSL repo at `/home/alex/dev/rts-0`; the Windows portal is
  `C:\Users\Alex\rts-0-control\repo`, which points through `C:\Users\Alex\rts-0-wsl` to the WSL
  filesystem.
- The current `origin/main` shell is `desktop/maccursor-shell`, but it is no longer only a local
  server launcher. It has a checked-in startup UI under `desktop/maccursor-shell/ui`, built-in beta
  and mainline profiles, Tauri remote URL allowlisting for those profiles, and local shell logs.
- The remaining hard blocker is platform policy. `desktop/maccursor-shell/src-tauri/src/main.rs`
  still returns an error on non-macOS, injects `platform: "macos"`, requires the macOS native cursor
  backend, and disables browser Pointer Lock for every Tauri runtime.
- The client already has the browser Pointer Lock path needed by Windows WebView2. The Windows work
  is to keep `__RTS_DESKTOP_RUNTIME` for installed-app hotkeys while preventing the macOS
  `__RTS_NATIVE_CURSOR` bridge from being inferred or installed on Windows.
- This Windows host currently has WebView2 installed, but Windows-native `rustc`, `cargo`, `node`,
  `npm`, and `git` were not on `PATH` during the investigation. WSL has Rust, Node, npm, and Git,
  but those produce Linux binaries and are not enough for a Windows Tauri artifact.

## External References

- Tauri Windows prerequisites: Microsoft C++ Build Tools and Microsoft Edge WebView2 are required
  for Windows development; WebView2 is already present on most Windows 10 1803+ and later systems.
  <https://v2.tauri.app/start/prerequisites/#windows>
- Tauri CLI installation: `cargo install tauri-cli --version "^2.0.0" --locked` is the Cargo path.
  <https://v2.tauri.app/reference/cli/>
- Tauri Windows installers: build Windows installers on a Windows computer with `cargo tauri build`;
  prefer NSIS first for this playtest because MSI adds a VBSCRIPT requirement.
  <https://v2.tauri.app/distribute/windows-installer/>

## Overall Constraints

- Keep `/home/alex/dev/rts-0` as the source of truth for repo edits unless the user explicitly
  changes the repo guidance. Do not create a Windows checkout or copy the repo into `C:` as a
  default action.
- Current repo guidance forbids agents from running repo `git`, `cargo`, `npm`, `node`, `bash`, or
  test commands natively on Windows. Phase 0 must either get an explicit, narrow exception for the
  Windows Tauri shell build commands or stop with that blocker documented.
- Use WSL commands for repo inspection, normal edits, git, and ordinary tests. Use Windows-native
  commands only for the Windows toolchain audit/build exception established in Phase 0.
- Do not build, bundle, or ship `rts-server.exe` in the core path. The shell should load game content
  from the selected beta/mainline release channel.
- Preserve the working macOS native cursor shell. macOS should keep native cursor capture and
  browser Pointer Lock disabled inside the shell.
- Windows should use browser/WebView Pointer Lock. It should not expose `__RTS_NATIVE_CURSOR`, mark
  `nativeCursorCapture`, or replace `requestPointerLock` unless a future Windows native cursor
  backend is deliberately added.
- Keep Tauri remote navigation restricted to the built-in release-channel origins and explicitly
  allowed developer loopback URLs.
- Do not add a JavaScript app bundler. The startup UI is plain modules; the game client continues to
  load from the selected server.
- Each implementation phase should land on its own `zvorygin/` branch, be pushed as an owned PR with
  auto-merge armed, and wait for a definite PR merge with the phase head reachable from `origin/main`
  before the next phase starts.
- When a phase is complete, mark its phase document done in the implementation commit and provide a
  handoff message describing what changed, what the next agent should do, and the core manual test
  focus.

## Phase Summaries

### [Phase 0 - Windows Workstation Gate](phase-0.md)

Audit and prepare the Windows-native build environment without changing product behavior. This phase
must verify or install MSVC Build Tools, Rust, the Tauri CLI, Node, Git for Windows, and WebView2,
then decide whether the existing WSL portal is acceptable for native Windows builds. It also records
the explicit command-surface exception needed before any agent runs Windows-native Tauri build
commands from this repo.

### [Phase 1 - Platform Runtime Split](phase-1.md)

Remove the macOS-only runtime assumption while keeping the macOS native cursor path intact. This
phase should make the shell launch on Windows, inject a Windows desktop runtime that uses browser
Pointer Lock, and keep startup profile selection, logging, and navigation allowlists working across
platforms. It should add focused Rust and JS contract tests proving macOS still exposes the native
cursor bridge while Windows does not.

### [Phase 2 - Windows Source Build](phase-2.md)

Build and run the thin shell on Windows from source against beta/mainline, still without creating a
playtester installer. This phase should add a Windows developer launcher or documented command path,
exercise the startup UI, verify the shell can navigate to beta, and capture any native toolchain,
path, or WebView2 failures. The output is a reproducible source-run procedure and a short build log
handoff for the packaging phase.

### [Phase 3 - Windows Input Hardening](phase-3.md)

Validate real gameplay input inside the Windows WebView and fix only the narrow issues found there.
This phase should verify browser Pointer Lock, Escape/blur release, edge pan, minimap, right-click
orders, wheel zoom, HUD interactions, and installed-app control-group hotkeys. If WebView2 rejects
raw mouse input or focus behavior differs from Chrome, this phase owns the targeted client fallback
and tests.

### [Phase 4 - Unsigned Windows Artifact](phase-4.md)

Create a repeatable unsigned Windows playtest artifact for the thin shell. This phase should extend
or mirror the existing unsigned macOS build script so it produces an NSIS setup executable or an
equivalent first-playtest artifact, plus manifest, checksum, contents listing, and user-facing
README. The artifact must prove it does not include `rts-server`, the browser game client, maps, lab
scenarios, match-history databases, or other server runtime assets.

### [Phase 5 - Playtester Release Rehearsal](phase-5.md)

Install the unsigned artifact on a clean Windows user profile or fresh Windows machine and run the
same flow a first playtester will run. This phase should document SmartScreen/unsigned-app friction,
startup channel selection, beta gameplay, log retrieval, uninstall behavior, and the exact artifact
path to send to playtesters. The phase is complete only when the release checklist says the shell is
good enough for first external Windows playtesters or lists the remaining blocker.

## Phase Index

0. [Phase 0 - Windows Workstation Gate](phase-0.md)
1. [Phase 1 - Platform Runtime Split](phase-1.md)
2. [Phase 2 - Windows Source Build](phase-2.md)
3. [Phase 3 - Windows Input Hardening](phase-3.md)
4. [Phase 4 - Unsigned Windows Artifact](phase-4.md)
5. [Phase 5 - Playtester Release Rehearsal](phase-5.md)

## Non-Goals

- Do not port the server for Windows in the core path.
- Do not bundle local maps, lab scenarios, match-history data, or the static game client into the
  Windows shell artifact.
- Do not add a Windows native cursor backend unless browser Pointer Lock is proven unusable.
- Do not sign, notarize, timestamp, or auto-update the first Windows playtest artifact.
- Do not add Electron or another shell unless Tauri cannot pass the Windows source-run and gameplay
  gates.
- Do not make the shell navigate to arbitrary user-entered URLs for first playtesters.

## Required Verification Themes

- `git diff --check`
- `node desktop/maccursor-shell/tests/startup_ui.mjs`
- `node tests/client_contracts/input_contracts.mjs`
- `node tests/client_contracts/match_shell_contracts.mjs` if runtime diagnostics or settings UI
  behavior changes
- `node scripts/check-client-architecture.mjs` for client module changes
- `cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml` where the current host
  can run the shell crate tests
- A Windows-native source run of the shell once Phase 0 grants the command exception
- A manual Windows gameplay pass against beta before packaging

## Implementation Process

Implement one phase at a time. Do not start a later phase from an assumed merge; use the PR wait
gate and confirm the phase head is reachable from `origin/main`. For unattended executor passes after
the plan is approved, use explicit phase ids, for example:

```bash
scripts/phase-runner.sh --plan windowstauri phase-1 --pr --wait
```

Phase 0 includes machine setup and may need human confirmation before a PR is meaningful. If Phase 0
cannot establish the Windows-native command exception or a viable build path from the WSL portal,
stop and hand off the blocker instead of starting Phase 1.
