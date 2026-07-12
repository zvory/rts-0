# Phase 3 - Renderer-Neutral Presentation Frame

## Phase Status

- [x] Done.

## Depends On

- Phase 2 merged with semantic projection and backend-neutral selection/marquee behavior.

## Objective

Create and validate one frame-local least-privilege presentation object assembled by the shared
client and safe for either renderer to consume. Preserve the existing entity/fog/feedback ordering
while ensuring the model excludes mutable `GameState`, hidden authoritative variants, and backend
objects. Build it beside the existing Pixi call path; Phase 3.5 owns renderer cutover.

## Work

- Preserve the frame order explicitly: build shared interpolated/current inputs, update fog from the
  authoritative fog-source subview, then assemble the final renderer frame with the updated fog
  revisions. A backend cannot run between those stages or trigger another state/fog query.
- Grow the existing frame-local entity cache into a documented shared frame context plus a narrow
  `RendererFrame`. Keep static map presentation separately versioned so terrain does not need to be
  recopied every frame.
- Keep authoritative/current/fog-source variants in shared-client subviews used by fog, HUD,
  minimap, and diagnostics only. The renderer submodel includes only renderable received entities,
  explicit remembered/reveal views, and already-resolved feedback needed to draw the current frame.
- Freeze and use the plan-locked semantic layer ids/order; later backends may implement their
  ordering but may not introduce a competing cross-backend enum. Include plain renderer data with
  narrow submodels for:
  - visual-clock sample and interpolation metadata;
  - renderable interpolated entity presentation;
  - remembered buildings and other fog-memory presentation;
  - visual selection state, relationship/team colors, HP/progress, and presentation anchors, but
    not Phase 2 picking proxies or candidate geometry;
  - authoritative current visibility plus client-accumulated explored grids, revisions, and map/tile metadata;
  - ground decals, trenches, smoke/ability objects, shot reveals, and visual-only intel;
  - placement, range, rally, order, command-target, hover, miss, and Lab tool feedback;
  - observer/Lab map overlays and visual sample inputs; and
  - bounded diagnostics/context needed to explain fallbacks and frame cost.
- Build renderer feedback and ownership/relationship decisions once in the shared presentation
  assembly for every field admitted to RendererFrame. Do not place Babylon/Pixi nodes, textures, materials, matrices,
  cameras, mutable `GameState`, mutable `ClientIntent`, or transport objects in the frame.
- Define frame lifetime precisely. Ordinary scalar/object/array records are detached and frozen in
  development/contracts; large fog/terrain data uses the Phase 0 revisioned immutable
  `GridSnapshot` accessor with no exposed typed array and copies only into backend-owned staging.
  Backends may retain immutable snapshots by revision, and Phase 5 pins the required revision.
- Update the client architecture checker and focused contracts to prevent the new presentation
  area from importing transport/UI/renderer internals or exposing mutable state.

## Expected Touch Points

- `client/src/frame_entity_views.js`
- a focused presentation-frame/model area or module
- `client/src/frame_recovery.js`
- `client/src/renderer/feedback_view_model.js` as a pure assembly input
- `client/src/match.js`/`client/src/frame_recovery.js` only to build the sidecar model once per frame
- frame, immutable-grid, renderer feedback, replay/Lab reset, and architecture contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-3.md` status update in the implementation commit

## Presentation Boundary Requirements

- One shared assembly pass per rAF/fixed capture; repeated backend calls do not query or consume
  additional state.
- Renderer data is plain, least-privilege, and frame-local. A backend may retain stable ids and its
  own visual instance state and retain immutable grid snapshots by revision, but not a mutable
  shared model reference after render.
- Static map revisions and fog revisions are explicit so backends can cache safely.
- Hidden authoritative/current/fog-source variants never enter `RendererFrame` or backend
  diagnostics. Explicit received reveals and remembered views are separate typed categories.
- Phase 2 `SelectionScene` remains a shared input model outside the backend frame. A backend receives
  only visual selected state/anchors and never becomes picking authority.
- A bad source record is dropped with bounded diagnostics and cannot prevent later frame assembly.

## Explicit Exclusions

- No Babylon dependency or backend.
- No Pixi `render(frame)` cutover or destructive-read reconciliation; Phase 3.5 owns them.
- No transient event normalization/history; Phase 6 adds the first real event shape only after the
  Phase 5 playtest confirms it is needed.
- No protocol change and no client-side hidden-state reconstruction.
- No visual redesign, faction asset work, batching, or shadows.

## Implementation Checklist

- [ ] Define static map and dynamic presentation-frame schemas.
- [ ] Assemble one frame per rAF with plain entity, fog, feedback, overlay, and diagnostic data.
- [ ] Define detached records and revisioned immutable grid snapshots and enforce them in contracts.
- [ ] Add architecture and pure-data contracts, including replay/Lab/reset behavior.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/frame_entity_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/client_contracts/renderer_feedback_contracts.mjs
    node scripts/check-client-architecture.mjs
    git diff --check

The new presentation-frame suite covers one assembly per frame, least-privilege renderer data,
fog-before-final-frame ordering, detached records, grid accessor immutability/revision reuse, static/fog
revisions, replay seek, Lab reset, and rematch.

## Manual Test Focus

Run a normal Pixi match and confirm sidecar assembly diagnostics do not change visible output or
frame scheduling. Review pure replay/Lab/reset cases and least-privilege field inventories; runtime
Pixi cutover remains Phase 3.5.

## Handoff Expectations

Document the final shared-context/renderer/static-map shapes, detached/immutable-grid lifetime, fog
ordering, layer descriptors, excluded hidden fields, and architecture enforcement. Name Phase 3.5
as next and identify Pixi legacy reads, one-shot/destructive consumption, shared UI consumers, soft
render errors, capture seam, and cutover equivalence.
