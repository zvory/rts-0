# Phase 3.5 - Pixi Presentation Cutover

## Phase Status

- [ ] Not started.

## Depends On

- Phase 3 merged with the least-privilege RendererFrame, immutable grids, and layer descriptors.

## Objective

Make `render(frame)` the only backend seam available from `Match` while preserving current Pixi
behavior. Quarantine necessary Pixi legacy reads and move non-event destructive consumption into
shared reconciliation so extra renders cannot change state. Prove runtime equivalence before event
normalization begins.

## Work

- Provide a narrow named Pixi compatibility adapter that consumes `RendererFrame` and may expose
  only an exact allowlist of temporary legacy reads frozen in Phase 0. Babylon code can never import
  or receive this adapter.
- Change `Match`/frame orchestration to assemble once, then call only `backend.render(frame)`.
  Repeated backend calls with the same frame do not query state or assemble again.
- Move pending decal batches and every renderer-triggered non-event one-shot/destructive read behind
  shared reconciliation before final frame assembly. Rendering or capture cannot consume shared
  queues differently.
- Preserve per-frame/per-entity soft-error behavior through bounded dropped-record diagnostics;
  backend failure cannot stop later Match frames.
- Share existing frame subviews with HUD, minimap, fog diagnostics, and observer analysis only where
  it removes duplicate state queries without expanding scope.
- Add a compatibility ratchet that fails on new Pixi legacy reads and records each remaining read/
  removal owner in the active ledger.
- Exercise normal/replay/live pause/Lab reset/fixed capture/rematch Pixi paths and compare ordering,
  decals, smoke/ability state, selection, placement, fog memory, and overlays.
- Update durable docs/ledger with the runtime seam, reconciliation ownership, allowlist, and evidence.

## Expected Touch Points

- Phase 3 presentation assembler
- `client/src/frame_recovery.js`, `client/src/match.js`, and `client/src/match_fixed_capture.js`
- `client/src/state_ground_decals.js`
- `client/src/renderer/index.js` through the named Pixi compatibility adapter
- `client/src/renderer/feedback_view_model.js`
- HUD/minimap/observer consumers only where they already share frame views
- presentation/Pixi equivalence/capture/replay/Lab/architecture contracts
- durable rendering/client docs and parity ledger
- `plans/render3d/phase-3.5.md` status update in the implementation commit

## Acceptance Requirements

- One assembly per ordinary/fixed frame and one `render(frame)` seam from Match.
- Pixi legacy reads are exact, ratcheted, ledgered, and unavailable to Babylon.
- Renderer/capture calls cannot destructively consume state or queues.
- Current Pixi presentation ordering and behavior remain materially equivalent.

## Explicit Exclusions

- No Babylon dependency/backend and no broad Pixi DTO rewrite.
- No transient event identity/history; Phase 4 owns it.
- No protocol, visual redesign, batching, shadows, or faction work.

## Implementation Checklist

- [ ] Add/quarantine the Pixi compatibility adapter and exact legacy-read ratchet.
- [ ] Cut Match over to one assembled `render(frame)` call.
- [ ] Move non-event one-shot/destructive consumption before assembly.
- [ ] Prove Pixi runtime/capture/replay/Lab/rematch equivalence and soft errors.
- [ ] Update durable docs/ledger and mark this phase done.

## Verification

    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/client_contracts/renderer_feedback_contracts.mjs
    node tests/client_contracts/lab_interact_capture_contracts.mjs
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Run normal Pixi, replay seek, live pause, Lab reset, fixed capture, and rematch. Watch decals,
smoke/ability objects, selection feedback, placement, fog memory, and observer overlays for missing,
stale, duplicate, or differently timed presentation.

## Handoff Expectations

Report the final runtime seam, Pixi adapter/allowlist, destructive reconciliation, shared UI views,
soft-error behavior, and equivalence evidence. Name Phase 4 as next and identify event sources,
pose lookups, deduplication inputs, renderer-local derivation debt, and reset semantics.
