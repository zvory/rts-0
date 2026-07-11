# Phase 1.5 - Navigation and Minimap Migration

## Phase Status

- [x] Done.

## Depends On

- Phase 1 merged with the semantic camera/projection core and compatibility edge.

## Objective

Move camera navigation and minimap behavior onto semantic operations while leaving Pixi behavior
unchanged. Prove the highest-frequency gesture and viewport-footprint consumers before migrating
app-shell/audio/Lab surfaces. Keep the raw-read ratchet permissive only for consumers explicitly
owned by Phase 1.75.

## Work

- Migrate `CameraNavigationInput`, camera controls, and replay camera input from raw zoom read-
  modify-write to semantic pan and anchor-aware zoom/dolly operations, preserving wheel, pinch,
  edge, keyboard, middle/space drag, pointer-lock, and touch gestures.
- Make the minimap draw the semantic viewport ground polygon and use semantic recentering. Under
  Pixi the polygon must remain materially identical to the current rectangle.
- Keep input and minimap coordinates in viewport-local CSS pixels under resize and non-1 DPR.
- Add temporary architecture allowlist entries for the exact Phase 1.75 consumers still reading raw
  camera representation; do not add a broad directory exemption.
- Update durable docs/ledger with migrated navigation/minimap surfaces and remaining allowlist.

## Expected Touch Points

- `client/src/input/camera_navigation.js`
- `client/src/input/camera_controls.js`
- `client/src/replay_camera_input.js`
- `client/src/minimap.js`
- camera/minimap/input and architecture contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-1.5.md` status update in the implementation commit

## Behavioral Requirements

- Current Pixi gestures, anchoring, clamping, recenter, and viewport footprint remain equivalent.
- Public inputs/minimap use CSS pixels and semantic operations only.
- Empty/partial perspective ground polygons render safely and do not fabricate bounds.

## Explicit Exclusions

- No audio, control-group, App/Match carryover, Lab, profile, observer, or diagnostic migration; Phase 1.75 owns them.
- No selection rewrite, Babylon dependency, perspective visual change, free orbit, protocol, or replay-format change.

## Implementation Checklist

- [x] Migrate live/replay navigation and camera controls.
- [x] Migrate minimap footprint/recenter to semantic polygon/focus.
- [x] Add focused equivalence/resize/DPR tests and bounded temporary allowlist.
- [x] Update durable docs/ledger and mark this phase done.

## Verification

    node tests/client_contracts/camera_projection_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/client_contracts/match_replay_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Exercise keyboard/edge/middle/space/touch pan, wheel/pinch zoom, pointer lock, minimap recenter/drag,
resize, and replay navigation. Confirm the Pixi viewport polygon remains the same rectangle and
gesture behavior does not change.

## Handoff Expectations

List migrated gestures/minimap behavior, equivalence results, and the exact temporary raw-read
allowlist. Name Phase 1.75 as next and identify audio, control groups, app/replay carryover,
visual profiles, Lab manifests, observer/diagnostics, and final ratchet closure.
