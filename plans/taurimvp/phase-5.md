# Phase 5 - Final Manual Gate

## Phase Status

- [ ] Planned.

## Plain-Language Summary

This phase is intentionally delayed until someone can test on macOS. Use the built unsigned app
from Phase 4 and run the real startup, server-selection, local-URL, and native-cursor gameplay
checks. The output should be a clear ship or stop decision for sending the MVP shell to playtesters.

## Objective

Perform the final human gate after the technical shipping prep is complete.

## Scope

- Use the unsigned artifact from Phase 4, not `cargo run`.
- Verify startup selection for beta, mainline, a custom server URL, and a local URL pointing at a
  separately running repo server.
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
- Update `docs/tauri-retrospective.md`, `plans/maccursor/phase-4.md`, or this phase with the final
  go/no-go result.

## Expected Touch Points

- Tauri shell/native cursor files for small fixes
- startup UI files for small fixes
- `desktop/maccursor-shell/README.md`
- `docs/tauri-retrospective.md`
- `plans/maccursor/phase-4.md`
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

Use the app as a player would: pick a server, join or create a match, enable native cursor mode, and
play long enough to judge latency and cleanup reliability. For the local URL path, start the repo
server separately before opening the shell.

## Handoff Expectations

The handoff must include a ship/stop recommendation, tested artifact id/SHA, tested macOS hardware
and version, selected servers tried, known failure modes, log location confirmation, and the next
concrete action.
