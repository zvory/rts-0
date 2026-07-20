# Phase 2 - Worker-Safe Presentation and Assets

## Phase Status

- [ ] Ready after Phase 1 is merged and reachable from `origin/main`.

## Objective

Make the complete Pixi input boundary safe to send to a worker without changing the active
main-thread renderer. Replace non-cloneable projection functions and DOM-only asset paths, define
the bounded message vocabulary the worker will consume, and remove the Map Editor's direct use of
Pixi objects so Phase 3 can cut over every Pixi surface at once.

This phase may add pure wire-format and worker-entry contract tests, but it must not start a
production worker, transfer a production canvas, add a selector, or retain an experimental second
renderer path.

## Entry Gate

- Phase 1 is merged and its head is reachable from `origin/main`.
- The handoff identifies the lifecycle result vocabulary, frame-id ordering, durable decal revision,
  fixed-capture promise, and latest successfully displayed selection semantics.
- Phase 1 deterministic parity is exact with ready assets.

## Cloneable Presentation Boundary

- Keep the rich semantic camera/projection object on the main thread for input, minimap, audio, and
  `SelectionSceneV1`. Put only plain finite camera, viewport, map-bounds, and perspective values in
  the renderer-facing frame and reconstruct renderer-private projection helpers inside the Pixi
  owner.
- Version the changed detached presentation shape and update its design source of truth. Do not send
  functions, DOM nodes, Pixi objects, class instances, callbacks, mutable Maps/Sets, or other values
  that fail `structuredClone`.
- Add a contract that runs representative live, replay/spectator, Lab, visual-sample, observer,
  fog, trench, decal, and effect frames through `structuredClone` and verifies the clone is detached
  and semantically equal.
- Separate data by lifetime rather than inventing a general delta engine:
  - initialization: canvas dimensions, DPR, renderer version, and immutable configuration;
  - map generation: static map and renderer-static inputs once per generation;
  - durable updates: monotonic ground-decal batches;
  - revisioned data: visible/explored grids and other large buffers only when revisions change;
  - frame data: dynamic presentation records, plain projection data, visual time, and frame id;
  - control: resize, capture/flush, generation reset, and destroy.
- Specify one response vocabulary matching Phase 1: ready/asset readiness, retained durable
  revision, presented frame id with worker timing, superseded frame id, bounded failure, and
  destroyed. Validate message type, version, generation, ids, finite numeric bounds, and expected
  transferables on both sides.
- Prefer transferable typed-array buffers for large revisioned grids when ownership can move
  safely. Clone ordinary frame records first; a binary codec, shared memory, and cross-frame entity
  delta protocol remain deferred until measurements prove they are needed.

## Worker-Safe Asset Pipeline

- Preserve the existing authored ground-decal SVG files as source art, but generate and check in a
  deterministic PNG mask atlas plus manifest/rect metadata that the runtime can decode in a worker.
  Update asset contracts to verify source-to-atlas coverage, dimensions, deterministic selection,
  and successful runtime readiness; do not silently fall back to procedural decals when the atlas
  fails.
- Make PNG rig atlases, frame strips, color-adjusted textures, terrain/trench canvases, and other
  current Pixi assets load through `fetch`, `createImageBitmap`, `OffscreenCanvas`, or another
  worker-safe primitive. Do not rely on `document.createElement`, DOM `Image`, a hidden HTML canvas,
  or browser-global event systems in code that Phase 3 will execute in the worker.
- Preserve nearest-neighbor sampling, tinting, alpha, source rectangles, readiness reporting, and
  deterministic clocks. A changed pixel, missing texture, pending asset, or swallowed load error
  blocks the phase.
- Keep one asset implementation that works in the current main-thread owner and the future worker;
  do not retain parallel DOM and worker loaders as long-lived branches.

## Map Editor Boundary

- Stop `MapEditorViewport` from constructing `PIXI.Graphics`, `PIXI.Text`, or reaching into renderer
  layers/application internals. Represent its terrain, grid, symmetry guides, starts, bases,
  selection markers, labels, and paint preview as a small detached editor presentation record.
- Keep pointer and keyboard listeners, hit math, camera control, session edits, and tool state on the
  main thread. Bind input listeners to the stable HTML canvas element owned by the renderer host,
  not to a Pixi application object.
- Let the current main-thread renderer consume the editor record in this phase. Phase 3 will send
  the same record to the worker, avoiding a special second Pixi route after cutover.

## Expected Touch Points

- `client/src/presentation/frame.js` and projection record/reconstruction modules
- `client/src/camera_projection.js` only where the renderer-facing snapshot is separated
- `client/src/renderer/pixi_compatibility_adapter.js`
- `client/src/renderer/index.js`
- `client/src/renderer/decals/asset_loader.js`, manifest, generated atlas metadata, and source assets
- PNG rig, frame-strip, color-adjusted texture, terrain, and trench asset helpers
- `client/src/map_editor_viewport.js` and a detached Map Editor presentation module
- new versioned worker wire validator/records under `client/src/renderer/`
- asset generation/check scripts and focused client contracts
- `docs/design/client-rendering.md`
- `docs/design/client-ui.md` for Map Editor ownership
- `docs/design/rendering-parity.md`

## Verification

- Focused structured-clone and wire validation contracts for every message and response type,
  generation reset, malformed ids/numbers, detached buffers, and no mutation of source records.
- Ground-decal source/atlas coverage and browser readiness contracts, plus PNG rig, frame-strip,
  terrain, trench, and texture readiness checks without DOM image/canvas dependencies.
- Map Editor contracts for terrain rebuild, overlays, pointer behavior, resize, one present per editor
  RAF, and idempotent teardown without direct Pixi access.
- Existing presentation-frame, projection, Pixi adapter, renderer, fixed-capture, replay, Lab,
  visual-sample, and observer contracts.
- `node scripts/client-render-parity.mjs --baseline-worktree <phase-1-baseline> --candidate-worktree <phase-worktree> --samples 16 --seed renderworker-phase-2`
- `node scripts/check-client-architecture.mjs`
- `node scripts/check-docs-health.mjs`
- `git diff --check`

Exact decoded-RGBA parity and complete ready assets are required. Do not approve a raster decal
difference merely because the source moved from per-SVG runtime decoding to a generated atlas; the
atlas must reproduce the current masks and compositing exactly.

## Manual Test Focus

Inspect one normal match with infantry, vehicle, mortar, artillery, and building ground marks plus
PNG/frame-strip units at early, middle, and late deterministic ticks. Exercise Map Editor painting,
symmetry guides, bases, zoom/pan, resize, and leave/re-enter while confirming the ordinary renderer
still behaves exactly as before.

## Completion and Handoff Expectations

Mark this phase done in its implementation commit. The handoff must name the final presentation and
worker-message versions, list each payload lifetime, record asset generation and readiness evidence,
confirm Map Editor no longer touches Pixi, and provide exact parity results. State explicitly that
the production renderer still has one main-thread path and that Phase 3 must replace it atomically
without adding a flag or fallback.
