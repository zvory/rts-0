# Client rendering architecture

This document owns the renderer-neutral camera, selection, and presentation boundaries. It is a
design contract, not a production-migration roadmap. Current Pixi behavior remains authoritative
in [client-ui.md](client-ui.md), and implementation status lives in
[rendering-parity.md](rendering-parity.md).

## 1. Non-negotiable boundary

- There is one JavaScript client, one `Match`, one state/interpolation pipeline, and one active
  world renderer per match. Pixi is the default.
- During a match, `Match` owns the only `requestAnimationFrame` loop and visual clock. Pixi is
  constructed with `autoStart:false`; no normal, capture, or teardown path starts its ticker. A
  backend updates and presents only when Match calls it; Babylon never calls `runRenderLoop()`.
- Server/application coordinates remain two-dimensional world pixels. Scene axes, scale, height,
  and facing are backend-private presentation conversions.
- Input and commands use semantic projection and selection data. Meshes, asset bounds, LODs,
  shadow proxies, and GPU picking are never gameplay authority.
- A backend consumes only detached `PresentationFrameV1` data. It never receives `GameState`,
  `ClientIntent`, transport objects, hidden entity variants, or another backend's engine objects.
- The backend root owns its canvas, engine/renderer, scene, and shared GPU resources. Entity/effect
  children own instances only and cannot dispose shared resources.
- Missing `rtsRenderer` means Pixi. Babylon loads only for an explicit experimental selector.
- HUD, lobby, minimap, audio, diagnostics, and Lab panels remain shared external surfaces.

Rendering work does not change the Rust server, protocol, simulation, fog authority, replay format,
or command coordinates. A renderer failure is a bounded presentation failure and cannot stop later
Match frames.

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
by Match. Large grids cross the boundary as immutable accessors:

```text
GridSnapshotV1 = {
  version: 1, revision, width, height,
  get(index) -> number,
  copyInto(targetTypedArray, targetOffset = 0) -> copiedCount
}

StaticMapPresentationV1 = {
  version: 1, generation, revision,
  widthPx, heightPx, tileSizePx,
  terrain: GridSnapshotV1,
  resourceSites: readonly detached records[]
}

PresentationFrameV1 = {
  version: 1, generation, frameId, visualTimeMs,
  projection, staticMapRevision,
  visible: GridSnapshotV1,
  explored: GridSnapshotV1,
  layers: LayerRecordsV1,
  diagnosticsContext
}
```

Presented entity records include backend-neutral `visualBounds` (`class`, `widthPx`, `depthPx`,
`heightPx`) derived from the mirrored entity stats. Placement feedback includes a detached
tile-footprint descriptor. These are presentation hints only: `SelectionSceneV1` remains the sole
entity/ground interaction authority.

The backend owns any mutable staging buffers. Frame objects, arrays, and ordinary records are
detached and frozen. Malformed individual records are dropped with bounded category diagnostics;
they do not abort the whole frame.

For received interpolated entities, Match creates one aligned frame-local preparation entry per
source record. Its complete graph-aware detached interaction feeds `SelectionSceneV1`; its
separately admitted presentation fields feed the entity layer without repeating nested clones.
Unadmitted fields can neither reach nor invalidate presentation, while cycles, mutable collection
views, unsupported prototypes, excessive depth, and non-finite values in admitted fields retain
the existing bounded entity-drop behavior. These entries are positional and frame-local—there is
no cross-frame cache or id lookup. A failed render never publishes or retains that frame's
interaction; the prior successful selection scene remains authoritative for input.

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
pending ground decals, assembles one frame, and calls `renderer.render(frame)`. Only after a
successful presentation does Match publish the matching `SelectionSceneV1` and acknowledge its
reconciled ground decals. Each adapter call first updates backend scene state, then synchronously
presents exactly once. Pixi presentation is one `PIXI.Application.render()` call; Babylon
presentation is one `Scene.render()` call. Update or present failure returns `presented:false`, does
not advance the successful renderer-frame count, and cannot prevent Match from scheduling its next
RAF.

The `PixiPresentationAdapter` is the sole bridge to existing Pixi helpers. Its exact private-read
allowlist uses `{id, reviewTrigger}` records: a trigger is a concrete reason to reconsider a read,
not a promised cleanup phase. New reads fail the contract, and Babylon cannot import the adapter or
receive its sources.

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

Babylon renders only the categories already separated by `PresentationFrameV1`. Current/explored
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

The current foundations do not require replay/spectator Babylon routes, default rollout, hostile
asset validation, checksums/decoder policy, retained event history, generalized registries/pools,
benchmark schemas/budgets, vegetation, required shadows, quality tiers, full rig/animation parity,
new or re-authored faction art, or release certification. Add one through a new evidence-backed
plan when a playtest, content need, or measurement justifies it.

The deleted proof-of-concept remains historical only. Its observations may motivate a focused test
after the corresponding real resource/effect exists, but they are not requirements or baselines.

## 7. Executable evidence

The code and focused contracts are the executable specification:

- `client/src/camera_projection.js` and camera projection contracts;
- `client/src/input/selection_projection.js` and selection projection contracts;
- `client/src/presentation/` and presentation-frame/layer contracts;
- `client/src/renderer/pixi_compatibility_adapter.js` and its exact allowlist contract; and
- `scripts/check-client-architecture.mjs` for presentation/renderer boundary enforcement.

Update this document when those cross-file contracts change. Record actual backend capability in
the parity ledger; do not add speculative future implementation detail here.
