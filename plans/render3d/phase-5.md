# Phase 5 - Deterministic Capture and Retained Event Replay

## Phase Status

- [ ] Not started.

## Depends On

- Phase 4 merged with immutable spatially self-contained normalized presentation events.

## Objective

Make realistic short-lived effects reviewable without changing their gameplay lifetime. Generalize
the existing fixed visual-clock capture seam for any backend, freeze one detached presentation
revision, and replay a bounded real received event at monotonic non-negative offsets. Prove
deterministic timing, cancellation, revision isolation, and cleanup before Babylon visual evidence
depends on it.

## Work

- Preserve the rule that fixed capture suspends the ordinary `Match` rAF, replaces only the
  injected visual clock, renders explicit frames, and restores normal ownership afterward. Network,
  audio, browser timers, and global `performance.now()` remain real-time systems.
- Define backend-neutral readiness, enter, fixed-frame render/present, diagnostics, and exit hooks.
  Pixi implements the same lifecycle the later Babylon backend receives; no backend object crosses
  the Lab bridge.
- At capture entry, create a detached bounded snapshot of the selected `RendererFrame` revision,
  including copied fog grids/revisioned large data needed by the capture. Incoming snapshots may
  continue updating live client state, but capture renders only the detached revision; seek/reset/
  destroy aborts rather than mixing generations.
- Add a bounded history of Phase 4 normalized, real, already-received events. History is scoped to
  the current timeline generation, has explicit count/time bounds, stores no future-state lookup,
  and clears on seek/reset/destroy.
- Add a launch-gated Lab Interact operation selecting a retained event by stable safe diagnostic id
  or descriptor. Preserve pose/payload/seed and rebase only visual start onto the capture clock; do
  not accept arbitrary hidden ids/positions or fabricate an event.
- Accept a sorted strictly monotonic sequence of non-negative visual offsets for an effect replay.
  Capture a pre-effect baseline as a separate event-disabled frame/session, not as negative time;
  reinitialize the detached replay state when a caller requests a new non-monotonic sequence.
- Make repeated sequences deterministic for the same detached revision, event seed, viewport, DPR,
  backend settings, and offsets. Prefer exact underlying event/frame diagnostics when browser PNG
  raster hashes are not portable.
- Handle timeout, missing event, invalid/duplicate/decreasing offset, revision generation change,
  readiness failure, render exception, cancel, and match destruction through one idempotent cleanup
  path restoring ordinary rAF and releasing replay-only state.
- Record backend, presentation revision/generation, event id/kind, offsets, viewport, DPR, and
  readiness/error diagnostics in capture metadata.
- Use the `lab-interact` skill to capture a real Pixi short event at multiple fixed offsets and
  inspect one artifact once under `target/lab-interact/`; never commit capture bytes.

## Expected Touch Points

- `client/src/visual_clock.js`
- `client/src/match_fixed_capture.js`
- `client/src/frame_recovery.js`
- Phase 3 frame snapshot and Phase 4 event store
- `client/src/lab_interact_bridge.js`
- Pixi capture-readiness adapter
- `scripts/lab-interact/fixed_capture.mjs`
- `scripts/lab-interact/driver.mjs` and CLI help/validation
- `tests/client_contracts/presentation_capture_contracts.mjs`
- existing Lab Interact fixed-capture/driver/artifact contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-5.md` status update in the implementation commit

## Safety Requirements

- Only actually received normalized events enter history or replay.
- Capture data is a detached revision; live snapshot mutation cannot change an in-progress sequence.
- Rebase visual time only. Preserve authorized pose, payload, seed, kind, layer, and finite lifetime.
- Never start a second rAF/engine loop or patch global time.
- Exit, abort, and destroy are idempotent, including destruction during an in-flight capture.

## Explicit Exclusions

- No Babylon backend or particle implementation.
- No looping showcase effect, arbitrary effect authoring, or extended gameplay TTL.
- No attempt to freeze audio/network/browser time and no committed PNG/video.

## Implementation Checklist

- [ ] Generalize backend-neutral fixed-capture hooks.
- [ ] Freeze a detached renderer-frame revision with generation-change abort behavior.
- [ ] Add bounded real-event history and safe retained-event selection.
- [ ] Enforce monotonic offsets and separate pre-effect baseline capture.
- [ ] Cover deterministic diagnostics, errors, cancellation, teardown, and rAF restoration.
- [ ] Capture/inspect one real short event with Lab Interact.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/presentation_capture_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node tests/lab_interact_driver_contracts.mjs
    node tests/client_contracts/lab_interact_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Trigger one real visible short event, fixed-capture a separate pre-effect baseline and monotonic
start/mid/end offsets, and confirm normal expiry. Let live snapshots arrive during one capture,
cancel another, force missing-event/readiness and seek/reset abort cases, resume play, then leave/
re-enter twice and confirm normal effects and frame scheduling remain healthy.

## Handoff Expectations

Report the detached revision policy, event-history bounds, selected event/id policy, tested offsets,
deterministic evidence, abort/cleanup results, and absolute inspected artifact path. Name Phase 6 as
next and call out pre-join dependency loading, synchronous `START`, default static/network absence,
one-rAF rendering, unsupported graphics, and repeated teardown as its focus.
