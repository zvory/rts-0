# Pre-alpha 3D Renderer Experiment

## Purpose

Use the renderer-neutral seams completed in Phases 0 through 3.5 to put an opt-in Babylon world
renderer in front of players quickly. This is a pre-alpha experiment, not a production migration:
Pixi remains the default, and the value of the work is a playable comparison on the same
authoritative game rather than proof that every future rendering concern has been solved.

The only goal of this plan is a small, honest Babylon path that uses the shared presentation,
camera, and selection contracts. It ends after an explicit Babylon live-play session; future work
must be chosen from playtest evidence rather than precommitted as a long certification chain.

## Current State

Phases 0, 1, 1.5, 1.75, 2, 3, and 3.5 are complete. They established the two boundaries that keep
the Babylon path from silently diverging from Pixi:

- `Match` assembles one detached, fog-filtered `PresentationFrameV1` and calls one
  `render(frame)` backend seam.
- Camera, projection, selection, marquee, and command coordinates are semantic plain-data
  contracts; a renderer mesh is never gameplay authority.

The Pixi compatibility adapter is intentional temporary code. It keeps existing Pixi presentation
working while preventing a new backend from reading `GameState`, `ClientIntent`, hidden fog data,
or Pixi internals. Its remaining private reads are debt to remove only when a concrete Babylon or
Pixi need makes that work worthwhile; they are not a scheduled production-cleanup program.

## Pre-alpha Constraints

- Pixi is the default. Babylon is available only when the user explicitly requests
  `rtsRenderer=babylon`; it never silently replaces or falls back from a running Pixi match.
- There is one `Match`, one simulation/interpolation path, one visual clock, and one active world
  renderer per match. `Match` alone owns `requestAnimationFrame`; Babylon must not call
  `runRenderLoop()` or create a second visual loop.
- Babylon consumes the existing detached presentation frame and semantic camera/selection APIs.
  It must not query `GameState`, `ClientIntent`, transport data, raw fog data, or engine-derived
  ownership/visibility.
- The selected backend bundle creates the semantic camera before `Match` starts and the world
  renderer separately. The renderer receives only the detached frame; its scene and
  `SelectionSceneV1` must use the same camera-produced `ProjectionSnapshotV1`. Babylon perspective
  must never be paired with Pixi's orthographic projection for picking, marquee, or ground commands.
- World coordinates, commands, fog authority, and simulation remain two-dimensional and
  server-owned. Babylon scene conversion lives in one small renderer-private helper; meshes and
  assets never control picking or commands.
- The first Babylon slice renders only received/authorized data. A focused real two-recipient
  check must prove a never-received entity or position is not shown, picked, or named in Babylon
  diagnostics.
- A backend owns and releases its own canvas, engine, scene, and resources. One successful
  enter/leave/rematch check is required; elaborate registries, ten-cycle certification, and
  speculative pooling are not.
- Keep a missing WebGL capability or asset failure bounded and visible. A pre-alpha experiment may
  show an error or a simple placeholder; it must not stop the Match frame loop or affect Pixi.

## Deliberate Deferrals

The following are not acceptance gates for the experiment: retained-event capture, universal
event identity, full effect parity, a GLB schema/validator pipeline, asset checksums, a hierarchical
resource registry, benchmark schemas and budgets, batching/pool tuning, vegetation, shadows,
quality tiers, a representative final asset, replay/spectator parity, and a device rollout matrix.
They become separate small plans only when a playtest or a measured failure demonstrates a need.

## Remaining Phases

### [Phase 4 - Babylon Opt-in Kernel](phase-4.md)

Add the smallest selectable Babylon backend behind the existing frame seam. It creates one
renderer-owned canvas/scene, uses the semantic camera and a centralized world-to-scene conversion,
and renders static ground plus simple generic primitives in an explicitly requested Lab launch.
Prove the single-loop and enter/leave behavior, then capture one authoritative Lab scene; do not
build an asset pipeline, performance program, or full visual catalog.

### [Phase 5 - Playable Fogged Vertical Slice](phase-5.md)

Turn the kernel into an explicitly requested live pre-alpha path: authoritative current/explored
fog, visible generic entities, semantic selection, marquee, and basic movement feedback. Prove a
focused two-recipient no-leak case, then enable only explicit Babylon live/Lab launches while Pixi
remains unchanged. Stop after a user playtest; replay/spectator support, effects, art, and
performance work are follow-up decisions rather than implied commitments.

## Phase Index

1. Phase 0 - Contract, inventory, and baselines — done
2. Phase 1 - Semantic camera and projection core — done
3. Phase 1.5 - Navigation and minimap migration — done
4. Phase 1.75 - Shared camera consumer closure — done
5. Phase 2 - Perspective-safe picking and marquee selection — done
6. Phase 3 - Renderer-neutral presentation frame — done
7. Phase 3.5 - Pixi presentation cutover — done
8. [Phase 4 - Babylon Opt-in Kernel](phase-4.md)
9. [Phase 5 - Playable Fogged Vertical Slice](phase-5.md)

## Execution and Playtest Gate

Run one phase at a time from a clean worktree and wait for its owned PR to merge before starting the
next one. The phase runner may execute Phase 4 and, once it is merged, Phase 5; it must not invent
additional render3d phases or treat historical phase files as work items.

After Phase 5 merges, make a Babylon live/Lab link available for an actual playtest. Record what
players can see and do, what was confusing or slow, and the first concrete limitation encountered.
Only then create a new, bounded plan for that limitation. The next plan may not make Babylon the
default or remove Pixi without a separate reviewed decision.

Each phase handoff reports the changed seam, focused automated checks, one Lab Interact capture
path, the core manual test, and any playtest-relevant limitation. Do not manufacture exhaustive
matrices, benchmark reports, or future work merely to satisfy a handoff template.
