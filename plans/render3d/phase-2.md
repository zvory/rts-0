# Phase 2 - Perspective-Safe Picking and Marquee Selection

## Phase Status

- [ ] Not started.

## Depends On

- Phase 1.75 merged with the semantic camera/projection contract and closed raw-consumer ratchet.

## Objective

Make ground interaction and entity selection correct for both orthographic and elevated
perspective cameras before the Babylon backend is present. Commands, placement, and map tools use a
nullable ground hit; entity click and box selection use projected renderer-neutral selection
proxies and the real screen marquee. Asset geometry, Babylon meshes, LODs, shadow proxies, and
placeholder art must never become selection authority.

## Work

- Split the current `_worldAt` assumption into explicit nullable ground interaction used by move,
  attack-ground/ability targets, placement, hover previews, Lab paint/spawn tools, and other
  commands that land on the authoritative plane. A missed/behind-camera/horizon result must cancel
  or leave the interaction armed according to existing intent semantics without emitting invalid
  coordinates. Use the projection snapshot in the last presented SelectionScene, not a newer
  camera transform awaiting presentation.
- Define plain-data selection proxies from entity kind, authoritative footprint/size, facing,
  setup state where relevant, and interpolated presentation position. The proxy may include ground
  polygons, elevated anchor points, or screen-radius policy, but cannot depend on render meshes.
- Introduce a detached fog-filtered `SelectionScene` containing candidate ids/proxies and the
  semantic camera projection snapshot from the last successfully presented frame. Publish it only
  after that frame renders; input keeps the previous scene across a render failure.
- Make every click, hover, entity-target, ctrl-in-viewport, control-group admission, and marquee
  calculation read the last presented `SelectionScene`. Do not query fresh
  `entitiesInterpolated(1)`, current mutable state, or a camera transform newer than the pixels the
  player is targeting.
- Route every entity-targeting interaction through the same projected proxy picker: ordinary click
  selection, right-click attack/gather/repair classification, hover and command previews, armed
  entity-target abilities, and Lab entity-click tools. Reserve `groundAtScreen()` for commands and
  tools whose semantic target is truly a point on the authoritative ground plane.
- Project candidate proxies through the semantic camera, reject clipped/behind-camera candidates,
  and implement deterministic screen-point hit scoring. Preserve own-entity preference, stable id
  tie-breaking, current unit/building/resource selectability, oriented vehicle behavior, and
  shot-reveal/vision-only/fog-filtered exclusions.
- For overlapping click candidates, apply the interaction's existing eligibility/ownership
  preference first, then screen-space distance to the projected semantic anchor, then nearest
  positive visible depth, then stable entity id. Order drag-selected ids by screen-space distance
  from the drag start to the projected semantic anchor and then stable id; never require a ground
  ray at the drag anchor.
- Replace the world rectangle between two ground hits with actual screen-marquee intersection.
  Preserve units-over-buildings fallback, shift add/remove, ctrl/meta same-kind selection, command
  supply admission, nearest-to-drag-anchor ordering, spectator/Lab ownership policy, and mixed-owner
  Lab rules.
- Make ctrl-select-in-viewport and control-group candidate visibility use projected viewport
  containment rather than an orthographic AABB. Use conservative bounds only for candidate
  prefiltering; final admission must be screen/projection correct.
- Route Lab box remove/inspect behavior through the same selected-id calculation. If Lab
  diagnostics still expose world coverage, report the screen rect plus ground polygon/conservative
  bounds instead of claiming the two-corner world rectangle is the selection region.
- Move the visual marquee to a backend-neutral screen overlay or define it as an explicit backend
  screen-overlay operation with identical lifecycle. Input must not need a Pixi renderer type just
  to draw/clear a rectangle.
- Add pure tests using both the real orthographic adapter and a deliberately skewed fake
  perspective adapter. Include cases where the two-ground-hit rectangle would select the wrong
  entity, partially intersected large/oriented proxies, near/far candidates, clipped points, and
  nullable ground hits. Add a moving entity and camera-between-frames case proving picking follows
  the last presented proxy/camera rather than fresh state.
- Update the durable rendering contract and parity ledger with the implemented selection proxy and
  evidence.

## Expected Touch Points

- `client/src/input/selection.js`
- `client/src/input/index.js`
- `client/src/input/commands.js`
- `client/src/input/placement.js`
- `client/src/input/lab_tools.js`
- `client/src/input/control_groups.js`
- a pure selection projection/proxy helper in the input or presentation area
- a detached last-presented `SelectionScene` owner in `Match`/frame orchestration
- a backend-neutral screen overlay/marquee collaborator
- `client/src/renderer/index.js` only for Pixi compatibility extraction
- input/state/Lab/client smoke contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-2.md` status update in the implementation commit

## Behavioral Requirements

- A screen marquee selects by projected overlap, not by the ground quadrilateral or its AABB.
- Clicking a visibly elevated body may hit its semantic projected proxy even when the ground ray
  under the pointer does not intersect the base footprint.
- Depth/clip information participates in admission and deterministic tie-breaking; entities behind
  the camera cannot be selected.
- Selection candidates still come only from the client's fog-filtered authoritative/interpolated
  views. Projection must not reconstruct hidden candidates.
- Picking uses the last successfully presented `SelectionScene`; moving entities and a camera input
  awaiting the next frame cannot be targeted at an unseen future pose.
- Ground commands remain authoritative world pixels and cannot emit NaN, infinity, or an old cached
  hit after a miss.
- Orthographic Pixi behavior must remain materially equivalent, including pointer lock and touch
  navigation separation from selection.

## Explicit Exclusions

- No Babylon scene or mesh picking.
- No renderer-specific ray type in input, state, HUD, minimap, or Lab APIs.
- No change to command authority, selection budget, control policy, protocol, or server hitboxes.
- No orbit camera or terrain elevation picking.

## Implementation Checklist

- [ ] Add nullable ground-interaction handling to every world-command/tool path.
- [ ] Define and project plain-data selection proxies.
- [ ] Publish and consume the last-presented detached SelectionScene.
- [ ] Convert click, marquee, ctrl-in-viewport, and Lab box selection.
- [ ] Decouple the screen marquee from the Pixi renderer type.
- [ ] Preserve ownership, fog, preference, budgeting, and stable ordering semantics.
- [ ] Add skewed fake-perspective and Pixi regression coverage.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/state_input_contracts.mjs
    node tests/client_contracts/selection_projection_contracts.mjs
    node tests/minimap_input_contracts.mjs
    node tests/lab_interact_driver_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Under Pixi, test click and drag selection near all viewport edges, partially enclosed units and
buildings, oriented vehicles, shift/ctrl behavior, unit-over-building fallback, control groups,
pointer lock, and touch camera gestures. In Lab, test remove/inspect box tools and empty clicks.
Review the skewed fake-perspective contract diagnostics for near/far projected candidates and a
marquee whose two ground-hit corners produce a visibly different world region; no interactive fake
demo is required before the Babylon adapter exists.

## Handoff Expectations

Describe the final proxy representation, SelectionScene publication/lifetime, ground-miss behavior,
marquee owner/lifecycle, and any conservative prefilter. State that selection is independent of
asset geometry and name Phase 3 as
next, with immutable revisioned grids, least-privilege renderer data, fog update ordering, and Pixi
compatibility as its focus.
