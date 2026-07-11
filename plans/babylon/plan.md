# Babylon.js perspective-renderer proof of concept

## Purpose

Build one deliberately small, integrated proof of concept to decide whether Babylon.js should
replace PixiJS as Bewegungskrieg's world renderer. This is not a multi-phase implementation plan
and it does not authorize a production renderer migration. The proof must answer the expensive
architectural questions with a real game scene: can the existing authoritative JavaScript client
drive a perspective 3D presentation, can input still land in the server's 2D world coordinates,
and can dynamic shadows, animated vegetation, and particles deliver the desired visual step up at
an acceptable cost?

The desired result is a disposable-but-honest vertical slice. Visual polish and broad content
coverage are secondary; integrating with the battle-tested client and producing evidence for a
go/no-go decision are primary.

## Product requirements established by the discussion

These are the requirements for a future graphics revamp. Only the subset explicitly named in
**Proof-of-concept scope** must be implemented by this plan.

### Preserve the game while replacing its presentation

- Keep the Rust server authoritative and keep the existing wire protocol, simulation, fog-filtered
  snapshots, command model, prediction, interpolation, match lifecycle, and replay data model.
- Keep the existing JavaScript client. Babylon.js is a rendering dependency inside that client,
  not a reason to port application logic to a different engine or language.
- Keep the DOM application surfaces, including the lobby, HUD, command card, settings, match
  controls, and game-over UI. The minimap may remain on its existing canvas.
- Do not move gameplay authority, collision, pathfinding, visibility decisions, or combat into the
  renderer. Renderer animation and effects remain visual-only.
- Preserve the server's two-dimensional world-pixel coordinate contract. The 3D renderer maps that
  plane into its scene; it does not change the protocol to 3D coordinates.
- Allow an incremental migration. Pixi must remain the default during experimentation, and missing
  3D content may temporarily use ugly sprites, billboards, or primitive placeholders.
- Preserve clean teardown between matches. A replacement renderer must release its scene, engine,
  canvas, textures, meshes, materials, particle systems, listeners, and GPU resources.
- Continue to support the web as a first-class target. Do not introduce a Godot/GDScript rewrite,
  a C# dependency, or a native-only asset/runtime path.

### Camera and spatial interaction

- Move away from an orthographic camera. The intended presentation is a perspective, elevated RTS
  view with visible depth and parallax.
- Preserve camera pan and zoom/dolly behavior and make pitch, height, field of view, and initial
  look direction explicit, tunable presentation values.
- A player must still be able to select and command on the authoritative 2D plane. Screen-to-world
  input therefore becomes a ray/ground-plane intersection, while world-to-screen projection must
  remain available for labels and overlays.
- Perspective must not make gameplay unreadable. Units, team identity, selection, command
  feedback, terrain boundaries, and important combat events must remain legible at normal play
  zooms.
- Player-controlled camera orbit is not yet required. The first implementation may keep camera yaw
  and pitch constrained while proving perspective projection.

### Three-dimensional units and shadows

- Units and buildings must be able to use real 3D volume rather than requiring a pre-rendered
  sprite or shadow sprite for every possible facing direction.
- Units may face arbitrary angles. Vehicles must be able to represent independently oriented or
  animated parts such as a hull, turret, barrel, wheels, or tracks.
- Use a directional world light and dynamic shadow mapping so a rotating or moving object produces
  a correspondingly changing ground shadow.
- Terrain must receive shadows. Shadow quality must be tunable, visually stable during camera
  movement, and bounded by quality/performance settings.
- Simplified invisible shadow-proxy meshes are allowed where the visible asset is still 2D or where
  a detailed model is needlessly expensive.
- Adopt a browser-friendly 3D asset path, preferably glTF/GLB, with explicit conventions for world
  scale, origin, forward axis, named parts/anchors, team-color material slots, animation clips,
  texture ownership, and disposal.
- AI-assisted model generation is welcome, but generated assets must still pass the same scale,
  pivot, material, topology, and performance conventions as manually authored assets.

### Environment and animation

- Support genuine scene depth for terrain, buildings, trees, rocks, and other props.
- Support trees and other foliage that visibly animate in wind without per-object JavaScript
  animation work.
- Support dense tufts of grass and repeated environmental props through GPU/hardware instancing or
  an equivalent batched path.
- Permit quality tiers that reduce vegetation density, animation, and shadow participation on less
  capable hardware.

### Effects

- Support efficient particle effects for smoke, dust, debris, sparks, muzzle flashes, impacts,
  explosions, fire, and similar transient combat feedback.
- Effects must be driven by existing authoritative snapshots/events or existing client presentation
  state. They must not manufacture gameplay results or reveal information hidden by fog.
- Effects need world depth, layering, lifetime management, pooling/reuse where appropriate, and
  deterministic teardown. The eventual renderer should be able to combine particles with lighting,
  terrain interaction, and depth testing.

### Gameplay presentation that a full migration must retain

- Authoritative fog of war and remembered-building behavior, including no visual leaks of hidden
  entities, positions, targets, projectiles, deaths, or effects.
- Interpolated entity movement, weapon facing, recoil, setup/deployment state, construction state,
  shot reveals, and local-only presentation animation.
- Team colors, selection rings, HP bars, queues, placement ghosts, ranges, rally points, order and
  target previews, decals, trenches, observer analysis, Lab overlays, and other current world-space
  feedback.
- Fixed-capture/Lab screenshot behavior, replay viewing, spectator viewing, resize handling,
  rematches, and diagnostics sufficient to investigate missing assets and render failures.
- A graceful fallback or explicit unsupported message when required graphics capabilities are not
  available. The production decision should use WebGL 2 as the compatibility baseline; WebGPU may
  be an optional enhancement rather than a requirement for the first migration.

### Performance and maintainability

- Avoid one draw call, material, or texture load per repeated unit or vegetation instance where
  batching or instancing is possible.
- Define measurable budgets for draw calls, triangles, texture memory, active particles, shadow
  casters, shadow-map resolution, and vegetation density before production cutover.
- Retain the current soft-failure philosophy: one broken asset or entity draw must not terminate
  the render loop or match.
- Keep Babylon-specific objects behind a renderer-owned boundary. Model selection, asset metadata,
  render view models, and animation sampling should remain plain data where practical.
- Pin and self-host production dependencies or bundle them reproducibly. A pinned CDN build is
  acceptable for this isolated proof of concept, but the normal Pixi path must not download
  Babylon when the experiment is disabled.
- Do not judge the technology from close-up asset renders alone. Review visual quality,
  readability, aliasing, shadows, and performance from actual gameplay camera distances.

## Proof-of-concept scope

Implement exactly one opt-in Babylon vertical slice. It must be available from a namespaced Lab or
development launch flag such as `?rtsRenderer=babylon`; the exact spelling may follow existing URL
parsing conventions. The normal game and normal Lab launch must continue to use Pixi unchanged.

The proof must include all of the following:

1. **A real renderer seam.** `Match` selects the existing Pixi renderer or an experimental Babylon
   renderer through injected construction/configuration rather than spreading Babylon conditionals
   throughout application code. Babylon is loaded only for the experimental route. Both renderers
   retain `resize`, per-frame render, selection-box clearing/drawing as needed, capture-readiness or
   an explicit experimental substitute, and idempotent `destroy` behavior expected by `Match`.
2. **Real client state.** The Babylon scene consumes the same authoritative map, interpolated
   entity views, facing values, ownership/team colors, and visual clock used by the live Pixi path.
   Do not build a disconnected Babylon playground or manually animate a fake unit along a timer.
3. **A perspective RTS camera.** Use a Babylon perspective camera at an elevated angle, not an
   orthographic camera. Existing pan and zoom inputs must move/dolly the view. Implement accurate
   ray-to-ground `screenToWorld` conversion so at least click-selection and a ground move command
   work against the real game; implement the corresponding projection needed by any retained
   screen-space marker. Camera orbit and drag-box selection may be deferred.
4. **One representative articulated 3D unit.** Render at least one real in-match vehicle entity as
   a 3D tank-like object with a hull and an independently orientable turret or weapon part. A small
   checked-in GLB is preferred because it tests the intended asset pipeline; a documented
   procedural mesh is acceptable only if GLB loading becomes the sole blocker. Its position,
   hull facing, weapon facing, movement, and team identification must come from real entity state.
   All other entity kinds may use obvious primitive or billboard placeholders.
5. **Dynamic directional shadows.** Add one directional light, make the representative unit cast a
   dynamic shadow, and make the ground receive it. The shadow must visibly update as the unit turns
   and moves, remain reasonably stable while the camera moves, and expose at least shadow-map
   resolution and quality/enablement as experimental settings. Do not use pre-rendered directional
   shadow sprites.
6. **Animated instanced vegetation.** Place one bounded cluster containing instanced trees and
   instanced grass tufts. At least the foliage or grass must move through a material/vertex wind
   animation driven by the render clock, not one JavaScript update per plant. Trees should
   demonstrate selective shadow casting; grass may omit shadows.
7. **One event-driven particle effect.** Convert one existing combat presentation event—preferably
   a muzzle flash, impact, or explosion—into a Babylon particle effect with finite lifetime and
   cleanup. It must be triggered by real client event/presentation data and respect the same
   visibility inputs as that event, not by a looping showcase timer.
8. **Minimal gameplay readability.** Show team identity on the representative unit, one visible
   selection indication, the simple ground/terrain boundary, and enough placeholder rendering to
   follow the selected unit. Reuse the existing DOM HUD and minimap. Full fog and the complete Pixi
   overlay catalog are explicitly outside this proof.
9. **Lifecycle and evidence.** Enter and leave the experimental match twice without leaving extra
   canvases, active render loops, listeners, particle systems, or WebGL contexts. Publish bounded
   diagnostics for engine backend, frame time, draw calls, active meshes/instances, triangles,
   active particles, and shadow settings. Capture one clean gameplay PNG through the project-local
   Lab Interact workflow and inspect it once as required by repository guidance.

## Recommended implementation boundary

Keep the experiment isolated under a clearly named renderer area, for example
`client/src/renderer_babylon/`, and keep the existing `client/src/renderer/` implementation intact.
Introduce the smallest renderer-factory seam in the app shell. If perspective input requires a new
camera implementation, inject a camera/projection adapter that continues to expose the semantic
operations used by input (`pan`, `zoom/dolly`, `screenToWorld`, `worldToScreen`, resize/bounds) rather
than teaching input modules about Babylon classes.

Map authoritative world `(x, y)` to Babylon ground `(x, 0, z)` with one documented conversion and
forward-axis convention. Centralize that conversion; entity modules, effects, vegetation, and input
must not each invent their own sign swaps or scaling. Preserve server world pixels at the boundary
even if the Babylon scene uses a documented scene-unit scale internally.

Use a single Babylon scene and engine owned by the experimental renderer. Reconcile the
representative entity by stable server id, update transforms from the interpolated frame view, and
dispose removed objects after a bounded grace period. Keep materials and loaded assets shared rather
than cloning them per entity, and use instances for repeated vegetation.

Do not attempt a clever Pixi/Babylon production compositing architecture in this proof. A temporary
Pixi overlay is acceptable only if it is the smallest way to retain a selection box or existing
feedback, and it must be documented and torn down. The decision evidence should clearly distinguish
what Babylon rendered from what Pixi or the DOM retained.

## Acceptance criteria

The proof is complete only when all of these statements are supported by code and captured evidence:

- The default launch still uses Pixi and does not load or initialize Babylon.
- The opt-in route enters a real authoritative Lab/development match with a perspective Babylon
  view and no protocol or server-simulation changes.
- Pan and zoom/dolly work, clicking the representative unit selects it, and clicking valid ground
  can issue a real move command through the existing command path.
- The representative unit follows interpolated position plus hull/weapon facing, shows team
  identity, casts a moving/rotating dynamic shadow, and has a visible selection indication.
- A bounded instanced tree/grass cluster is present, wind motion is shader/material-driven, and
  vegetation does not create one per-frame JavaScript animation update per instance.
- One existing combat event produces a finite Babylon particle effect and hidden events are not
  made visible by the experimental path.
- Resizing works, two enter/leave cycles leave one expected canvas while active and no experimental
  canvas or render loop afterward, and no uncaught exception interrupts the match frame loop.
- Diagnostics record a reproducible benchmark scene and report median and p95 frame time together
  with draw calls, triangles, instance count, active particles, shadow-map size, browser, renderer
  backend, viewport, and device-pixel ratio. Do not claim a universal FPS target from one machine;
  compare Babylon enabled/disabled settings within the same experimental scene.
- Focused automated checks cover URL/config selection, coordinate conversion and ray/ground math,
  renderer lifecycle/idempotent teardown, hidden/default dependency loading, and any plain-data
  entity/effect adapters. `node scripts/check-client-architecture.mjs` passes.
- Lab Interact produces a clean PNG under `target/lab-interact/` showing the perspective camera,
  representative 3D unit, dynamic shadow, vegetation, selection indication, and a particle effect
  if it can be captured deterministically. The implementing agent inspects that artifact once and
  reports its absolute path.

## Explicitly out of scope for this proof

- Replacing Pixi as the default renderer or deleting any Pixi implementation.
- A multi-phase production migration plan.
- Finished art, an entire faction, complete unit/building coverage, skeletal character animation,
  or an AI-generated model-production pipeline.
- Full terrain fidelity, elevation gameplay, physics, 3D collision, server-side 3D coordinates, or
  changes to authoritative simulation.
- Full fog-of-war rendering, remembered buildings, shot-reveal fidelity, trenches, decals, every
  tactical overlay, observer analysis, replay parity, or fixed-capture parity beyond what is needed
  to collect the proof screenshot.
- Free camera orbit, cinematic controls, drag-box selection, mobile camera redesign, or final camera
  tuning.
- Production asset compression, final LOD policy, WebGPU support, global post-processing, final
  quality tiers, or a comprehensive performance target matrix.
- Converting all current sprites. Poor-looking billboards and primitives are acceptable evidence
  during the experiment as long as the representative unit and required scene features are clear.

## Required implementation handoff

The implementing agent must finish with a concise go/no-go assessment, not merely a list of files.
It must include:

- the exact launch URL or command for the proof;
- the inspected Lab Interact PNG path;
- focused automated-check results;
- the reproducible benchmark setup and recorded metrics;
- which parts were Babylon, Pixi, Canvas/DOM, procedural, or GLB-backed;
- lifecycle/resource-leak observations from two enter/leave cycles;
- known visual, input, fog, performance, asset-pipeline, and browser risks;
- a recommendation to proceed, revise the approach, or stop, supported by the evidence; and
- the core manual checks the next reviewer should repeat: perspective pan/zoom, selection and move
  input, arbitrary unit/weapon facing, shadow stability, wind motion, event particles, resize, and
  rematch teardown.

Implementation follows the repository's normal task-worktree and owned-PR workflow. Because the
proof changes rendering, the implementing agent must use the project-local `lab-interact` skill for
the authoritative scene capture and visual review.
