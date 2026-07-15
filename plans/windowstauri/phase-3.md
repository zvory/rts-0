# Phase 3 - Windows Input Hardening

## Phase Status

- [x] Completed on 2026-07-14.

## Objective

Make the Windows shell playable enough for first playtesters. This phase owns real Windows WebView2
input validation and narrow client fixes found during that validation.

## Work

- Run the Windows shell from source against beta.
- Join or create a lobby, start a one-player sandbox or AI match, and test the normal RTS input
  surface:
  - cursor lock through the settings button
  - Escape releases cursor lock
  - window blur releases cursor lock or leaves the app in a recoverable state
  - left-click select
  - drag box-select
  - right-click move/attack/gather commands
  - command-card hotkeys
  - control-group save/recall hotkeys in installed-app mode
  - WASD/arrow/edge pan
  - mouse wheel zoom
  - minimap click and drag
  - HUD/settings clicks while locked and unlocked
- Inspect `window.__RTS_DESKTOP_RUNTIME`, `window.__RTS_NATIVE_CURSOR`, and
  `window.__rtsPointerLockDebug` after cursor-lock attempts.
- Keep Windows cursor capture raw-only. If WebView2 rejects `{ unadjustedMovement: true }`, fail
  closed and treat it as a playtest blocker until a reliable raw-input path is available.
  - Do not retry normal/adjusted Pointer Lock.
  - Prefer WebView2 raw Pointer Lock; add a Windows-native backend only if WebView2 cannot provide
    raw input reliably.
  - Do not weaken macOS native cursor behavior.
- If focus behavior differs from Chrome, fix focus targeting in the existing browser Pointer Lock
  path with focused tests.
- Keep changes scoped to input/runtime behavior. Do not change gameplay rules, protocol, rendering,
  or server behavior.

## Expected Touch Points

- `client/src/input/browser_pointer_lock.js`
- `client/src/input/cursor_lock.js`
- `client/src/input/index.js`
- `client/src/match.js`
- `client/src/match_net_reporter.js`
- `tests/client_contracts/input_contracts.mjs`
- `tests/client_contracts/match_replay_contracts.mjs` only if settings or pointer-lock UI behavior
  changes
- `desktop/maccursor-shell/README.md` for Windows manual test notes
- `plans/windowstauri/phase-3.md` status update

## Implementation Checklist

- [x] Complete a Windows source-run gameplay pass and record failures.
- [x] Fix only the Windows input/runtime failures that block first playtesters.
- [x] Preserve macOS native cursor tests and behavior.
- [x] Add focused contract tests for any new Windows fallback (no fallback was added; the existing
  raw-only rejection contract remains authoritative).
- [x] Re-run the Windows gameplay pass after fixes.
- [x] Mark this phase as done in this file in the implementation commit.

## Verification

```bash
node tests/client_contracts/input_contracts.mjs
node tests/client_contracts/match_shell_contracts.mjs
node scripts/check-client-architecture.mjs
git diff --check
```

Run `node tests/client_contracts/match_replay_contracts.mjs` if pointer-lock settings behavior is
touched.

Manual Windows source-run verification is required for this phase.

## Manual Test Focus

The manual test is the phase. Spend enough time in one match to confirm cursor lock, selection,
orders, panning, minimap, zoom, HUD, settings, and control groups. Record any remaining input
weakness as either a blocker or an accepted first-playtest limitation.

## Handoff Expectations

State whether Windows WebView2 supports the existing raw Pointer Lock request. Include the final
input test matrix result and any known limitations. Tell the packaging agent which source-run commit
is playable enough to package.

## Completed Handoff

- Windows 11 WebView2 150.0.4078.65 accepts the existing
  `requestPointerLock({ unadjustedMovement: true })` request. No normal/adjusted Pointer Lock retry
  exists or is wanted.
- The first pass exposed deployment drift: Beta was still serving `3c1bfc7b0a50`, whose old client
  inferred the macOS bridge from Tauri globals and failed on `maccursor_start`. The supported Beta
  workflow deployed runtime-policy merge `41e9a0baeebf`; Beta later advanced to source-launcher merge
  `33934810d526`, which contains the same fix. The rerun had no native bridge and acquired raw lock.
- Final Windows matrix:
  - raw lock: pass; locked DOM crosshair appeared with no diagnostic panel
  - raw movement: pass; relative deltas moved the DOM crosshair by the requested amounts
  - left-click and drag selection: pass; an engineer selected and its command card appeared
  - right-click order: pass; the selected engineer moved to the locked-cursor destination
  - command-card and control-group keys: pass; input remained locked and responsive
  - arrow/camera pan, edge movement, wheel zoom, minimap click/drag: pass
  - HUD/settings interaction: pass in the lock/unlock workflow
  - Escape: pass; releases Pointer Lock and removes the locked crosshair
  - Alt-Tab: pass; blur releases lock, focus returns cleanly, and settings can reacquire raw lock
- Accepted first-playtest limitation: Windows does not aggressively re-lock after Alt-Tab. The
  player re-enables **Lock cursor pan** in settings. This avoids surprising capture and is
  recoverable.
- Package from a merged commit at or after `33934810d526`; it is the first main commit containing
  both the Windows runtime policy and source launcher that completed the live input pass.
