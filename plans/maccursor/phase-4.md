# Phase 4 - In-Game Plausibility Gate

## Phase Status

- [ ] Planned.

## Plain-Language Summary

Use the desktop shell and native cursor backend in real local gameplay and decide whether this path
is worth continuing. The test is about felt mouse latency first, then correctness of selection,
commands, HUD, minimap, and cleanup. If the cursor still feels delayed, or if capture cleanup is
unreliable, stop and document the failure instead of polishing the shell.

## Objective

Produce a go/no-go decision for continuing the macOS native cursor desktop app effort.

## Scope

- Run the native cursor path in at least one local sandbox and one AI or two-client match flow.
- Compare against normal browser Pointer Lock on the same Mac when possible.
- Exercise cursor-heavy workflows: short flicks, edge pan, selection boxes, right-click move/attack,
  minimap interaction, HUD/command-card clicks, placement if available, and Escape unlock.
- Add or refine diagnostics only enough to explain the result.
- Update plan/docs with the decision and any required next plan if the spike succeeds.
- Do not add packaging polish, signing, notarization, updater work, or cross-platform backends in
  this phase.

## Non-Negotiable Latency Rules

- Do not explain away cursor delay as render or gameplay lag if the player-visible cursor lags.
- Do not switch to a frame-coalesced cursor path to make diagnostics easier.
- Do not mark the phase successful unless the manual test says the cursor feels materially better
  than browser Pointer Lock on the available Mac.

## Expected Touch Points

- Tauri shell/native cursor backend files for small fixes found during playtest.
- `client/src/input/*` for small routing or cleanup fixes found during playtest.
- `docs/tauri-retrospective.md` or a new desktop-spike note if the spike result changes product
  guidance.
- This plan and phase file to record the final decision.

Avoid touching:

- simulation rules
- protocol contracts
- balance values
- unrelated UI polish

## Verification

- Re-run the targeted tests from Phase 3 after any code changes.
- Run `node scripts/check-client-architecture.mjs` after any `client/src/` changes.
- Run focused shell/native checks added by earlier phases.
- Confirm `git status --short` contains only files belonging to the spike.

## Manual Testing Focus

Play locally with the native cursor path for several minutes. The cursor should feel immediate
during fast movement, short precise movements, HUD/minimap interactions, edge panning, and after
brief render stress; Escape, Command-Tab, and window close must always restore normal cursor control.

## Handoff Expectations

The handoff must include a clear go/no-go recommendation, the observed cursor latency quality, known
failure modes, cleanup reliability, whether Tauri should remain the shell, and the next concrete
plan if the spike should continue.
