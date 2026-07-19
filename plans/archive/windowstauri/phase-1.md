# Phase 1 - Platform Runtime Split

## Phase Status

- [x] Done. Completed on 2026-07-14.

## Objective

Make the Tauri shell runtime platform-aware. macOS should keep the current native cursor behavior;
Windows should launch the shell and use browser/WebView Pointer Lock.

## Work

- Remove the non-macOS hard stop in `desktop/maccursor-shell/src-tauri/src/main.rs`.
- Introduce an explicit shell platform/runtime policy that can be unit-tested without compiling on
  every target:
  - macOS: `platform: "macos"`, native cursor backend present, native cursor capture required,
    browser Pointer Lock disabled, `__RTS_NATIVE_CURSOR` injected.
  - Windows: `platform: "windows"`, native cursor backend absent, native cursor capture not
    required, browser Pointer Lock allowed, no `__RTS_NATIVE_CURSOR` injection.
  - Other desktop targets may use a conservative browser Pointer Lock policy if they compile, but
    they are not a release target for this plan.
- Keep startup profile metadata, beta/mainline release URLs, same-origin target-blank redirect,
  navigation allowlisting, timeout handling, and shell logging active on all supported desktop
  platforms.
- Gate macOS-only runtime calls:
  - `NativeCursorBackend::install`
  - capture stop calls on blur/close if they assume native capture
  - `set_activation_policy(tauri::ActivationPolicy::Regular)` if it is not valid on Windows
  - the macOS native cursor bridge JavaScript and Pointer Lock replacement block
- Make `desktop_reveal_logs` cross-platform:
  - macOS: `open <log-dir>`
  - Windows: `explorer.exe <log-dir>`
  - other desktop targets: a documented best-effort opener or clear error
- Update `client/src/input/cursor_lock.js` so Tauri globals alone do not infer macOS native cursor
  mode. `installedAppRuntime()` should still return true for `__RTS_DESKTOP_RUNTIME`, but it should
  not install a native cursor bridge as a side effect on Windows.
- Update `client/src/match_net_reporter.js` and pointer-lock diagnostics as needed so Windows Tauri
  reports show desktop runtime present, Tauri globals present, and native cursor absent/inactive.
- Preserve installed-app hotkey behavior in `client/src/input/control_groups.js`.

## Expected Touch Points

- `desktop/maccursor-shell/src-tauri/src/main.rs`
- `desktop/maccursor-shell/src-tauri/src/native_cursor.rs`
- `desktop/maccursor-shell/src-tauri/src/diagnostics.rs` only if log-opening helpers move there
- `client/src/input/cursor_lock.js`
- `client/src/input/browser_pointer_lock.js` only if a small helper is needed
- `client/src/match_net_reporter.js`
- `tests/client_contracts/input_contracts.mjs`
- `tests/client_contracts/match_shell_contracts.mjs`
- `desktop/maccursor-shell/src-tauri` Rust unit tests
- `desktop/maccursor-shell/README.md` if source-run docs need a short note

## Implementation Checklist

- [x] Add a runtime policy helper and use it for initialization script generation.
- [x] Make Windows runtime script omit the native cursor bridge and Pointer Lock override.
- [x] Keep macOS runtime script behavior equivalent to current behavior.
- [x] Make log reveal/open commands platform-aware.
- [x] Update client Tauri/native cursor inference so Windows uses browser Pointer Lock.
- [x] Add focused tests for macOS-vs-Windows runtime script behavior.
- [x] Add focused JS tests for Windows desktop runtime using browser Pointer Lock.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

Use WSL/repo commands unless Phase 0 explicitly granted a Windows-native exception:

```bash
node desktop/maccursor-shell/tests/startup_ui.mjs
node tests/client_contracts/input_contracts.mjs
node tests/client_contracts/match_shell_contracts.mjs
node scripts/check-client-architecture.mjs
cargo test --manifest-path desktop/maccursor-shell/src-tauri/Cargo.toml
git diff --check
```

If the Windows-native shell-build exception is available, also run the shell crate tests with the
Windows toolchain.

## Manual Test Focus

No full gameplay pass is required yet. On macOS, confirm the shell still injects native cursor
runtime flags. On Windows, confirm the shell opens instead of exiting with a macOS-only error and
that the startup screen renders.

## Handoff Expectations

State the exact runtime fields injected on macOS and Windows. Call out whether any platform-specific
Tauri API had to be gated. Tell the next agent the exact Windows command to use for the first source
run.

## Phase Handoff

- macOS still injects `platform: "macos"`, all four native-cursor flags as `true`, the existing
  `__RTS_NATIVE_CURSOR` bridge, and the Pointer Lock replacement. Its command permissions are now
  isolated in a macOS-only capability.
- Windows injects `platform: "windows"`, all four native-cursor flags as `false`, and the common
  startup/profile/navigation/logging helpers. Its script contains no `__RTS_NATIVE_CURSOR`,
  `maccursor_*`, or Pointer Lock replacement code.
- Tauri globals alone no longer infer macOS native-cursor mode. Windows still counts as an installed
  app through `__RTS_DESKTOP_RUNTIME` and reports Tauri diagnostics with native cursor absent.
- Browser Pointer Lock remains raw-input-only. Rejection of `{ unadjustedMovement: true }` still
  fails closed; no adjusted-movement fallback was added.
- `desktop_reveal_logs` uses `open` on macOS and `explorer.exe` on Windows.
- Added the Windows `icon.ico` required by `tauri-build` and listed it in the bundle config.
- Windows crate tests pass with a worktree-specific Windows-local Cargo target and
  `CARGO_BUILD_JOBS=2`; use a temporary `pushd` mapping when the source lives in a WSL worktree.
