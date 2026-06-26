# Phase 3 - Windows Input Hardening

## Phase Status

- [ ] Not started.

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
- If Windows WebView2 rejects `{ unadjustedMovement: true }`, implement the smallest reliable
  fallback:
  - Prefer a Windows desktop-runtime flag that requests normal Pointer Lock without raw input.
  - If a retry is required, ensure it preserves user-gesture constraints and is covered by focused
    tests.
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

- [ ] Complete a Windows source-run gameplay pass and record failures.
- [ ] Fix only the Windows input/runtime failures that block first playtesters.
- [ ] Preserve macOS native cursor tests and behavior.
- [ ] Add focused contract tests for any new Windows fallback.
- [ ] Re-run the Windows gameplay pass after fixes.
- [ ] Mark this phase as done in this file in the implementation commit.

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
