# Phase 9 - Visibility and Fog Core

## Phase Status

- [ ] Not started.

## Depends On

- Phase 8 merged with proven shared-resource ownership and lifecycle diagnostics.

## Objective

Implement the locked terrain/current-visibility/client-explored fog core with registry-owned
resources and explicit view-generation resets. Prove revisioned uploads, layer ordering, and
lifecycle before memory/reveal secrecy is added. Keep the controlled-Lab route gate closed.

## Work

- Implement the plan-locked semantic layer ids/order; do not redefine the cross-backend contract or
  copy Pixi container names into it.
- Render map/terrain boundary, authoritative current visibility, and client-accumulated explored
  state derived only from received visibility from Phase 3 snapshots. Bound uploads/allocations by
  revision and expose readiness/resource diagnostics.
- Introduce `viewGeneration`, advancing whenever replay/Lab/observer recipient perspective changes
  even at the same tick. Before the next render, clear/rebuild explored fog, SelectionScene, current
  fog GPU resources, and recipient-derived diagnostics; Phase 9.5 extends the reset set to memory,
  events/history, decals, and reveals.
- Gate all implemented surfaces—terrain/fog geometry, picking admission, diagnostics, capture
  metadata, resource keys, and future caster hooks—against received presentation only.
- Cover replay perspective hooks, spectator union view, Lab reset, resize, fixed/event capture,
  freeze, rematch, and resource teardown in controlled contracts without enabling normal routes.
- Add authoritative Lab scenario `render3d-fog-core` with fixed seed/team vision and deterministic
  current/explored/unseen regions. Capture through `lab-interact` and inspect one PNG once.
- Update durable rendering/parity docs with visibility/explored semantics, view generation, upload
  policy, and remaining memory/reveal work.

## Expected Touch Points

- Phase 3 frame/layer/grid descriptors
- `client/src/renderer_babylon/terrain.js`
- `client/src/renderer_babylon/fog.js`
- view-generation/reset and capture-readiness hooks
- `tests/client_contracts/babylon_fog_contracts.mjs` (create it in this phase)
- `tests/browser_babylon_fog.mjs` core cases wired into the authoritative runner
- durable rendering docs/parity ledger
- `plans/render3d/phase-9.md` status update in the implementation commit

## Security Requirements

- Babylon receives only least-privilege snapshots, never GameState/transport/full snapshots/fog-source subviews.
- Explored state accumulates only received authoritative visibility and resets on view generation.
- No previous-generation fog texture, SelectionScene, resource key, or diagnostic renders after advance.

## Explicit Exclusions

- No remembered buildings, below-fog intel, above-fog reveals, event/history/decal generation reset,
  or real two-recipient gate; Phase 9.5 owns them.
- No generic entities, overlays/effect, batching, shadows, vegetation, representative GLB, or normal route enablement.

## Implementation Checklist

- [ ] Implement locked layers and revisioned terrain/current/client-explored fog.
- [ ] Add view-generation clearing for core fog/input/resource surfaces.
- [ ] Cover replay/spectator/Lab/reset/capture/rematch lifecycle.
- [ ] Capture/inspect `render3d-fog-core` and update durable docs/ledger.
- [ ] Mark this phase done.

## Verification

    node tests/client_contracts/babylon_fog_contracts.mjs
    node tests/client_contracts/presentation_frame_contracts.mjs
    node tests/browser_babylon_fog.mjs --scenario core
    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Inspect current/explored/unseen boundaries, revision updates, perspective reset, resize, capture,
freeze, and rematch in controlled Lab. Look for stale textures/resource keys and generation mixing;
memory/reveals remain intentionally absent.

## Handoff Expectations

Report layer implementation, visibility/explored policy, upload revisions, view-generation clears,
resource baselines, preview command/URL, and inspected PNG. Name Phase 9.5 as next and identify
remembered/visionOnly/reveal categories, full reset set, real two-recipient sentinels, and fog-edge capture.
