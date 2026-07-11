# Phase 3 - Renderer-Neutral Presentation Frame

## Phase Status

- [ ] Not started.

## Depends On

- Phase 2 merged with semantic projection and backend-neutral selection/marquee behavior.

## Objective

Create the real migration boundary: one frame-local least-privilege presentation object assembled
by the shared client and safe for either renderer to consume. Preserve the existing entity/fog/
feedback ordering while ensuring a backend never receives mutable `GameState`, hidden authoritative
entity variants, or source objects it can query differently. Keep Pixi behavior intact through a
clearly quarantined compatibility adapter rather than turning this phase into a renderer rewrite.

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
- Include plain renderer data with narrow submodels for:
  - visual-clock sample and interpolation metadata;
  - renderable interpolated entity presentation;
  - remembered buildings and other fog-memory presentation;
  - selection, relationship/team colors, HP/progress, and semantic selection proxies;
  - current visible/explored fog grids plus revisions and map/tile metadata;
  - ground decals, trenches, smoke/ability objects, shot reveals, and visual-only intel;
  - placement, range, rally, order, command-target, hover, miss, and Lab tool feedback;
  - observer/Lab map overlays and visual sample inputs; and
  - bounded diagnostics/context needed to explain fallbacks and frame cost.
- Build renderer feedback and ownership/relationship decisions once in the shared presentation
  assembly where practical. Do not place Babylon/Pixi nodes, textures, materials, matrices,
  cameras, mutable `GameState`, mutable `ClientIntent`, or transport objects in the frame.
- Define frame lifetime precisely. Ordinary scalar/object/array records are detached and frozen in
  development/contracts; large fog/terrain typed arrays may use a synchronous read-only borrowed
  lease keyed by revision to avoid per-frame copies. A backend cannot retain or mutate a borrowed
  lease after `render(frame)` returns; contracts detect mutation/retention, while fixed capture in
  Phase 5 will take a detached snapshot of the required revision.
- Move renderer-triggered one-shot non-event mutation behind shared reconciliation where needed.
  Pending decal batches and other destructive reads must not depend on which renderer ran first or
  whether capture rendered an extra frame; Phase 4 owns transient event normalization.
- Provide a narrow Pixi adapter that can temporarily supply legacy arguments/internal reads while
  exposing the new `render(frame)` backend seam to `Match`. Document every remaining legacy state
  read in the parity ledger and prohibit Babylon adapters from using it; do not perform a broad
  Pixi internals rewrite merely to remove the quarantine.
- Share the frame or its existing subviews with HUD, minimap, fog-facing diagnostics, and observer
  analysis where that reduces repeated state queries without forcing unrelated UI refactors.
- Update the client architecture checker and focused contracts to prevent the new presentation
  area from importing transport/UI/renderer internals or exposing mutable state.

## Expected Touch Points

- `client/src/frame_entity_views.js`
- a focused presentation-frame/model area or module
- `client/src/frame_recovery.js`
- `client/src/renderer/feedback_view_model.js`
- `client/src/state_ground_decals.js`
- `client/src/match.js` and `client/src/match_fixed_capture.js`
- `client/src/renderer/index.js` through a named Pixi compatibility adapter
- `client/src/minimap.js`, HUD, and observer analysis only where they consume existing frame views
- frame, borrowed-lifetime, renderer feedback, replay/Lab reset, and architecture contracts
- durable rendering/client design docs and parity ledger
- `plans/render3d/phase-3.md` status update in the implementation commit

## Presentation Boundary Requirements

- One shared assembly pass per rAF/fixed capture; repeated backend calls do not query or consume
  additional state.
- Renderer data is plain, least-privilege, and frame-local. A backend may retain stable ids and its
  own visual instance state, but not a borrowed frame/grid or shared model reference after render.
- Static map revisions and fog revisions are explicit so backends can cache safely.
- Hidden authoritative/current/fog-source variants never enter `RendererFrame` or backend
  diagnostics. Explicit received reveals and remembered views are separate typed categories.
- The renderer's soft-error boundary remains per-frame/per-entity; a bad presentation record cannot
  stop future frames.
- The compatibility adapter is named, isolated, tested, and ledgered. It is not available to the
  Babylon module and does not become a general escape hatch for future features.

## Explicit Exclusions

- No Babylon dependency or backend.
- No requirement to rewrite every Pixi helper around a new DTO in this phase.
- No transient event normalization/history; Phase 4 owns it.
- No protocol change and no client-side hidden-state reconstruction.
- No visual redesign, faction asset work, batching, or shadows.

## Implementation Checklist

- [ ] Define static map and dynamic presentation-frame schemas.
- [ ] Assemble one frame per rAF with plain entity, fog, feedback, overlay, and diagnostic data.
- [ ] Define detached versus synchronous borrowed-frame lifetime and enforce it in contracts.
- [ ] Move non-event one-shot visual consumption/reconciliation out of renderer implementations.
- [ ] Add and quarantine the Pixi compatibility adapter.
- [ ] Add architecture and pure-data contracts, including replay/Lab/reset behavior.
- [ ] Update durable docs/ledger and mark this phase done in the implementation commit.

## Verification

    node tests/client_contracts/frame_entity_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/client_contracts/renderer_feedback_contracts.mjs
    node tests/client_contracts/lab_interact_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

The new presentation-frame suite covers one assembly per frame, least-privilege renderer data,
fog-before-final-frame ordering, detached records, borrowed-grid mutation/retention, static/fog
revisions, replay seek, Lab reset, and rematch.

## Manual Test Focus

Run a normal Pixi match, replay seek/vision changes, live pause, Lab map reset, fixed capture, and
rematch. Watch decals, smoke/ability objects, selection feedback, placement, fog memory, and
observer overlays for missing, stale, or differently timed visuals; event reconciliation itself
remains Phase 4 work.

## Handoff Expectations

Document the final shared-context/renderer/static-map shapes, detached/borrowed lifetime, fog update
ordering, remaining Pixi legacy adapter reads, and architecture enforcement. Name Phase 4 as next
and identify the current event sources, destructive reads, pose lookups, deduplication keys, and
seek/reset/rematch behavior it must normalize.
