# Client rendering architecture

This document owns the renderer-neutral camera, selection, and presentation boundaries. It is a
design contract, not a production-migration roadmap. Current Pixi behavior remains authoritative
in [client-ui.md](client-ui.md), and implementation status lives in
[rendering-parity.md](rendering-parity.md).

## 1. Non-negotiable boundary

- There is one JavaScript client, one `Match`, one state/interpolation pipeline, and one active
  world renderer per match. Pixi is the default.
- During a match, `Match` owns the only main-thread `requestAnimationFrame` loop and visual clock.
  Pixi is pinned at v8.19.0 and initialized in one module worker with `autoStart:false` and an
  explicit WebGL preference; no normal, capture, or teardown path starts its ticker. The worker
  updates and presents only from submitted frames; Babylon never calls `runRenderLoop()`.
- Server/application coordinates remain two-dimensional world pixels. Scene axes, scale, height,
  and facing are backend-private presentation conversions.
- Input and commands use semantic projection and selection data. Meshes, asset bounds, LODs,
  shadow proxies, and GPU picking are never gameplay authority.
- A backend consumes only detached `PresentationFrameV2` data. It never receives `GameState`,
  `ClientIntent`, transport objects, hidden entity variants, or another backend's engine objects.
- The backend root owns its canvas, engine/renderer, scene, and shared GPU resources. Entity/effect
  children own instances only and cannot dispose shared resources.
- Missing `rtsRenderer` means Pixi. Babylon loads only for an explicit experimental selector.
- HUD, lobby, minimap, audio, diagnostics, and Lab panels remain shared external surfaces.

Rendering work does not change the Rust server, protocol, simulation, fog authority, replay format,
or command coordinates. A Pixi worker failure is a visible bounded fatal presentation error for
that match: it settles pending work, stops that match's animation-frame loop, and tears down without
selecting another renderer.

## 2. Semantic camera and projection

All public screen coordinates are viewport-local CSS pixels. DOM offsets, DPR, canvas backing size,
engine matrices, and hardware scaling stay inside the adapter. All public numbers are finite;
invalid mutation input leaves the current view unchanged.

```text
WorldPoint       = { x, y }
PresentedPoint   = { x, y, heightPx }
ScreenPoint      = { x, y }
ProjectedPoint   = { x, y, depth, clip, visible }
ProjectedExtent  = { width, height, scaleX, scaleY, visible }
WorldBounds      = { minX, minY, maxX, maxY }
CameraSnapshotV1 = {
  version: 1,
  focus: { x, y },
  framingScale,
  boundsPolicy: "mapOverscroll"
}
AudioListenerV1  = { x, y, referenceDistancePx }
```

`heightPx=0` is the authoritative plane. Positive height is presentation-only and never changes the
underlying `(x,y)`. `clip` is `inside`, `outsideViewport`, `outsideDepth`, or `behindCamera`.

The public `SemanticCamera` surface is:

```text
project(point) -> ProjectedPoint
groundAtScreen(screen) -> WorldPoint | null
projectedExtent(point, worldWidthPx, worldHeightPx) -> ProjectedExtent
viewportGroundPolygon() -> WorldPoint[]
viewportGroundBounds() -> WorldBounds | null
containsProjected(point, marginCssPx = 0) -> boolean
focusAt(point) -> void
framingForWorldPoints(points, { paddingCssPx = 0 } = {}) -> CameraSnapshotV1 | null
fitWorldPoints(points, { paddingCssPx = 0 } = {}) -> boolean
panByScreenDelta(delta) -> void
dollyBy(factor, anchorScreen?) -> void
resize(viewportWidthCssPx, viewportHeightCssPx) -> void
setMapBounds(worldWidthPx, worldHeightPx) -> void
snapshot() -> CameraSnapshotV1
restore(snapshotOrLegacy) -> boolean
audioListener() -> AudioListenerV1
subscribe(listener) -> unsubscribe
projectionSnapshot() -> ProjectionSnapshotV1
```

`groundAtScreen` returns `null` for no valid plane hit and never reuses a previous hit.
`viewportGroundPolygon` is a bounded clockwise polygon or `[]`; its AABB is only a conservative
helper, never final selection admission. `dollyBy` preserves a valid anchor point. New persisted
camera data uses `CameraSnapshotV1`; finite legacy `{x,y,zoom}` is accepted only at the private
restore compatibility edge and immediately normalized.

`projectionSnapshot()` returns detached `{version:1,camera,viewport,mapBounds,perspective?}` plus
pinned query functions. Fixed perspective snapshots include only finite plain coefficients
(`fovYRad`, `pitchRad`, focal length, camera distance, and near/far world depths), never an engine
matrix. The snapshot contains no live camera, engine, DOM, DPR, or mutable matrix object. The
selected backend's scene and `SelectionSceneV1` must use the same snapshot for a presented frame.

The audio listener is the ground focus, with reference distance equal to one viewport width at the
focus plane. Navigation, minimap, audio, alerts, control groups, carryover, Lab, observer, capture,
and diagnostics use only semantic operations.

Ordinary live-player sessions limit the visible ground footprint to 100 map tiles on either
viewport axis. `Match` converts that tile count to a world-pixel span and both camera adapters
combine it with their existing minimum framing scale on every viewport resize. Spectators,
replays, Lab sessions, and the Map Editor retain their wider overview ranges.

## 3. Renderer-neutral selection

Selection is an app-side sibling of the presentation frame and is never sent to a backend:

```text
SelectionProxyV1 = {
  id, kind, owner, selectClass, facing,
  anchor: PresentedPoint,
  footprint: { kind: "circle", radiusPx } |
             { kind: "polygon", points: WorldPoint[] },
  minScreenRadiusCssPx: 6,
  interaction: DetachedEntityView
}
SelectionSceneV1 = { version: 1, generation, frameId, projection, proxies[] }
```

Candidates come only from already fog-filtered interpolated views. The producer uses mirrored
semantic stats and authoritative building footprints, never asset geometry. Click, hover,
entity-target commands, ctrl-in-viewport, control groups, marquee, and Lab entity tools use
projected proxies. Ground commands/tools alone use nullable `groundAtScreen`.

Read-only cursor previews may sample the current semantic projection after camera movement so a
visual target remains beneath the cursor. They never issue a command or replace the last
successfully presented `SelectionSceneV1` used by ground-command input.

The marquee is the actual CSS-pixel screen rectangle. A render failure leaves the previous
successfully presented selection scene active. Selection never uses mesh picking, GPU ids, asset
bounds, shadow proxies, or fresh mutable state.

## 4. Presentation frame

Static map presentation changes only with the static-map revision. Per-rAF data is assembled once
by Match. Large grids cross the boundary as source-detached cloneable buffers:

```text
GridSnapshotV2 = {
  version: 2, revision, width, height,
  values: Uint8Array
}

StaticMapPresentationV2 = {
  version: 2, generation, revision,
  widthPx, heightPx, tileSizePx,
  terrain: GridSnapshotV2,
  resourceSites: readonly detached records[]
}

PresentationFrameV2 = {
  version: 2, generation, frameId, groundDecalRevision, visualTimeMs,
  projection: RendererProjectionV2, staticMapRevision,
  visible: GridSnapshotV2,
  explored: GridSnapshotV2,
  layers: LayerRecordsV1,
  diagnosticsContext
}
```

Presented entity records include backend-neutral `visualBounds` (`class`, `widthPx`, `depthPx`,
`heightPx`) derived from the mirrored entity stats. Placement feedback includes a detached
tile-footprint descriptor. Freehand formation feedback crosses `tacticalFeedback` as one detached
`formationMovePreview` record containing the sampled world-space `points` and provisional unit
`slots`; every backend renders both the stroke and admitted slots. Live and server-remembered
deployed enemy Anti-Tank Gun warnings cross the same boundary as detached
`enemyAntiTankGunThreat` records, including an explicit stale-memory presentation flag, so a backend
can render current amber-orange hatching and thinner very-pale-pink frozen hatching without consulting
mutable gameplay state. Friendly selected field-of-fire wedges remain unhatched. These are
presentation hints only: `SelectionSceneV1` remains the sole
entity/ground interaction authority.

The main thread keeps this rich `ProjectionSnapshotV1` for input, audio, minimap, and selection.
`PresentationFrameV2` instead carries a function-free `RendererProjectionV2` with `kind`, `camera`,
`viewport`, `mapBounds`, and plain backend coefficients. Orthographic records preserve the sampled
origin, framing scale, world size, and CSS viewport size exactly; perspective records carry their
existing plain coefficients. The Pixi owner reconstructs private projection queries from that data;
functions and camera instances never cross the renderer boundary.

The backend owns mutable staging buffers. Frame objects, arrays, and ordinary records are detached
and frozen; the explicitly revisioned grid `Uint8Array` values are copied from mutable game sources
and are the only typed-array leaves. Malformed individual records are dropped with bounded category
diagnostics; they do not abort the whole frame.

For received interpolated entities, Match creates one aligned frame-local preparation entry per
source record. Its complete graph-aware detached interaction feeds `SelectionSceneV1`; its
separately admitted presentation fields feed the entity layer without repeating nested clones.
Unadmitted fields can neither reach nor invalidate presentation, while cycles, mutable collection
views, unsupported prototypes, excessive depth, and non-finite values in admitted fields retain
the existing bounded entity-drop behavior. These entries are positional and frame-local—there is
no cross-frame cache or id lookup. A failed or superseded frame never publishes or retains that
frame's interaction; the prior successfully presented selection scene remains authoritative for
input.

Back-to-front layer ids are exact:

1. `staticGround`
2. `persistentGroundMark`
3. `fogGatedWorld`
4. `rememberedWorld`
5. `belowFogIntel`
6. `currentFog`
7. `aboveFogReveal`
8. `tacticalFeedback`
9. `screenOverlay`

Each descriptor is `{id, order, space, visibilityPolicy, depthPolicy}`. Later work may add optional
namespaced metadata but cannot rename/reorder layers or weaken visibility policy.

`frame_recovery.js` samples one projection and visual time, updates fog, builds feedback, reconciles
one monotonic ground-decal revision, assembles one frame, and calls `renderer.render(frame)`.
`PresentationCoordinator` owns the pending metadata for every accepted generation/frame id and is
the only consumer of renderer lifecycle outcomes. A submission exposes an independent `retained`
promise plus one terminal `presented`, `superseded`, `failed`, or `destroyed` promise. `retained`
acknowledges only the exact durable decal revision and is independent of displayed pixels; a later
failure cannot make Match resend and double-stamp that revision. Only `presented` advances the
public displayed-frame counter and publishes the matching `SelectionSceneV1`. Superseded and failed
frames discard their pending selection scene, while teardown settles pending work as destroyed and
blocks late selection/decal/capture side effects.

Babylon still updates scene state and synchronously presents exactly once inside `render(frame)`.
Pixi's main-thread host returns asynchronous promise channels, while its module worker performs one
`PIXI.Application.render()` for the accepted frame and acknowledges the exact generation/frame id.
Duplicate, stale, unknown, impossibly
ordered, or presented-after-destroy outcomes are bounded protocol diagnostics and cannot change the
newest visible selection scene.

The `PixiPresentationAdapter` is the sole bridge to existing Pixi helpers. Its exact private-read
allowlist uses `{id, reviewTrigger}` records: a trigger is a concrete reason to reconsider a read,
not a promised cleanup phase. New reads fail the contract, and Babylon cannot import the adapter or
receive its sources.

### 4.1 Render-worker message boundary

`RenderWorkerMessageV1` is the only Phase 3 worker vocabulary. Every request has
`{version:1,type,generation,payload}` and is validated for known type, matching presentation/static
versions, safe integer ids, finite bounded dimensions/DPR/timings, and shape-matched typed arrays.
Its payload lifetimes are explicit:

- initialization: transferred canvas, CSS size, DPR, PresentationFrameV2/StaticMapPresentationV2
  versions, and immutable configuration;
- map generation: one static-map payload and transferred terrain copy per generation;
- durable update: monotonic ground-decal revision plus its detached records;
- revisioned data: transferred visible/explored copies only when each revision changes;
- frame: dynamic layers, plain projection, visual time, ids, and grid revision references;
- control: resize, capture/flush, generation reset, and destroy.

Responses are `ready` after all renderer assets are ready, `retained`, `presented` with frame id and worker
update/present timings, `superseded`, bounded `failed`, and `destroyed`. The message builder never
transfers an assembler-owned buffer: it transfers copies so retained Phase 2 source records remain
usable and unchanged.

The worker treats an observed WebGL context loss as a terminal presentation failure; it does not
silently keep acknowledging black frames. Worker uncaught errors, unhandled rejections, message
decode failures, and context loss return a bounded stable code plus available source location.
Terminal responses also carry a bounded stack when the browser provides one.
The ready response also carries bounded WebGL vendor/renderer/version context. The host retains
those details with submitted/presented ids, in-flight/pending ids and ages, and last-message age.
`MatchNetReporter` includes that state in its normal bounded report and immediately emits one
report for a new terminal incident. A visible-tab frame that remains unacknowledged for at least
two seconds is notable server-side even when no worker error event arrives; hidden-tab throttling
does not classify as a worker stall.

The production `PixiWorkerPresentationAdapter` transfers the sole visible canvas during
construction. It admits one in-flight frame plus one latest pending frame. Replacing the pending
frame settles its exact id as `superseded`; durable decal messages have an independent retained
lifetime. Only a current-generation `presented` acknowledgment publishes that frame's stored
`SelectionSceneV1`. Fixed capture cancels ordinary pending work, waits for its exact frame, and
reads decoded RGBA from that same worker render task. Resize is held behind an in-flight frame and
ordered before the next frame; reset and idempotent destroy discard every pending frame, selection,
decal, capture, and resize record. Live, replay, spectator, Lab, stress, fixed capture, and Map
Editor all use this same worker host. There is no Pixi flag, synchronous canvas, hidden renderer,
WebGPU probe, or fallback renderer.

Worker display age starts when the host accepts the frame, before host-pending time, message
construction, cloning, and dispatch. Queue age uses that same boundary through worker task start;
main-submit timing separately isolates message construction, cloning, and `postMessage` cost.

The Map Editor keeps its camera/input animation-frame loop responsive but applies backpressure only
to worker submission, with no more than one editor presentation in flight. Terrain patches coalesce
by tile and complete overlays remain pending until the worker acknowledges the exact revision as
presented; edits made while that presentation is in flight merge into the next submission. This
lets ordinary match frames remain latest-oriented without treating authoring changes as disposable
presentation data. A failed editor worker stops new submissions and reports one visible error.

## 5. Babylon foundation contracts

### 5.1 Backend bundle and lifecycle

The selected backend bundle creates the semantic camera before Match and the world renderer
separately. The renderer receives projection only inside the detached frame. Babylon owns one
canvas, engine, and scene; `destroy()` is idempotent and removes them. Support resize and one normal
leave/re-enter cleanup case. Add cancellation tokens, context-loss recovery, or a generalized
lifecycle manager only when a real async/resource path needs them.

### 5.2 Coordinates

One Babylon-private module owns world/scene point, inverse ground point, facing, height, and scale
conversion. Entity, terrain, effect, and asset code import it rather than applying local axis swaps
or scale constants. Pure representative-point and ground-hit tests prove the scene and semantic
projection agree.

### 5.3 Fog and generic content

Babylon renders only the categories already separated by `PresentationFrameV2`. Current/explored
fog uses the immutable revisions; remembered, intel, and reveal presentation use their explicit
layers and never query a hidden source id. A real two-recipient sentinel test covers scene,
selection, and diagnostics before the live route is enabled.

Generic entities share simple source geometry/materials and remain truthful placeholders. They
preserve team, facing, construction, selection, and HP data received in the frame. Shared HUD,
minimap, audio, and control-group surfaces are reused. Babylon is opt-in for ordinary live players
and Lab; replay and ordinary spectator matches explicitly fall back to Pixi.

### 5.4 Flat-art reuse, trusted 3D assets, and events

Existing checked-in PNG, WebP, sprite-sheet, and SVG art may be reused as Babylon textures on
billboards or planes without introducing a new asset descriptor. Prefer the existing public URL,
frame rectangle, or plain source description when it is already suitable; Babylon never imports a
Pixi display object or runtime class. One static frame is sufficient for the playable catch-up, a
shadow is optional, and any load/adaptation failure falls back immediately to a truthful generic
primitive.

The first new repository-owned 3D asset needs only a small descriptor: id/path, source/license
note, scene scale, axes, ground pivot, visible bounds, team-material slot, and required visual
anchors. Missing required metadata falls back to a generic placeholder. Flat or 3D assets never
affect selection or authority.

The backend root owns loaded source assets, shared materials, and textures; entity/effect instances
own instantiated nodes/state only. Introduce a registry, reference counting, or pooling only when
more than simple root ownership is required by real content.

The first shared presentation event is immutable and self-contained: kind, authorized pose/anchor,
start time, finite lifetime, seed, layer, and payload. It is reconciled before the backend, never
looks up future/hidden entity state, and uses the Match visual clock. Retained history and
deterministic effect-capture tooling are separate future features.

## 6. Explicitly deferred

The Pixi v8 version cutover does not require render groups or shared `GraphicsContext` resources;
both remain follow-up performance experiments that need workload evidence and visual review.
The current foundations do not require replay/spectator Babylon routes, default rollout, hostile
asset validation, checksums/decoder policy, retained event history, generalized registries/pools,
benchmark schemas/budgets, vegetation, required shadows, quality tiers, full rig/animation parity,
new or re-authored faction art, or release certification. Add one through a new evidence-backed
plan when a playtest, content need, or measurement justifies it.

The deleted proof-of-concept remains historical only. Its observations may motivate a focused test
after the corresponding real resource/effect exists, but they are not requirements or baselines.

## 7. Executable evidence

The Pixi v8.19.0 cutover was reviewed with the deterministic 16-tick decoded-RGBA gate using the
same stream, state, camera, viewport, DPR, visual clock, and ready assets on both versions. Exact
equality failed on all 16 frames because the native v8 renderer rasterizes differently: changed
pixels ranged from 4.654% to 14.911% per frame (12.022% average), with average RGB MAE 1.753/255
and maximum RMSE 11.736. The capture-health, input, nonblank, and changing-frame canaries all
passed, and early/mid/late manual review found the same terrain, units, and fight effects without
missing or black scenes. This is an explicitly reviewed, accepted rasterization difference—not a
claim of exact pixel parity.

The code and focused contracts are the executable specification:

- `client/src/camera_projection.js` and camera projection contracts;
- `client/src/input/selection_projection.js` and selection projection contracts;
- `client/src/presentation/` and presentation-frame/layer contracts;
- `client/src/renderer/pixi_compatibility_adapter.js` and its exact allowlist contract; and
- `scripts/check-client-architecture.mjs` for presentation/renderer boundary enforcement.

Update this document when those cross-file contracts change. Record actual backend capability in
the parity ledger; do not add speculative future implementation detail here.
