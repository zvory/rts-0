# Phase 4 - Babylon Opt-in Kernel

## Phase Status

- [ ] Not started.

## Depends On

- Phase 3.5 merged with the one-frame `render(frame)` seam, semantic camera/projection, and
  perspective-safe selection contracts.

## Objective

Put the smallest honest Babylon scene behind the existing presentation boundary. This phase is for
seeing the camera and renderer work in a controlled authoritative Lab scene, not for proving a
production asset, capture, loading, or performance architecture.

## Work

- Add the explicit `rtsRenderer=babylon` opt-in path while preserving Pixi for an unset selector.
  Keep Babylon out of the default launch path; record the chosen Babylon version and license in a
  short vendor/readme note, without building an integrity or update pipeline.
- Adapt the existing Pixi-only construction site to choose a small backend bundle before `Match`
  constructs its collaborators. The bundle creates the semantic camera and the renderer separately;
  the renderer accepts only `PresentationFrameV1`, never `GameState`, `ClientIntent`, raw fog grids,
  Pixi objects, or a mutable camera.
- Give Babylon an engine-free fixed-perspective semantic camera implementation with the same public
  camera API. Its detached `ProjectionSnapshotV1` feeds both `SelectionSceneV1` and the frame the
  scene renders. Add a focused pure projection/scene-conversion check for representative world
  points and ground hits now, even though Lab interaction is deferred, so Phase 5 cannot pair
  perspective visuals with Pixi's orthographic picking.
- Create one renderer-owned canvas, engine, and scene. Render static ground and a few simple generic
  primitives from the frame in `/lab?rtsRenderer=babylon`; primitives are sufficient and no GLB
  contract is required.
- Put world-pixel-to-scene point, facing, and height conversion in one small Babylon-private module.
  Public picking and commands continue to use the existing semantic projection/selection path.
- Call `scene.render()` only from the `Match` frame/capture hook. Do not use Babylon's engine loop,
  tickers, recurring timers, or a parallel visual clock.
- Implement normal resize and idempotent destroy. One Lab enter/leave/rematch check must show no
  extra canvas or active rAF; report a bounded error for unavailable WebGL or scene creation.
- Use `lab-interact` to arrange and inspect one authoritative Babylon kernel PNG under
  `target/lab-interact/`.

## Explicit Exclusions

- No live player route, replay/spectator support, full fog, remembered state, or broad entity set.
- No retained-event capture, effect system, asset schema/validator, GLB pipeline, resource registry,
  context-loss matrix, benchmark harness, pool, vegetation, shadow, quality-tier, or final art work.
- No protocol, server, command, selection-authority, or Pixi-default change.

## Acceptance

- An explicit Lab Babylon launch renders the controlled scene through the shared frame seam.
- Babylon's scene conversion and the selected semantic projection agree at representative points and
  ground hits; the renderer receives the projection only inside the detached frame.
- The normal Pixi launch remains Babylon-free and unchanged.
- `Match` remains the only rAF owner, and one leave/rematch returns to one world canvas and no
  orphaned Babylon scene.
- The inspected capture and a focused backend/selector check demonstrate the kernel is usable for
  the next phase.

## Expected Touch Points

- backend selection/composition near `client/src/match.js` and launch parsing
- a focused `client/src/renderer_babylon/` kernel and coordinate helper
- minimal selector/backend/lifecycle contracts and one browser/Lab smoke case
- `docs/design/client-rendering.md`, `docs/design/rendering-parity.md`, and this phase status

## Verification

Run focused selector/backend and coordinate tests added with the implementation, then:

    node scripts/check-client-architecture.mjs
    tests/run-all.sh --only-browser-scenarios=smoke
    git diff --check

## Manual Test Focus

Open the explicit Babylon Lab route, pan and resize the view, then leave and re-enter once. Confirm
that the normal Pixi launch has not changed, the Lab scene has one canvas, and the browser console
does not show a second renderer loop or a stale scene after teardown.

## Handoff Expectations

Report the selector, Babylon version/source note, backend ownership, coordinate convention, single
rAF evidence, focused check names, capture path, and the first limitation the controlled scene
reveals. Name Phase 5 as next; do not propose additional phases before that playtest slice exists.
