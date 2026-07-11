# Phase 1 - Semantic Camera and Projection Contract

## Phase Status

- [ ] Not started.

## Depends On

- Phase 0 merged and its rendering contract/parity ledger available from `origin/main`.

## Objective

Replace shared-client dependence on an orthographic state representation with renderer-neutral
camera meanings while leaving the Pixi view and controls behaviorally unchanged. The contract must
support a later fixed-perspective implementation without exposing Babylon objects or asking
consumers to infer geometry from `x`, `y`, `zoom`, `viewW`, or `viewH`. Complete this migration
before the backend factory or production Babylon camera is introduced.

## Work

- Implement the Phase 0 semantic contract with plain data and operations for:
  - nullable screen-to-ground projection;
  - world-point projection including screen position, depth, and clip/visibility state;
  - projected world-extent/screen-scale queries for constant-screen-size labels and overlays;
  - viewport ground polygon and derived conservative world bounds;
  - projected containment, point-cluster evaluation, and fit/focus operations;
  - update, center/focus, screen-delta pan, and anchor-aware zoom/dolly-by-factor;
  - map/viewport bounds and resize;
  - semantic snapshot/restore and diagnostics; and
  - an audio listener model containing world focus and reference distance without exposing zoom.
- Preserve the current world-pixel focus and zoom setting as the orthographic implementation's
  private state. Legacy persisted `{x,y,zoom}` input may be accepted at a compatibility edge, but
  new shared consumers and diagnostics use the semantic snapshot.
- Migrate `CameraNavigationInput` and camera controls away from read-modify-write access to raw
  zoom. Wheel, pinch, edge scroll, keyboard, middle/space drag, pointer lock, and minimap focus
  must call semantic navigation operations.
- Make the minimap render the camera's ground polygon rather than constructing an axis-aligned
  rectangle from top-left and zoom. Its click/drag recenter and command behavior remain unchanged.
- Change the frame/audio seam so audio receives the camera's listener model and never derives
  spatial reference distance from a nominal perspective zoom.
- Move match viewport alerts, control-group cluster framing, Lab camera focus/inspection,
  replay/camera carryover, observer overlays, visual samples, and profiler/diagnostic surfaces to
  semantic bounds, projection, snapshot, or listener operations as appropriate.
- Make control-group framing a semantic camera operation that can evaluate and focus a set of world
  points using the current view. Phase 2 may change which projected candidates count as currently
  visible, but it must not reimplement camera-size or focus math.
- Define every public screen coordinate and extent in viewport-local CSS pixels. Device-pixel ratio,
  canvas backing dimensions, Babylon hardware scaling, DOM bounds, and render-buffer conversion stay
  inside adapters so input, minimap, Lab, and overlays use one unit at non-1 DPR and after resize.
- Keep raw orthographic transform reads private to the current camera implementation and a clearly
  named Pixi compatibility edge. Add an architecture or focused contract ratchet preventing new
  shared input/UI/app-shell consumers from reintroducing raw representation reads.
- Update the durable design contract and parity ledger with actual method names, compatibility
  boundaries, and evidence.

## Expected Touch Points

- `client/src/camera.js` and small focused projection/view helpers
- `client/src/input/camera_navigation.js`
- `client/src/input/camera_controls.js`
- `client/src/input/control_groups.js`
- `client/src/frame_recovery.js`
- `client/src/audio.js`
- `client/src/minimap.js`
- `client/src/match.js`
- `client/src/camera_view_selection.js`
- `client/src/lab_interact_bridge.js`
- `client/src/frame_profiler.js`
- `client/src/renderer/observer_map_analysis.js`
- `client/src/renderer/visual_samples.js`
- focused camera, audio, minimap, Lab, and architecture contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-1.md` status update in the implementation commit

## Contract Requirements

- `groundAtScreen` or its final equivalent may return no hit; even though the orthographic adapter
  always hits, callers must not encode that assumption.
- World projection reports depth/clip visibility. A bare `{x,y}` result is insufficient for later
  perspective selection and overlays.
- `viewportGroundPolygon` remains a polygon in public semantics. Consumers may use its conservative
  AABB only when false-positive inclusion is explicitly acceptable.
- The perspective contract clips the ground/frustum intersection to map bounds, returns deduplicated
  points in stable clockwise world-`(x,y)` winding, returns an empty polygon when no bounded ground
  is visible, and returns no conservative bounds for an empty polygon. Partial corner-ray misses do
  not become fabricated far-away points.
- Constant-screen-size labels and selection/HP presentation use the projected-extent/overlay-sizing
  operation or a screen projection layer; they never derive size from nominal zoom.
- Snapshot/restore describes player intent in world terms and remains stable across backends; it
  does not serialize Babylon matrices or engine nodes.
- Perspective pitch, yaw, height, and FOV may be configuration in a later adapter, but are not
  player-orbit controls in this plan.
- The Pixi renderer may consume a private orthographic transform descriptor temporarily; no other
  shared feature may use that descriptor.

## Explicit Exclusions

- No Babylon dependency, backend, scene, engine, ray class, or matrix object.
- No perspective visual change and no free orbit.
- No selection behavior rewrite; Phase 2 owns entity picking and marquee semantics.
- No protocol or replay-format change. Stop if semantic camera carryover truly requires a wire
  change rather than a client-only compatibility adapter.

## Implementation Checklist

- [ ] Implement the semantic camera/projection interface with orthographic equivalence.
- [ ] Migrate navigation, minimap, audio, viewport, control-group, Lab, carryover, and diagnostics consumers.
- [ ] Confine raw orthographic state to the camera/Pixi compatibility edge.
- [ ] Add round-trip, clipping, polygon/bounds, pan, clamp, zoom-anchor, fit, snapshot, and listener tests.
- [ ] Add a ratchet against new raw camera-representation consumers.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/camera_fog_contracts.mjs
    node tests/client_contracts/camera_projection_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/lab_interact_driver_contracts.mjs
    node scripts/check-client-architecture.mjs
    node tests/select-suites.mjs --verify
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

In a normal Pixi match, exercise keyboard/edge/middle/space/touch pan, wheel/pinch zoom, pointer
lock, minimap recenter/drag, resize, and camera carryover across replay/rematch. Double-tap a control
group, jump through the minimap while spatial sounds are active, verify viewport alert suppression,
and use Lab single/multi-entity focus plus camera inspection. Confirm the viewport polygon renders
as the same rectangle under Pixi and no visible camera behavior changed.

## Handoff Expectations

List the final semantic method names, the remaining private Pixi compatibility reads, the ratchet
coverage, and any legacy restore edge. Name Phase 2 as next and call out near/far perspective
selection, empty ground-ray hits, marquee edges, control-group selection, and Lab box tools as its
manual test focus.
