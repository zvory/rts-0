# Phase 4 - Backend Kernel and Projection Seam

## Phase Status

- [x] Done.

## Depends On

- Phase 3.5 merged with the `render(frame)` seam, semantic camera API, perspective-safe selection,
  and detached presentation frame.

## Objective

Render the first authoritative Babylon Lab scene through the existing shared boundaries. Establish
the backend/camera ownership and coordinate decisions that would be expensive to reverse later,
without building fog parity, an asset pipeline, or production lifecycle machinery.

## Work

- Parse `rtsRenderer=pixi|babylon`; missing selects Pixi and invalid values show a bounded error.
  Load a pinned Babylon version only for the explicit Babylon path and record its version/license in
  a short vendor note.
- Replace the direct Pixi construction site with a small selected-backend bundle. The bundle creates
  the semantic camera first and the world renderer separately; the renderer receives only the
  detached frame produced with that camera's projection.
- Implement an engine-independent fixed-perspective semantic camera. Its
  `ProjectionSnapshotV1` drives both the scene and `SelectionSceneV1`.
- Put all world-pixel-to-scene point, facing, height, and scale conversion in one Babylon-private
  helper. Add pure representative-point and ground-hit tests proving scene/projection agreement.
- Create one Babylon-owned canvas, engine, and scene. Render map ground/bounds plus a few truthful
  generic visible primitives from `PresentationFrameV1` in an explicit Lab route.
- Call only `scene.render()` when Match invokes the backend. Support resize and idempotent destroy;
  show a bounded capability/creation error and perform one leave/re-enter cleanup check.
- Use `interact` to capture and inspect one authoritative kernel PNG.

## Keep Small

- No live player, replay, or spectator route.
- No GLB asset, asset schema/validator, event normalization, deterministic effect capture,
  generalized resource registry, context-loss program, benchmark harness, vegetation, or shadows.
- No protocol, server, simulation, command, or Pixi-default change.

## Acceptance

- Explicit Babylon Lab renders through `PresentationFrameV1`; normal Pixi remains unchanged and
  does not load Babylon.
- Babylon scene projection and semantic interaction projection agree for representative points and
  ground hits.
- Match remains the sole rAF owner, and one leave/re-enter leaves no extra canvas or active scene.
- The inspected capture shows the kernel is ready for fog/entities, not visual parity.

## Verification

Run focused selector, projection, coordinate, and backend-lifecycle contracts added by the phase,
then:

    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test and Handoff

Open the explicit Babylon Lab route, pan/dolly, resize, leave, and re-enter once. Report the selected
bundle shape, projection/conversion conventions, dependency/version note, single-loop evidence,
capture path, focused checks, and any limitation that Phase 5 must address.
