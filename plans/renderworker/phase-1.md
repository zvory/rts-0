# Phase 1 - Asynchronous Presentation Contract

## Phase Status

- [ ] Ready.

## Objective

Remove the assumption that returning from a render call means the requested pixels are already on
screen. Introduce an explicit presentation lifecycle keyed by generation and frame id, use it for
selection, durable ground decals, diagnostics, capture, and teardown, and adapt the existing
main-thread Pixi path to the new contract without changing when or what it draws.

This phase must not create a worker, transfer a canvas, add a runtime option, or change visual
output. Its value is making the asynchronous truth testable while failures are still easy to
attribute.

## Entry Gate

- Start from current `origin/main` in a clean worktree.
- Preserve the detached `PresentationFrameV1`, semantic camera, one Match-owned RAF, and current
  Pixi rendering behavior.
- Retain the latest canonical `supply-300-hellhole-stream` baseline location and headline numbers in
  the phase handoff for comparison after Phase 3.

## Presentation Lifecycle

- Replace the Match-facing synchronous boolean with one submission result carrying the submitted
  generation/frame id and asynchronous outcome channels. Use a small explicit vocabulary:
  `retained`, `presented`, `superseded`, `failed`, and `destroyed`; do not infer state from a timeout
  or private render counter.
- `retained` means the renderer owns the durable ground-decal revision carried by that submission.
  It does not mean pixels were displayed. A renderer that fails after retaining decals must still
  report the retained revision so `GameState` does not resend and double-stamp it.
- `presented` means the canvas displays that exact generation/frame id. It is the only result that
  advances the displayed-frame counter, publishes the matching `SelectionSceneV1`, or resolves a
  fixed capture.
- `superseded` means that frame will never be displayed because a newer accepted frame replaced it.
  Discard its selection scene, do not acknowledge it as presented, and retain any separately
  acknowledged durable decal update.
- `failed` is bounded to the submitted frame and includes a useful error; `destroyed` settles all
  outstanding work during teardown without later selection/decal/capture side effects.
- Ignore duplicate or stale acknowledgments and never let an older generation/frame id replace the
  most recently presented selection scene. Treat impossible ordering, unknown frame ids, and a
  presented-after-destroy message as bounded renderer protocol errors.

## Frame, Input, and Decal Integration

- Add one main-thread presentation coordinator at the current `frame_recovery.js` seam. It owns
  pending frame metadata and connects renderer outcomes to selection publication, game-state decal
  acknowledgment, frame health, and diagnostics; those consumers must not each implement their own
  message ordering.
- Continue assembling current state on every Match RAF and continue updating HUD, minimap, audio,
  input, and health on their existing owners. Do not make the RAF callback await presentation.
- Keep `SelectionSceneV1` on the main thread. Store it only until the corresponding presentation
  result and publish it exactly when that frame becomes the newest displayed frame.
- Give reconciled ground-decal batches a monotonic generation/revision or batch id. Acknowledge only
  the exact retained revision rather than clearing an unqualified current queue.
- Preserve the last successfully displayed selection scene through a failed or superseded frame.
  Teardown clears pending scenes and prevents late completion callbacks from reaching a new match.

## Fixed Capture and Measurement

- Make `renderFixedCaptureFrame` asynchronous and resolve it only when the requested generation and
  frame id is presented. Return the public acknowledged frame id instead of reading
  `renderer._renderFrameCount`.
- Update deterministic capture, parity, Interact/Lab capture, and any browser helpers to await the
  capture promise before reading PNG pixels or readiness.
- Keep the ordinary main-thread Pixi adapter synchronous internally in this phase, but deliver its
  lifecycle results through the same coordinator used by the future worker. Tests must prove that
  the immediate implementation does not accidentally re-enter Match or publish twice.
- Separate submitted, retained, presented, superseded, and failed counters. Preserve existing
  `match.renderer`, `renderer.update`, and `renderer.present` measurements until Phase 3 moves the
  update/present work to the worker.

## Expected Touch Points

- `client/src/frame_recovery.js`
- `client/src/match_fixed_capture.js`
- `client/src/match.js`
- `client/src/state.js` ground-decal reconciliation/acknowledgment seam
- a small coordinator/result module under `client/src/presentation/`
- `client/src/renderer/pixi_compatibility_adapter.js`
- `client/src/frame_profiler.js` and client performance report surfaces
- `scripts/client-render-parity.mjs` and fixed-capture helpers
- focused presentation, Pixi adapter, selection, decal, capture, replay, and teardown contracts
- `docs/design/client-rendering.md`
- `docs/design/rendering-parity.md` if capability/evidence wording changes

## Verification

- Focused coordinator contracts covering immediate success, retain-then-fail, supersede, stale or
  duplicate outcome, generation reset, and destroy with pending work.
- Existing Pixi presentation-adapter and renderer contracts.
- Focused `frame_recovery`, selection publication, ground-decal retry, fixed-capture, replay, and
  Match teardown contracts.
- `node scripts/client-render-parity.mjs --baseline-worktree <origin-main-baseline> --candidate-worktree <phase-worktree> --samples 16 --seed renderworker-phase-1`
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

The parity run must have ready assets and exact decoded-RGBA equality at every sampled tick. This
phase cannot accept a visual difference because it is only changing completion semantics.

## Manual Test Focus

Play one normal local match through selection, movement, combat, deaths that create persistent
decals, and leave/re-enter. Run one deterministic capture sequence and confirm that repeated ticks
change normally while the requested capture tick and displayed selection remain aligned.

## Completion and Handoff Expectations

Mark this phase done in its implementation commit. Lead the handoff with the new lifecycle shape,
name the owner of pending frame metadata, explain exactly when selection and decals advance, provide
the exact parity result and focused checks, and carry the current-main Hellhole baseline into Phase
2. State explicitly that no worker, worker flag, fallback, or second renderer path was added.
