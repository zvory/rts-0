# Phase 1.75 - Shared Camera Consumer Closure

## Phase Status

- [x] Done.

## Depends On

- Phase 1.5 merged with semantic navigation/minimap and a bounded temporary raw-read allowlist.

## Objective

Close every remaining shared raw camera consumer and enforce the final representation ratchet.
Migrate audio, control groups, viewport/app-shell, Lab, carryover, profiles, observer, and
diagnostic surfaces without changing Pixi behavior. Finish the complete camera contract before
selection semantics change.

## Work

- Change spatial audio to consume the Phase 0 listener model and frozen perspective reference-
  distance formula rather than derive behavior from zoom.
- Move viewport alerts and control-group containment/framing to semantic projection/bounds/fit.
  Phase 2 changes candidate admission, not camera fit math.
- Migrate `Match`/`App`/`ReplayViewer` carryover, branch/freeze recovery, visual profiles, and legacy
  restore through the versioned semantic snapshot. Accept `{x,y,zoom}` at one compatibility edge
  and immediately normalize it.
- Update Lab bridge/driver/status/capture manifests and focus/inspection to report/consume the
  versioned semantic snapshot rather than raw representation.
- Move observer overlays, visual samples, frame recovery, profiler, and diagnostics to semantic
  projection, listener, viewport, or snapshot operations.
- Replace the temporary allowlist with a ratchet that rejects raw `x/y/zoom/viewW/viewH` reads
  outside `camera.js` and the named Pixi compatibility adapter. List every remaining private Pixi
  read with its owner; no app/input/UI/Lab exception remains.
- Update durable docs/ledger with migrated consumers, snapshot version, audio formula, and ratchet.

## Expected Touch Points

- `client/src/audio.js`
- `client/src/input/control_groups.js`
- `client/src/frame_recovery.js`
- `client/src/match.js`, `client/src/app.js`, and `client/src/replay_viewer.js`
- `client/src/visual_profiles.js` and `client/src/camera_view_selection.js`
- `client/src/lab_interact_bridge.js` plus Lab driver/status/capture manifest contracts
- `client/src/frame_profiler.js`
- observer/visual-sample/viewport-alert consumers
- camera, audio, replay, Lab, observer, and architecture contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-1.75.md` status update in the implementation commit

## Behavioral Requirements

- Audio attenuation, framing, viewport alerts, carryover, profiles, Lab, and diagnostics remain
  materially equivalent under Pixi.
- Versioned semantic snapshots are the only public carryover/tooling representation.
- No raw camera read remains outside the camera and named Pixi adapter.

## Explicit Exclusions

- No new projection semantics, selection/marquee rewrite, Babylon dependency, perspective visual
  change, or protocol/replay-format change.

## Implementation Checklist

- [x] Migrate audio, viewport alerts, and control groups.
- [x] Migrate app/replay/profile/carryover and Lab manifests/focus.
- [x] Migrate observer/visual-sample/profiler/diagnostic consumers.
- [x] Close the raw-read ratchet with no shared consumer exception.
- [x] Update durable docs/ledger and mark this phase done.

## Verification

    node tests/client_contracts/camera_projection_contracts.mjs
    node tests/client_contracts/audio_contracts.mjs
    node tests/client_contracts/match_replay_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/lab_interact_driver_contracts.mjs
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Test spatial sounds, viewport alerts, control-group focus/containment, replay/rematch/branch carryover,
visual-profile initial views, Lab single/multi focus and manifests, observer overlays, resize, and
diagnostics. Confirm no visible Pixi behavior changed.

## Handoff Expectations

List migrated consumers, versioned snapshot/audio formula, final ratchet/remaining private Pixi
reads, and equivalence results. Name Phase 2 as next and call out last-presented SelectionScene,
near/far/elevated proxy picking, nullable ground hits, marquee, and Lab box tools.
