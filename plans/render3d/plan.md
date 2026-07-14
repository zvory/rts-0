# Sustainable 3D Renderer Foundations

## Purpose

Build an opt-in Babylon renderer on the renderer-neutral seams already completed through Phase
3.5. The goal is to make the 3D path playable quickly while keeping the few abstractions that are
expensive to correct later: backend ownership, projection/selection agreement, world/scene
coordinates, fog authority, and shared presentation data.

This is a pre-alpha renderer, not a production migration. Pixi remains the default. Performance
programs, exhaustive parity, polished content, and release certification wait until a playtest or
measurement demonstrates a need.

## Completed Foundation

Phases 0 through 3.5 are complete. They established:

- semantic camera and projection APIs used by navigation, minimap, audio, control groups, Lab, and
  diagnostics;
- perspective-safe selection and ground interaction that do not depend on renderer meshes;
- one detached, fog-filtered `PresentationFrameV1` assembled by `Match`; and
- one `render(frame)` backend seam, with current Pixi behavior isolated behind its compatibility
  adapter.

The Pixi compatibility adapter is intentional transition code. Its private reads are an exact
ratcheted list with concrete re-evaluation triggers; they are not a standing cleanup program.

## Remaining Architecture Constraints

- Pixi is the default. Babylon is selected explicitly with `rtsRenderer=babylon` and does not load
  on the default path.
- One backend is active per Match. `Match` owns the only `requestAnimationFrame` loop and visual
  clock; Babylon calls no engine loop.
- The selected backend bundle creates the semantic camera and world renderer. The scene and
  `SelectionSceneV1` use the same detached `ProjectionSnapshotV1`; Babylon perspective must never
  be paired with Pixi orthographic picking.
- Babylon receives only `PresentationFrameV1`, never `GameState`, `ClientIntent`, transport data,
  raw hidden variants, or Pixi objects.
- World/scene point, facing, height, and scale conversion live in one Babylon-private module.
  Commands and simulation remain two-dimensional world pixels.
- Renderer meshes, assets, LODs, and shadow proxies never determine selection, ownership,
  visibility, pathing, or commands.
- The backend root owns its canvas, engine, scene, and shared GPU resources. Entities/effects own
  only their instances. Add a generalized registry or pool only when multiple real resources need
  it.
- A real two-recipient test must prove never-received entities and positions do not appear in
  Babylon rendering, picking, or diagnostics before live play is enabled.
- Missing capability, malformed presentation data, or missing art produces a bounded error or
  truthful placeholder. It does not corrupt Match state or silently change renderer mid-match.

## Remaining Phases

### [Phase 4 - Backend Kernel and Projection Seam](phase-4.md)

Add the explicit selector, lazy Babylon dependency, selected-backend camera/renderer bundle, fixed
perspective projection, centralized world/scene conversion, and one renderer-owned scene. Render a
small authoritative Lab scene from `PresentationFrameV1`, with Match as the sole frame-loop owner.
Prove projection/scene agreement, resize, bounded failure, and one leave/re-enter cleanup cycle.

### [Phase 5 - Playable Fog and Interaction Slice](phase-5.md)

Render current/explored fog, remembered/reveal categories, truthful generic entities, selection/HP,
and only the command feedback required to play a basic match. Use the same perspective projection
for rendering and selection, and pass one real two-recipient secrecy check. Enable explicit Babylon
live and Lab routes, then stop for a user playtest while Pixi remains the default.

### [Phase 6 - Representative Asset and Effect Spine](phase-6.md)

After the Phase 5 playtest, validate the content boundaries with one repository-owned vehicle asset
and one finite attack/muzzle effect. Add only a small trusted asset descriptor, explicit shared
versus instance ownership, and a self-contained fog-filtered presentation-event shape. Capture and
inspect the representative scene, record simple observed counters, and end this plan without
turning one asset into a catalog or performance program.

## Phase Index

1. Phase 0 - Contract, inventory, and baselines — done
2. Phase 1 - Semantic camera and projection core — done
3. Phase 1.5 - Navigation and minimap migration — done
4. Phase 1.75 - Shared camera consumer closure — done
5. Phase 2 - Perspective-safe picking and marquee selection — done
6. Phase 3 - Renderer-neutral presentation frame — done
7. Phase 3.5 - Pixi presentation cutover — done
8. [Phase 4 - Backend Kernel and Projection Seam](phase-4.md)
9. [Phase 5 - Playable Fog and Interaction Slice](phase-5.md)
10. [Phase 6 - Representative Asset and Effect Spine](phase-6.md)

## Deferred Backlog

The removed phase files contained useful ideas, but these are not automatic follow-on work. Create
a new small plan only when a playtest, content need, or measurement justifies one:

- replay/spectator Babylon routes and default-renderer rollout;
- retained event history, deterministic multi-offset effect capture, and capture tooling;
- hostile/untrusted asset validation, checksums, decoder policy, and broad provenance machinery;
- generalized resource registries, reference counting, effect pools, and async-generation tracking;
- benchmark schemas, fixed scenario suites, comparison reports, structural budgets, and CI timing
  policies;
- batching/thin-instance optimization beyond the simple sharing used by real content;
- vegetation, shadows, quality tiers, and device-specific tuning;
- faction-wide art conversion, full overlay/effect parity, and polished animation; and
- exhaustive lifecycle/certification gates such as ten-cycle exact-count testing.

## How the Old Plan Was Compressed

| Old phases | Disposition |
| --- | --- |
| 4–5 events/capture | Minimal self-contained event contract moves to Phase 6; retained deterministic capture is deferred. |
| 6–8 loading/lifecycle/coordinates/resources | Essential selector, projection, conversion, and root ownership move to Phase 4; generalized infrastructure is deferred. |
| 9–10.5 fog/entities/interaction/routes | Core fog secrecy, generic entities, interaction, and minimal feedback combine in Phase 5; replay/spectator and long-tail parity are deferred. |
| 11–11.5 benchmarks/batching/pools | Simple observed counters and obvious sharing only; formal performance work is deferred. |
| 12–12.5 vegetation/shadows | Deferred as optional visual features. |
| 13 representative asset | One trusted representative asset moves to Phase 6 without deterministic generation or a hostile-input pipeline. |
| 13.5 evidence gate | Removed; focused phase checks plus the repository CI gate are sufficient for pre-alpha. |

## Execution and Handoff

Implement one phase per clean branch/owned PR and wait for it to merge before starting the next.
Run Phases 4 and 5 as the first chain, then stop; do not invoke Phases 4–6 as one unattended runner
range. Phase 6 requires explicit user approval after the Phase 5 playtest. Each graphics phase uses
Interact once for a small authoritative scene, inspects the returned PNG, and reports the
capture path. Verification should cover the phase's architectural risk, not an exhaustive matrix
of every route and lifecycle combination.

After Phase 5, the handoff must include an actual Babylon playtest. Phase 6 begins only when the
user confirms that its representative asset/effect slice remains the priority; otherwise revise or
stop it. After Phase 6, archive this plan. Any further work begins as a new bounded plan based on
observed needs; it is not inferred from the deferred backlog.
