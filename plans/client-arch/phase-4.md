# Phase 4 - Replay Controls Extraction

## Phase Status

- [ ] Pending implementation.

## Objective

Move replay and scenario speed/vision control logic out of `Match` while preserving identical DOM
structure and behavior. This phase touches UI-adjacent code, so it must be backed by programmatic DOM
contract tests.

## Work

- Add `client/src/replay_controls.js`.
- Extract only the replay/scenario controls currently embedded in `Match`:
  - showing/hiding `dom.replaySpeed`
  - speed button handling
  - seek button handling
  - scenario pause/step handling
  - replay vision button creation and selection
  - replay tick/status text
  - cleanup in `destroy()`
- Keep existing CSS classes, button text, `data-*` attributes, and hidden/class toggles unchanged.
- `Match` should construct `ReplayControls` only when replay or scenario controls are active, then
  delegate:
  - `applyReplayState(state)`
  - `destroy()`
- Add DOM contract tests using lightweight fake elements or the existing test DOM pattern:
  - speed click sends `net.setReplaySpeed`
  - seek click sends `net.seekReplay`
  - scenario step sends `net.stepDevTick`
  - replay vision single-player and multi-player selection send the same payloads as before
  - `destroy()` removes generated vision/status nodes and restores hidden states

## Implementation Segments

Mark each segment complete as it lands:

- [ ] Add `ReplayControls` and move only replay/scenario control ownership into it.
- [ ] Preserve existing DOM structure, classes, text, `data-*` attributes, and hidden/class toggles.
- [ ] Delegate replay state updates and teardown from `Match` to `ReplayControls`.
- [ ] Add DOM contract tests for speed, seek, scenario step, vision selection, and `destroy()`.
- [ ] Run verification and record any visual or browser-smoke gaps in the final handoff.

## Verification

- `node tests/client_contracts.mjs`
- `node scripts/check-client-architecture.mjs` if Phase 1 has landed
- Client smoke when practical.
- Manual visual check is optional, not the primary proof. The phase is only complete when DOM
  contract tests cover the extracted behavior.

## Manual Test Prompt

At handoff, ask the user to do this focused replay/scenario check:

> Manual testing requested, 10-20 minutes:
> 1. Open a replay or dev scenario that exposes replay controls.
> 2. Click each speed button and confirm playback speed and selected button state change as before.
> 3. Use seek/reset controls and confirm the replay jumps without leaving duplicate controls.
> 4. In a scenario, pause and step once; confirm only one tick advances.
> 5. Switch replay vision targets in both single-player and multi-player replay states if available.
> 6. Leave/re-enter the replay or scenario and report duplicated controls, stale status text,
>    broken selected states, or console errors.

## Handoff Expectations

In the final handoff, include the completed segment checklist, exact verification output summary,
and the filled manual testing prompt above. Tell the next agent to start Phase 5 only after this
phase is committed, merged to `main`, and pushed.

## Safety Notes

This phase is higher risk than Phases 1-3 because it touches UI behavior. Keep the extraction
mechanical. Do not rename CSS classes, change button labels, alter replay semantics, or redesign the
control surface.

If the extraction becomes hard to test without a browser, stop and first add a small DOM test helper
instead of finishing by visual inspection.

## Outcome

No intentional gameplay or visual change. `Match` sheds replay special-mode logic, and replay
controls become a testable collaborator.
