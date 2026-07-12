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

- Split visual time explicitly. `liveVisualClock` continues to timestamp/admit/expire incoming
  snapshots and events, while `capturePlaybackClock` is passed only to detached capture rendering;
  capture never assigns synthetic time to live `GameState` or the event store. Network, audio,
  browser timers, and global `performance.now()` remain real-time systems.
- Preserve the existing `capture-fixed` simulation-timeline command and its authoritative tick-
  stepping semantics. Add a distinct `capture-event` command that suspends ordinary rAF, never calls
  Lab `time.step`, renders one frozen presentation revision at explicit offsets, and restores normal
  ownership afterward.
- Define backend-neutral readiness, enter, fixed-frame render/present, diagnostics, and exit hooks.
  Pixi implements the same lifecycle the later Babylon backend receives; no backend object crosses
  the Lab bridge.
- At capture entry, create a detached bounded snapshot of ordinary records and pin the selected
  immutable grid, static-map, ground-decal, remembered-state, overlay, and asset-generation
  revisions. Incoming snapshots may
  continue updating live client state, but capture renders only the detached revision; seek/reset/
  map/asset generation change/destroy aborts rather than mixing generations.
- Close the Pixi compatibility escape hatch for capture-relevant paths: event capture must render
  entirely from the detached snapshot and capture-owned/pinned presentation resources. Add a spy
  backend contract that fails on live `GameState`, `ClientIntent`, `Fog`, event-store, or destructive
  decal reads during `capture-event`.
- Add a bounded history of Phase 4 normalized, real, already-received events. Retain at most 256
  events and at most 10 seconds of the current visual timeline, evicting anything beyond either
  bound; history stores no future-state lookup and clears on seek/reset/destroy.
- Add the launch-gated Lab Interact `capture-event` operation selecting a retained event by stable safe diagnostic id
  or descriptor. Preserve pose/payload/seed and rebase only visual start onto the capture clock; do
  not accept arbitrary hidden ids/positions or fabricate an event.
- Use the normalized 240 ms `attack`/muzzle-feedback event as the required short-event fixture.
  Accept a sorted strictly monotonic sequence of non-negative visual offsets and prove at least
  `0`, `80`, `160`, and `240` ms for that fixture.
  Capture a pre-effect baseline as a separate event-disabled frame/session, not as negative time;
  reinitialize the detached replay state when a caller requests a new non-monotonic sequence.
- Make repeated sequences deterministic for the same detached revision, event seed, viewport, DPR,
  backend settings, and offsets. Prefer exact underlying event/frame diagnostics when browser PNG
  raster hashes are not portable.
- Handle timeout, missing event, invalid/duplicate/decreasing offset, revision generation change,
  readiness failure, render exception, cancel, and match destruction through one idempotent cleanup
  path restoring ordinary rAF and releasing replay-only state.
- Record backend, presentation/static/decal/asset revisions and generation, event id/kind, offsets,
  live/capture clock samples, unchanged authoritative tick, viewport, DPR, and readiness/error
  diagnostics in capture metadata.
- Generalize Lab Interact's `open` contract and launch URL to accept an explicit
  `backend: "pixi"|"babylon"` while defaulting to Pixi, record it in status/capture manifests, and
  launch the private server with the selected worktree's `RTS_CLIENT_DIR`. Remove Pixi-only
  readiness/error wording and update the skill/help wording to be
  backend-neutral; Phase 5 exercises Pixi, and Phase 6.5 becomes the first Babylon use.
- Use the `lab-interact` skill to capture the real Pixi attack/muzzle event at the fixed offsets and
  inspect one artifact once under `target/lab-interact/`; never commit capture bytes.

## Expected Touch Points

- `client/src/visual_clock.js`
- `client/src/match_fixed_capture.js`
- `client/src/frame_recovery.js`
- Phase 3 frame snapshot and Phase 4 event store
- `client/src/lab_interact_bridge.js`
- Pixi capture-readiness adapter
- `scripts/lab-interact/fixed_capture.mjs`
- a distinct Lab Interact event-capture helper/command
- `scripts/lab-interact/driver.mjs` and CLI help/validation
- `.agents/skills/lab-interact/SKILL.md`
- `tests/client_contracts/presentation_capture_contracts.mjs`
- `tests/lab_interact_event_capture_contracts.mjs` (create it in this phase)
- existing Lab Interact fixed-capture/driver/artifact contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-5.md` status update in the implementation commit

## Safety Requirements

- Only actually received normalized events enter history or replay.
- Capture data is a detached revision; live snapshot mutation cannot change an in-progress sequence.
- Live state/event admission always uses `liveVisualClock`; synthetic offsets reach detached capture only.
- Rebase visual time only. Preserve authorized pose, payload, seed, kind, layer, and finite lifetime.
- Never start a second rAF/engine loop or patch global time.
- Exit, abort, and destroy are idempotent, including destruction during an in-flight capture.

## Explicit Exclusions

- No Babylon backend or particle implementation.
- No looping showcase effect, arbitrary effect authoring, or extended gameplay TTL.
- No attempt to freeze audio/network/browser time and no committed PNG/video.

## Implementation Checklist

- [ ] Generalize backend-neutral fixed-capture hooks.
- [ ] Separate live and capture playback clocks and preserve incoming snapshot/event timing.
- [ ] Freeze a detached renderer-frame revision with generation-change abort behavior.
- [ ] Add distinct no-tick-step `capture-event` and capture-purity spy coverage.
- [ ] Add the 256-event/10-second real-event history and safe retained-event selection.
- [ ] Enforce monotonic offsets and separate pre-effect baseline capture.
- [ ] Make Lab Interact backend selection explicit and backend-neutral while preserving Pixi default.
- [ ] Cover deterministic diagnostics, errors, cancellation, teardown, and rAF restoration.
- [ ] Capture/inspect one real short event with Lab Interact.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/presentation_capture_contracts.mjs
    node tests/lab_interact_fixed_capture_contracts.mjs
    node tests/lab_interact_event_capture_contracts.mjs
    node tests/lab_interact_driver_contracts.mjs
    node tests/client_contracts/lab_interact_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Trigger one real visible attack/muzzle event, event-capture a separate pre-effect baseline and
`0/80/160/240` ms offsets, and confirm normal expiry. Let live snapshots arrive during one capture,
cancel another, force missing-event/readiness and seek/reset abort cases, resume play, then leave/
re-enter twice and confirm normal effects and frame scheduling remain healthy.

## Handoff Expectations

Report both clock roles, detached/pinned revision policy, event-history bounds, selected event/id
policy, unchanged authoritative tick, tested offsets, deterministic evidence, abort/cleanup results,
and absolute inspected artifact path. Name Phase 6 as next and call out pre-join dependency loading,
transactional synchronous `START`, runtime provenance/integrity, and default static/network absence.
