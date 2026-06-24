# Phase 5 - Final Manual Gate

## Phase Status

- [x] Done.

## Final Gate Result - 2026-06-23

Recommendation: ship the unsigned macOS MVP shell to playtesters.

Tested artifact:

- Artifact: `maccursor-shell-v0.1.0-252fc8f35a0d-arm64`
- App bundle: `RTS Mac Cursor Shell.app`
- Git SHA: `252fc8f35a0d1a798229782dcdcffc9666a3ab18`
- Zip SHA-256: `3db3bd908a94bef87c4569b484ba08053f23ce20f1ea3f07c5962ca3e0eb5e00`
- Target: macOS arm64, unsigned, thin-shell bundle

Tested Mac:

- macOS 15.7.7 build 24G720
- MacBook Pro `MacBookPro18,3`
- Apple M1 Pro, 16 GB memory

Release channels checked:

- Beta: `https://rts-0-zvorygin-beta.fly.dev/`, `/version` returned `eea9a3f3250f`.
- Mainline: `https://rts-0-zvorygin.fly.dev/`, `/version` returned `9a86566770a3`.

Manual result:

- Startup selector opened from the built unsigned app and exposed only Beta and Mainline.
- Beta and Mainline navigation worked.
- Lab launch stayed inside the same shell window.
- In-game native cursor capture started and stopped, and the tester confirmed the cursor/gameplay
  path looked good.
- Core cursor workflows were accepted for playtesting: selection boxes, right-click orders,
  HUD/command-card interaction, minimap interaction, edge pan, wheel zoom, Escape unlock, app
  focus/blur cleanup, and normal quit.
- Shell logs were present at
  `~/Library/Logs/dev.bewegungskrieg.MacCursorShell/shell.log` and recorded startup, profile
  selection, navigation, Lab navigation, and native cursor capture events.
- Artifact manifest confirmed no bundled `rts-server`, client assets, maps, lab scenarios, or
  match-history runtime data.

Known follow-up: this is still an unsigned macOS-only playtest artifact. Signing, notarization,
auto-update, cross-platform packaging, and wider release automation remain intentionally out of
scope.

## Plain-Language Summary

This phase is intentionally delayed until someone can test on macOS. Use the built unsigned app
from Phase 4 and run the real startup, server-selection, and native-cursor gameplay checks. The
output should be a clear ship or stop decision for sending the MVP shell to playtesters.

## Objective

Perform the final human gate after the technical shipping prep is complete.

## Scope

- Use the unsigned artifact from Phase 4, not `cargo run`.
- Verify startup selection for beta and mainline.
- Verify basic logs are created and accessible after normal launch and at least one forced failure.
- Confirm the shipped artifact is thin and does not include or start a local game server.
- Run the original maccursor in-game plausibility checks:
  - native cursor capture starts and stops,
  - cursor feels immediate enough for playtesting,
  - selection boxes, right-click move/attack, HUD/command-card clicks, minimap, edge pan, wheel
    zoom, and Escape behave acceptably,
  - Command-Tab, blur, window close, and app quit restore normal cursor control.
- Compare against normal browser play on the same Mac when practical.
- Apply only small fixes found during the gate. Larger product changes should become a follow-up
  plan.
- Update `docs/tauri-retrospective.md`, `plans/archive/maccursor/phase-4.md`, or this phase with the final
  go/no-go result.

## Expected Touch Points

- Tauri shell/native cursor files for small fixes
- startup UI files for small fixes
- `desktop/maccursor-shell/README.md`
- `docs/tauri-retrospective.md`
- `plans/archive/maccursor/phase-4.md`
- this phase document

Avoid touching:

- packaging architecture beyond small release-blocker fixes
- server simulation rules
- protocol contracts
- balance values

## Verification

- Re-run focused checks for any files changed in this phase.
- Confirm the unsigned artifact still builds after any fixes.
- Record the app artifact identifier and git SHA used for the manual gate.

## Manual Testing Focus

Use the app as a player would: pick beta or mainline, join or create a match, enable native cursor
mode, and play long enough to judge latency and cleanup reliability.

## Handoff Expectations

The handoff must include a ship/stop recommendation, tested artifact id/SHA, tested macOS hardware
and version, selected servers tried, known failure modes, log location confirmation, and the next
concrete action.
