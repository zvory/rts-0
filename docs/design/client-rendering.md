# Client rendering architecture

This document is the durable contract for renderer-neutral camera, selection, presentation,
capture, backend lifecycle, asset, ownership, and performance work. The existing Pixi module
catalog and current look remain authoritative in [client-ui.md](client-ui.md); this document owns
the boundary that allows another world renderer without duplicating gameplay authority. Backend
status and gate evidence live in the active [rendering parity ledger](rendering-parity.md).

## 1. Scope and non-negotiable boundary

- There is one JavaScript client, one `Match`, one state/interpolation pipeline, and exactly one
  active world backend per match. Pixi is the default and release-required backend.
- `Match` owns the only `requestAnimationFrame` loop and the visual clock. A backend renders only
  when called by the ordinary frame or fixed-capture path; Babylon must never call
  `runRenderLoop()`.
- Server and application coordinates remain two-dimensional world pixels. Scene axes, scale,
  elevation, and facing conversion are backend-private and never enter commands, snapshots,
  pathing, fog authority, or replay data.
- Input and selection use the plain-data projection and selection contracts below. Render meshes,
  asset bounds, LODs, shadow proxies, and GPU picking are never command or selection authority.
- A backend consumes only a detached `PresentationFrameV1` and already-received, fog-filtered
  presentation events. It cannot query `GameState`, `ClientIntent`, hidden entity variants, or
  infer visibility/ownership from scene objects.
- GPU resources belong to the backend hierarchy. Entity/effect children may release their own
  instances but may never dispose a shared texture, material, shader, source asset, pool, shadow
  resource, engine, or scene.
- A default launch must not download, import, parse, initialize, or retain Babylon code. Missing
  `rtsRenderer` means `pixi`; Babylon stays explicitly experimental until a later reviewed default
  cutover.
- DOM HUD, lobby, minimap, audio, diagnostics, and Lab panels are shared external surfaces. They
  are not composited into either world backend.

No rendering phase may change the Rust server, wire protocol, simulation, fog filtering, replay
format, command coordinates, or gameplay authority. Renderer failures are bounded presentation
failures: malformed views/assets/effects log diagnostics and use a truthful fallback where the
contract permits, without terminating the frame loop.

## 2. Semantic camera and projection contract

All public screen values are viewport-local CSS pixels. DOM offsets, device pixel ratio, canvas
backing size, matrices, engine rays, and hardware scaling are adapter-private. All public numbers
must be finite; invalid input is rejected without mutating the current view.

### 2.1 Plain-data shapes

```text
WorldPoint       = { x, y }                         // authoritative plane, world px
PresentedPoint   = { x, y, heightPx }               // height is presentation-only world-px scale
ScreenPoint      = { x, y }                         // viewport-local CSS px
ProjectedPoint   = { x, y, depth, clip, visible }
ProjectedExtent  = { width, height, scaleX, scaleY, visible }
WorldBounds      = { minX, minY, maxX, maxY }
CameraSnapshotV1 = {
  version: 1,
  focus: { x, y },                                  // player-intent ground focus in world px
  framingScale,                                     // CSS px per world px at focus
  boundsPolicy: "mapOverscroll"
}
AudioListenerV1  = { x, y, referenceDistancePx }
```

`heightPx=0` is the authoritative plane. Positive `heightPx` is allowed only for semantic visual
anchors/proxies and never changes the underlying `(x,y)`. `ProjectedPoint.depth` is signed adapter
view depth: positive is in front, zero is the camera plane, negative is behind. `clip` is exactly
`inside`, `outsideViewport`, `outsideDepth`, or `behindCamera`; `visible` is true only for finite,
positive-depth points inside viewport and depth limits. Clip classification uses this priority:
non-positive depth is `behindCamera`, positive depth outside the adapter's near/far limits is
`outsideDepth`, and an otherwise valid point outside the CSS viewport is `outsideViewport`.
The Pixi orthographic adapter intentionally projects positive `heightPx` at the same screen point
and depth as its ground anchor because the current top-down renderer has no elevation axis; a
perspective adapter projects it along its renderer-private vertical scene axis.

`framingScale` is measured at the focus ground point. It equals legacy Pixi `zoom`, but perspective
adapters may implement it by bounded dolly/height while keeping fixed reviewed pitch, yaw, and FOV.
Those adapter settings are not player orbit state and are not serialized.

### 2.2 Public `SemanticCamera` operations

The production API names and behavior are frozen as follows:

```text
project(point: PresentedPoint) -> ProjectedPoint
groundAtScreen(screen: ScreenPoint) -> WorldPoint | null
projectedExtent(point: PresentedPoint, worldWidthPx, worldHeightPx) -> ProjectedExtent
viewportGroundPolygon() -> WorldPoint[]
viewportGroundBounds() -> WorldBounds | null
containsProjected(point: PresentedPoint, marginCssPx = 0) -> boolean
focusAt(point: WorldPoint) -> void
fitWorldPoints(points: WorldPoint[], { paddingCssPx = 0 } = {}) -> boolean
panByScreenDelta(delta: ScreenPoint) -> void
dollyBy(factor, anchorScreen?: ScreenPoint) -> void
resize(viewportWidthCssPx, viewportHeightCssPx) -> void
setMapBounds(worldWidthPx, worldHeightPx) -> void
snapshot() -> CameraSnapshotV1
restore(snapshotOrLegacy) -> boolean
audioListener() -> AudioListenerV1
subscribe(listener) -> unsubscribe
projectionSnapshot() -> ProjectionSnapshotV1
```

`groundAtScreen` is nullable even though the orthographic adapter normally hits. It rejects points
behind the camera/horizon and returns only finite map-plane `(x,y)`. It never returns an elevated
point or a cached prior hit. `projectedExtent` projects a centered semantic width/height at the
point and supplies local CSS-per-world `scaleX/scaleY`; it does not expose nominal zoom.

`viewportGroundPolygon` intersects the current frustum with the `heightPx=0` plane and map bounds,
deduplicates coincident vertices, and returns stable clockwise winding in world `(x,y)`. It returns
`[]` when no bounded ground is visible. `viewportGroundBounds` is only its conservative AABB and
returns `null` for an empty polygon; consumers must not use it for final selection admission.

`focusAt`, `fitWorldPoints`, pan, dolly, resize, and map bounds preserve the current Pixi overscroll
policy. `dollyBy(factor)` multiplies framing scale (`factor>1` makes content larger) and keeps the
world point under a valid anchor fixed; omitted anchor means viewport center. `fitWorldPoints`
returns false and leaves the view unchanged for no finite points. `subscribe` emits one detached
`CameraSnapshotV1` after a successful semantic mutation, never raw adapter state.

Phase 1 implements this surface on `Camera` as the production Pixi orthographic adapter. Query
inputs and mutation values are finite numbers; invalid query values throw except
`groundAtScreen`, whose contract-safe failure is `null`, while invalid mutations leave the view
unchanged. World sizes, map sizes, viewport sizes, padding, and margins must be non-negative.
`panByScreenDelta` accepts the semantic `{x,y}` CSS-pixel record and temporarily retains the legacy
numeric `(dx,dy)` call shape for Phase 1.5 migration.

The audio listener is the ground focus. Its perspective-safe formula is exact:

```text
referenceDistancePx = viewportWidthCssPx /
  projectedExtent({ ...focus, heightPx: 0 }, 1, 1).scaleX
```

The value is a world-pixel reference distance despite its historical `Px` suffix. The adapter uses
the finite positive focus scale; if projection is temporarily unavailable it retains the last
valid value, or uses `1920` before the first valid view. This preserves Pixi's one-screen-width
attenuation and gives perspective a deterministic focus-plane equivalent.

### 2.3 Restore, projection snapshots, and compatibility

New carryover, profile, replay, Lab, capture, and diagnostics data stores only
`CameraSnapshotV1`. `restore` accepts version 1 and rejects unknown versions. The one named legacy
edge may also accept finite `{x,y,zoom}` and immediately normalize it using the current viewport
and the adapter's ordinary framing-scale bounds:

```text
normalizedZoom = clamp(zoom, minFramingScale, maxFramingScale)
focus.x = x + viewportWidthCssPx / (2 * normalizedZoom)
focus.y = y + viewportHeightCssPx / (2 * normalizedZoom)
framingScale = normalizedZoom
```

Legacy values are never re-emitted. This read compatibility remains at the App/camera restore edge
through the foundations plan; removing it requires a separate migration decision. Raw
`x/y/zoom/viewW/viewH`, `worldToScreen`, and `screenToWorld` remain temporary private orthographic
compatibility only inside `camera.js` and the named Pixi adapter. Phase 1.75 closes every shared
consumer read.

`projectionSnapshot()` returns detached `ProjectionSnapshotV1`: `{version:1, camera, viewport,
mapBounds}` plus `project`, `groundAtScreen`, `projectedExtent`, `viewportGroundPolygon`,
`viewportGroundBounds`, `containsProjected`, `snapshot`, and `audioListener` query functions.
`viewport` is exactly `{widthCssPx,heightCssPx}` and `mapBounds` is a world-pixel AABB or `null`
before positive map dimensions exist. Its closures pin immutable orthographic coefficients and
contain no live `Camera`, Babylon/Pixi, DOM, backing-store, DPR, or matrix objects. Selection and a
frozen frame use the last successfully presented projection snapshot, never a newer camera pose
awaiting presentation.

Through Phase 1.75, `camera.js` exposes the raw `x`, `y`, `zoom`, `viewW`, `viewH`,
`worldToScreen`, `screenToWorld`, `centerOn`, `setZoom`, `setBounds`, and `setView` compatibility
edge for existing Pixi and shared consumers. Phase 1.5 migrates navigation and minimap; Phase 1.75
migrates every other shared consumer and leaves raw reads only in `camera.js` and the named Pixi
adapter. The `{x,y,zoom}` legacy restore read remains accepted at the App/camera restore edge for
the foundations plan but is always re-emitted as `CameraSnapshotV1`.

## 3. Renderer-neutral selection

The presentation assembler owns `SelectionSceneV1` as an application-side sibling of the backend
frame; it is never a field of `PresentationFrameV1`. Input consumes it only after `Match` publishes
the scene for a successfully rendered frame. A render failure leaves the previous scene active.
Candidates come only from already fog-filtered interpolated views.

```text
SelectionProxyV1 = {
  id, kind, owner, selectClass, facing,
  anchor: PresentedPoint,
  footprint: { kind: "circle", radiusPx } |
             { kind: "polygon", points: WorldPoint[] },
  minScreenRadiusCssPx: 6
}
SelectionSceneV1 = { version: 1, generation, frameId, projection, proxies[] }
```

The app-owned proxy producer uses mirrored semantic stats, never assets. The anchor `(x,y)` is the
interpolated presentation position and `heightPx` is exactly `max(8, STATS[kind].size || 0)` for
units/resources and `max(8, max(footW,footH) * tileSize * 0.5)` for buildings. Unit/resource
footprints use mirrored `size`; buildings use their rotated authoritative tile footprint. Oriented
vehicle bodies may use their mirrored body polygon. These sources are presentation-only and do not
change server hitboxes.

Every entity click, hover, entity-target command/ability, ctrl-in-viewport, control-group admission,
ordinary marquee, and Lab entity/box tool uses projected proxies. Ground commands/tools alone use
`groundAtScreen`. Click ordering first applies the interaction's eligibility and existing own-unit
preference, then distance to projected anchor, nearest positive visible depth, and stable numeric
id. Marquee ids order by distance from drag start to projected anchor, then id. The marquee is the
actual CSS-pixel screen rectangle; proxy intersection, not a ground polygon/AABB, is final.

The screen marquee is `screenOverlay` presentation with backend-neutral lifecycle. Selection never
uses mesh picking, GPU ids, asset bounds, shadow proxies, or fresh mutable state.

## 4. Presentation frame and layers

### 4.1 Static versus per-frame ownership

Static map data is submitted on backend creation and whenever the static-map revision changes. It
does not ride the per-rAF frame. Per-rAF data is assembled once by `Match`: projection,
already-filtered interpolated entities, remembered buildings, fog grids, visual selection state,
client feedback, normalized events, Lab/observer overlays, and screen overlays. The app-side
`SelectionSceneV1` is assembled from the same frame context but is published only after successful
presentation and is not sent to a backend. A backend receives no `GameState`, `ClientIntent`,
selection proxies, or mutable typed-array view.

`GridSnapshotV1` is frozen and exact:

```text
{ version: 1, revision, width, height,
  get(index) -> number,
  copyInto(targetTypedArray, targetOffset = 0) -> copiedCount }
```

There is no field or method that exposes the source typed array. `get` returns `undefined` outside
`0 <= index < width*height`; `copyInto` validates capacity. The assembler reuses the same immutable
snapshot while source revision is unchanged and creates a new detached copy only on revision
change. Each backend owns its staging buffers. Fixed capture pins the snapshot object/revision.

```text
StaticMapPresentationV1 = {
  version: 1, generation, revision,
  widthPx, heightPx, tileSizePx,
  terrain: GridSnapshotV1,
  resourceSites: readonly detached records[]
}

LayerRecordsV1 = {
  staticGround: readonly detached records[],
  persistentGroundMark: readonly detached records[],
  fogGatedWorld: readonly detached records[],
  rememberedWorld: readonly detached records[],
  belowFogIntel: readonly detached records[],
  currentFog: readonly detached records[],
  aboveFogReveal: readonly detached records[],
  tacticalFeedback: readonly detached records[],
  screenOverlay: readonly detached records[]
}

PresentationFrameV1 = {
  version: 1, generation, frameId, visualTimeMs,
  projection, staticMapRevision,
  visible: GridSnapshotV1, explored: GridSnapshotV1,
  layers: LayerRecordsV1,
  diagnosticsContext
}
```

The static-map object, layer object, arrays, and records crossing this seam are detached and frozen.
`LayerRecordsV1` is a frozen plain object with exactly the locked layer keys, not a JavaScript
`Map`: `Object.freeze()` does not prevent `Map.prototype.set()`, and this client has no type system
that could make a `ReadonlyMap` enforceable at runtime. A backend may report counts and failures
but cannot attach engine objects to either object. `staticGround` per-frame records are normally
empty because terrain and resource sites live in `StaticMapPresentationV1`; the key remains present
so the semantic layer shape and backend traversal order are total.

### 4.2 Locked semantic layers

Back-to-front ids are exact and permanent:

1. `staticGround`
2. `persistentGroundMark`
3. `fogGatedWorld`
4. `rememberedWorld`
5. `belowFogIntel`
6. `currentFog`
7. `aboveFogReveal`
8. `tacticalFeedback`
9. `screenOverlay`

Every descriptor has exactly `{id, order, space, visibilityPolicy, depthPolicy}`. `space` is
`world` or `screen`; `visibilityPolicy` is `static`, `alreadyFiltered`, `remembered`, `intel`,
`fogMask`, `reveal`, or `local`; `depthPolicy` is `ground`, `world`, `overlay`, or `screen`.
Later phases may add optional, namespaced metadata but may not rename/reorder ids or weaken their
visibility policy. Event kinds are assigned to one descriptor in Phase 4.

## 5. Presentation events and deterministic capture

Normalized events are detached records:

```text
PresentationEventV1 = {
  version: 1, id, kind, seed, admittedAtMs, durationMs, expiresAtMs,
  layerId, owner, position: PresentedPoint | null, facing, payload
}
```

`id` uses a received stable id when one exists. Otherwise it is exactly
`${timelineGeneration}:${authoritativeTick}:${eventIndex}:${kind}:${authorizedPayloadHash}`, where
`eventIndex` is the event's ordinal in the ordered tick event array and `authorizedPayloadHash` is
a stable canonical hash of the normalized received payload. Identical delivery/reconstruction of
the same event deduplicates; identical same-tick events remain distinct by ordinal; a same-id,
different-payload collision is dropped with a bounded diagnostic. `seed` is unsigned FNV-1a over
`id|kind|owner` and never uses `Math.random`. Admission copies every authorized pose/anchor/payload
needed later; retained events never resolve an old entity id against future state. Lifetimes are
finite and kind-owned. The required short fixture is normalized `attack`/muzzle feedback with
exactly `240 ms` duration and fixed-capture offsets `0/80/160/240 ms` (240 is the expired boundary).

The bounded history contains at most 256 actually received events and at most the trailing 10,000
ms of live visual time; oldest entries are removed until both constraints hold. Reset, seek,
rematch, or view-generation change clears history. A fixed capture freezes one complete
`StaticMapPresentationV1` and `PresentationFrameV1`, their projection/grid/static revisions, and a
detached selection of retained events. Synthetic time is applied only to that detached playback.
Live admission always uses the live visual clock; network, input, audio, health, timeouts, and
server ticks remain real. Capture never patches `performance.now()` or starts another rAF.

## 6. Backend selection and lifecycle

The selector is exactly `rtsRenderer=pixi|babylon`. Missing means `pixi`; any other value is an
actionable pre-join error. It is not persisted. Babylon requires WebGL 2 and is prepared before an
App, socket, join, or auto-launch exists. Lack of WebGL 2 or dependency/integrity failure is a
bounded pre-join error, never WebGL 1, WebGPU, or silent Pixi fallback.

The app-owned resolver returns a `BackendBundleV1 {id, prepareToken, create}`. Pixi resolves from
the static graph. In Phase 6, Babylon selects the highest official stable patch available at
implementation time and records official package/source integrity, file SHA-256, license, and
update procedure; preparation loads only same-origin vendored minified UMD core then glTF loader.
A stale or cancelled token cannot create App/socket/canvas/listeners. After preparation, `START`
handling is synchronous and transactional.

`create(parent, {renderClock})` returns one backend implementing:

```text
id
buildStaticMap(staticMap)
render(frame) -> { presented: boolean }
resize(widthCssPx, heightCssPx, dpr)
freeze(frame)
reset({ generation })
enterFixedCapture(captureClock)
presentFixedCaptureFrame(frame)
exitFixedCapture(liveClock)
captureReadiness(query)
destroy()
```

Construction failure destroys the partial backend and every already-created Match collaborator
exactly once, restores bounded lobby/error UI, and prevents later messages from reaching it.
Failure after `START` never changes renderer id or falls back mid-match; it retains the last
successfully presented frame where possible, reports diagnostics, and keeps `Match` frame recovery
alive. `resize`, `reset`, capture enter/exit, and `destroy` are idempotent. `freeze` returns detached
data only. Late asset/effect completions check backend generation and destruction before attach;
stale completions release only what they created. Destroy during load/capture invalidates tokens,
restores/suppresses rAF as appropriate, and leaves no canvas/context/listener.

## 7. Resource ownership and teardown

The parent scope is the only default disposer. Children release references/instances upward and
never call disposal on a shared dependency.

| Class | Parent scope / allowed disposer | Lifecycle rule |
| --- | --- | --- |
| Application/backend/scene | `Match` backend slot / backend `destroy` | Owns canvas, engine, scene, root registries, and all descendants; destroyed once. |
| Shared dependency | Backend/scene registry / registry root | Reference-counted or root-lived; a child only releases a handle. |
| Cached source asset | Backend asset cache / cache | Source container/template survives instances; stale loads self-release before registration. |
| Material/texture/shader | Backend shared registry / registry | Shared by stable key; entity/effect instances cannot dispose it. |
| Entity instance | Entity registry / entity reconciler | Owns only transform/nodes unique to id; removal releases shared handles. |
| Effect instance | Effect pool/registry / effect reconciler | Owns only leased instance state; completion returns a fully reset lease. |
| Pool | Scene/backend / pool | Bounded; reset clears event, pose, owner, seed, callbacks, visibility, clocks, and generation. |
| Shadow resource | Scene shadow manager / shadow manager | Light/map/proxy sources are shared; entity proxies register/unregister only. |
| Listener | Module/backend that installed it / same owner | Exact callback/options removed on reset/destroy; Match calls collaborator teardown. |
| Canvas/context | Backend / backend | One active world canvas/context; removed/released on rollback/destroy. |
| Timer/rAF | `Match` or owning collaborator / same owner | `Match` alone owns rAF; all timers are recorded and cancelled. |
| Late async load | Backend generation token / initiating registry | Completion must validate token; cancellation cannot attach or mutate diagnostics as live. |

Current Pixi ownership includes its Application/stage/world/layers, pooled Graphics/Text/rig
instances, decal canvas/texture, cached adjusted textures, atlas/frame-strip loads, fog graphics,
selection and placement graphics, and the screen drag graphic. Raw Pixi asset-cache textures are
shared dependencies and are not destroyed by adjusted-texture teardown.

## 8. Current-main inventory and migration ownership

### 8.1 Raw camera representation consumers

| Surface | Current dependency | Owner phase |
| --- | --- | --- |
| `camera.js` and Pixi world transform in `renderer/index.js` | private `x/y/zoom/viewW/viewH`, orthographic transforms | Phase 1 compatibility; stays backend-private |
| `frame_recovery.js` and spatial `audio.setListener` | raw center, zoom, viewport width | Phase 1.75 listener model |
| navigation/camera controls/replay input | zoom read-modify-write, screen drag | Phase 1.5 semantic pan/dolly |
| `input/index.js`, `selection.js`, placement/commands/Lab tools | non-null `screenToWorld`, world rectangle | Phase 2 ground hit/proxies |
| `input/control_groups.js` | raw visible rectangle and center | Phase 1.75 projection/fit |
| `minimap.js` | raw rectangle and `centerOn` | Phase 1.5 ground polygon/focus |
| `match.js` | bounds/resize/home focus, viewport alert AABB | Phase 1 core then Phase 1.75 shared closure |
| App/replay carryover and `camera_view_selection.js` | legacy view objects | Phase 1.75 versioned snapshot |
| `lab_interact_bridge.js` and Lab capture manifests | raw set/focus, world bounds, viewport tests | Phase 1.75 semantic tooling snapshot |
| `visual_profiles.js` and scenario initial cameras | `{x,y,zoom}` literals | Phase 1.75 normalize profiles |
| observer map analysis and visual samples | nominal zoom for screen sizing | Phase 1.75 projected extent |
| `frame_profiler.js`, capture/readiness/status | raw viewport/zoom diagnostics | Phase 1.75 semantic snapshot |

Backend-private orthographic math is allowed only in `camera.js` and the named Pixi adapter.
Application, UI, input, minimap, audio, Lab, replay, observer, capture, and diagnostics reads must
be removed by Phase 1.75; Phase 2 then replaces selection's semantic assumptions.

### 8.2 Current Pixi presentation catalog

The implementation order in `renderer/layers.js` maps to the semantic contract as follows:

| Current presentation | Semantic layer / notes |
| --- | --- |
| Terrain | `staticGround`; cached map terrain |
| Permanent death/impact decals and trench ground terrain/connectors | `persistentGroundMark` |
| Local visual samples | diagnostic records split by their declared semantic category |
| Resource nodes, building/unit shadows, buildings/overlays, units and SVG/PNG rigs | `fogGatedWorld` |
| Remembered building silhouettes | `rememberedWorld` |
| Legacy vision-only intel | `belowFogIntel` |
| Smoke/ability ground effects and authoritative fog overlay | effects by event policy, fog in `currentFog` |
| Shot-reveal shadows/units and authorized reveal events | `aboveFogReveal` |
| Selection/range/FoF rings, HP bars, attack/hover feedback, miss toasts, projectile/impact/muzzle effects | `tacticalFeedback` unless Phase 4 assigns an authorized reveal |
| Placement ghosts/footprints, command/order lines/destinations/entity markers, ability and Lab tool area previews | `tacticalFeedback` |
| Observer map analysis and Lab world overlays | fog-safe declared layer; never implicit above-fog access |
| Drag marquee and capture/readiness/error overlays | `screenOverlay` |
| HUD, menus, scoreboard, observer/Lab panels, minimap, audio | shared external |

The catalog includes selection and HP presentation, setup/deployment animation, tank tracks,
trench occupant lips/shadows, static resources, ground decals, remembered buildings, fog, shot
reveals, smoke, abilities, attacks/projectiles/impacts/muzzle flashes, Panzerfaust and magic-anchor
feedback, command/placement feedback, observer/Lab diagnostics, visual samples, capture readiness,
soft render diagnostics, resize, reset, and teardown. Placeholder coverage is not parity.

### 8.3 Current lifecycle and capture dependencies

`Match` constructs `GameState`, `ClientIntent`, `Camera`, Pixi `Renderer`, `Fog`, HUD, input router,
minimap, input/replay input, audio routing, diagnostics, and room-time surfaces. It installs network,
window, document, and DOM listeners, interval/timer collaborators, then owns/reschedules rAF through
`frame_recovery.js`. `Match.destroy()` is the parent cleanup point. Renderer teardown must cover
Pixi Application/canvas, display pools, rigs, textures it created, decals, selection/placement/fog
graphics, and pending asset loads. Minimap owns canvas/window pointer listeners and cached canvases;
input/replay input own pointer/keyboard/lock listeners; audio is App-owned and persists between
matches but its match voices/listener pose are reset.

Fixed capture currently suspends/restores Match rAF, swaps the injected render clock, renders with
alpha 1, and explicitly presents the Pixi frame. Capture-time rig/effect/smoke/projectile/recoil/
toast sampling uses the visual clock; snapshots, health, profiler admission, input, audio, and
timeouts remain real. Phase 5 must stop synthetic capture time from entering live state, pin frame
and grid revisions, retain authorized real events, and add no-tick-step event capture.

## 9. Asset and representative-fixture policy

Assets are same-origin, manifest-declared, checksum-validated, and bounded before GPU creation.
Malformed/missing assets use an explicit generic placeholder and diagnostic; they cannot change
selection proxies, fog, ownership, command behavior, or stop the frame. Optional decoders are
absent unless separately vendored and declared.

The required production-representative fixture is one repository-authored neutral tracked vehicle,
generated deterministically by `scripts/art/generate-render3d-foundation-glb.mjs`. It contains a
hull, turret, independently articulated barrel, team-color material slot, named muzzle/selection/HP
anchors, visible bounds, and a shadow proxy. Primitive source parameters, provenance/repository
license, manifest, and GLB are checked in; regeneration is byte-identical and network-free. It is
contract-complexity evidence, not final art. Its semantic hull is exactly `50.4` world px long and
`28.8` world px wide, matching the mirrored tracked-vehicle envelope already used by the client;
decorative parts and the shadow proxy must stay within declared visual bounds but never alter that
semantic size. No third-party or AI-generated art may substitute.

Phase 7 owns the numeric world-to-scene scale and derives it once per static-map generation from the
actual map received by the client; it is not a build-time constant or a scan of the server's
possibly overridden map directory. Choose the largest value in `{1, 1/2, 1/4, ...}` that keeps
`max(widthPx,heightPx) * scale < 4096` while keeping the locked `50.4 * scale >= 0.5`. If no value
satisfies both, that map is an actionable bounded backend incompatibility rather than a silent
clamp or Pixi fallback. Phase 13 owns fixture generation and final manifest values, not the scale
input dimensions.

## 10. Reproducible benchmark contracts

The committed schema will live at `scripts/rendering-benchmark.schema-v1.json`; generated reports
live only under `target/rendering-benchmarks/`. Every scenario uses authoritative Lab Interact
`open` with map `No Terrain`, scenario `blank`, seed `3303`, viewport `1000x700`, DPR `1`, clean
presentation, paused room time, camera focus `{x:2048,y:2048}` with `framingScale:1`, Babylon fixed
perspective, and quality `medium` unless the scenario says otherwise. Bulk spawns are one atomic
request, aliases are deterministic, and readiness requires selected backend id, no fallback/error,
expected subject/counts, matching generation/revisions, and two consecutive stable presented
frames.

| Id | Exact setup and expected workload |
| --- | --- |
| `quiet` | 12 entities: kinds `[worker,rifleman,machine_gunner,anti_tank_gun,scout_car,tank]`; owner-1 kind `i` is at `(1664+64*i,1984)` and owner-2 kind `i` at `(2432-64*i,2112)`. No active effects; fog reveal-all for benchmark presentation only. |
| `dense-placeholders` | 240 entities in a 20x12 grid, 56 world px x-spacing and 40 world px y-spacing, origin `(1516,1828)`, alternating owners by cell and kinds `[rifleman,machine_gunner,scout_car,tank]` by row. This scenario alone uses `framingScale:0.8`, keeping all 240 collision-safe generic entities visible in the fixed viewport. No effects; reveal-all. Maximum visible generic entity count is 240. |
| `active-effects` | 64 riflemen, 32 per owner in opposing 8x4 grids at origins `(1792,1904)` and `(2208,2096)`, 24 px spacing. Admit exactly 64 simultaneous normalized 240 ms attack events at capture offset 80 ms; this declared maximum sets the Phase 11.5 pool capacity. |
| `fog-overlays` | 24 entities. Per owner: rifle `i=0..7` is a 4x2 grid with `(x,y)=(1856+32*(i%4),1920+32*floor(i/4))` for owner 1 and its point reflection through `(2048,2048)` for owner 2; tanks are `(1888,2016),(1952,2016)` and reflected; barracks are `(1792,2112)` and `(2304,1984)`; workers are `(1952,2112)` and `(2144,1984)`. Ordinary player-1 fog; one remembered enemy barracks, one below-fog intel proxy, one shot reveal, one selection/range/HP set, one move line/destination/entity marker, one valid and one invalid placement footprint, one Lab area preview, one marquee, and one 240 ms attack event. |
| `lifecycle` | Construct, present `quiet` for 60 frames, destroy, and return to the same-page baseline three times in Phase 11 and ten times in Phase 13.5. Each cycle asserts canvas/context/rAF/listener/registry/pool/pending-load counts. |
| `vegetation` | `quiet` plus 2,048 deterministic candidate plants from seed 3303 within the map, rejecting occupied/impassable cells. Admitted counts are exactly floor(`2048*density`): off 0, low 614, medium 1228, high 2048; no shadows. |
| `vegetation-shadows` | Exact `vegetation` candidates plus locked shadow tiers; visible current `fogGatedWorld` entities only may cast. |
| `representative-asset` | `fog-overlays`, replacing exactly one owner-1 tank placeholder with the tracked fixture; compare asset and placeholder in the same medium-tier scene. |

Spawn coordinates are row-major and exact. Failure to admit any named spawn is a scenario/readiness
failure; the launcher must not shift, drop, or substitute content.

Warmup is 120 presented frames, followed by 600 sampled authoritative Match frames. Each report
contains three fresh-browser repetitions; structural values use per-frame maximum plus repetition
maximum, while timing reports repetition median and p95. Lifecycle uses its explicit 60-frame
cycle instead. Reports record schema id/version, commit, scenario definition hash, map/seed,
resolved entity/effect counts, projection/camera, viewport/DPR, quality tier, warmup/sample/repeat,
browser/version, OS, GPU/driver/API, backend/runtime, and fallback/readiness state.

Counters are current-authoritative-frame draw calls, active meshes, hardware instances, thin
instances, triangles submitted, unique/live materials, textures and estimated texture bytes,
active particles, shadow casters/map updates, and registry live/pending resources. Each also names
whether it is gauge, current-frame delta, or cumulative diagnostic. Babylon instrumentation resets
exactly once at Match-frame start; cumulative counters never masquerade as current-frame values.
Timing includes total frame and `scene.render` median/p95. Teardown records canvas, context, rAF,
listener, timer, registry, pool lease, and pending-load counts against the pre-run baseline.

No Phase 0/PoC number is a budget. Phase 11.5 sets each structural ceiling from three optimized
repetitions as `maxObserved + max(1, ceil(maxObserved * 0.10))`. Same-device timing regression over
20% is a warning/evidence field, not an absolute CI gate. `off/low/medium/high` vegetation factors
are `0/.30/.60/1.00`; shadow starting caps are `0/0/never`, `512/32/every 4th frame`,
`1024/64/every 2nd frame`, and `2048/128/every frame`. Later measurement may only reduce optional
work without a reviewed plan.

## 11. Historical PoC leads

The deleted proof-of-concept implementation is intentionally unavailable and must not be recovered
from Git history, reflogs, caches, old worktrees, PR patches, artifacts, or another clone. Its only
non-binding leads are that shared particle-texture disposal may fail on a later rematch, Babylon
draw counters may accumulate when its engine loop is bypassed, and a realistic 200 ms effect may
be hard to capture asynchronously. Repeated lifecycle assertions, once-per-frame counter reset, and
retained real-event capture test those risks from current code. No historical mesh/draw/triangle
count is a baseline, target, or evidence.
