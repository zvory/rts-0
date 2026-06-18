## 4. JS client — modules & exported APIs

`client/` (ES modules, no bundler; `index.html` imports `src/main.js` as a module).
PixiJS is loaded globally from CDN as `PIXI`.

```
index.html        # PINNED — CDN + #app + module entry + screens markup
map-editor.html   # standalone handcrafted-map editor; loads/saves server map JSON
styles.css        # HUD, lobby, menus, command card
src/
  protocol.js     # PINNED — message tag constants + builder helpers (mirror of §2)
  config.js       # PINNED — render/UI constants: colors, sizes, costs, sight (mirror balance)
  net.js          # Net: WebSocket wrapper, typed send helpers, event emitter
  prediction_controller.js # PredictionController: local command sequence/buffer bookkeeping
  prediction_compatibility.js # server/client prediction-build compatibility guard
  prediction_settings.js # localStorage-backed prediction toggle
  sim_wasm_adapter.js # optional WASM prediction adapter
  state.js        # GameState: holds prev+current snapshot, selection, control groups, display overlays
  client_intent.js # ClientIntent: browser-local placement, command targeting, previews, feedback
  command_budget.js # client mirror of command-supply selection admission and outgoing command guard
  progress_extrapolator.js # local display extrapolation for active construction progress
  camera.js       # Camera: pan/zoom, world<->screen transforms, edge/keyboard/pointer-lock scroll
  renderer/       # Pixi app facade plus layers, terrain, entities, units, buildings,
                  # resources, fog overlay, feedback, rig schema/import, and renderer-local palette helpers
  renderer/feedback_view_model.js # Builder for renderer feedback's narrow per-frame read model
  fog.js          # Fog overlay: accumulate explored, compute visible from own entities
  input/          # lifecycle facade plus selection, commands, placement, shared camera navigation, UI input routing
  audio.js        # Audio: Web Audio context, buses, one-shots, spatialization
  hud.js          # HUD: resources/supply bar, selected panel, command card (build/train)
  resource_icons.js # Shared DOM resource icon helpers for HUD and observer analysis
  minimap.js      # Minimap: draw terrain+entities+viewport; click to move camera/command
  lobby.js        # Lobby screen controller: name/room, ready/start, host controls
  lobby_view.js   # Lobby roster renderer: team columns, seat rows, spectators
  scoreboard.js   # Shared score/result formatting helpers
  observer_analysis_overlay.js # replay/live spectator analysis overlay
  match_health.js # match network/render health reporter
  branch_staging.js # replay branch staging panel
  lab_client.js  # LabClient: lab request ids, pending results, state/result subscriptions
  lab_panel.js   # LabPanel: app-owned lab controls/status UI mounted around Match
  lab_control_policy.js # Lab control collaborator placeholder injected into Match
  settings_container.js # Reusable settings shell: opener, tabs, focus, teardown
  settings_panels.js # Portable settings tab panel descriptors
  main.js         # Entry point: starts App
  app.js          # Lobby/app shell lifecycle and persistent Net/Audio ownership
  match.js        # Match lifecycle, module dependency wiring, render loop, transient events
  replay_controls.js # Replay/scenario speed, seek, vision, and timeline controls
  alerts.js       # Notice/toast alert ids and viewport alert behavior constants
  bootstrap.js    # DOM lookup, ws/dev-watch/lab launch config, startup helpers
```

### 4.1 Module export contracts

`net.js`
```js
export class Net {
  constructor(url)                       // ws url; auto-derived from location in main.js
  connect(): Promise<void>
  on(type, handler)                      // type ∈ ServerMessage tags + "open"/"close"
  off(type, handler)
  join(name, room, spectator?, replayOk?)
  ready(isReady)
  start()
  setTeamPreset(preset)                  // deprecated compatibility command; server ignores it
  setTeam(id, teamId)                    // host-only scripted lobby team assignment
  setFaction(factionId)
  addAi(teamId?, aiProfileId?)
  setAiProfile(id, aiProfileId)
  removeAi(id)
  setQuickstart(enabled)
  setSpectator(spectator, id?)
  command(cmd, clientSeq)                // lower-level sequenced gameplay command envelope
  giveUp()
  returnToLobby()
  ping()
  netReport(report)
  setReplaySpeed(speed)                  // replay rooms and dev-watch scenarios
  stepDevTick()                          // paused dev scenarios
  seekReplay(ticksBack)                  // replay rooms; pass huge N for full reset
  seekReplayTo(tick)
  setReplayVision(vision)
  lab(requestId, op)                     // lab rooms only; request id allocated by LabClient
  requestReplayBranch()
  claimBranchSeat(playerId)
  releaseBranchSeat(playerId)
  startBranch()
  selectMap(map)
  get playerId()
  get bufferedAmount()
}
```

`prediction_controller.js`
```js
export class PredictionController {
  constructor({sendCommand, enabled, now?, commandTimeoutMs?})
  issueCommand(cmd)                      // allocates clientSeq, records pending, calls sendCommand(cmd, seq)
  applyAuthoritativeSnapshot(snapshot)   // consumes snapshot.netStatus sim-consumption ack metadata
  applySimAcknowledgement(clientSeq, serverTick?)
  recordSocketReceipt(clientSeq, detail?)// diagnostic only; does not reconcile
  recordCommandRejection(clientSeq, reason?)
  enterPredicting(), beginResync(correction?), finishResync()
  predictionDisplayOverlay()             // view data for optimistic production/rally display only
  reset({enabled?})
  debugSummary()                         // pending count/seqs, latest authoritative tick, ack/correction metrics
  get pendingCommandCount()
}
```
Live player command sources receive a `commandIssuer` seam from `Match` and call
`commandIssuer.issueCommand(cmd)`. The controller owns browser-local `clientSeq` allocation and
passes the sequenced envelope to `Net.command(cmd, clientSeq)`. Replay viewers, spectators, and
dev-watch passive viewers keep prediction disabled and do not allocate gameplay command sequence ids.
`GameState.applySnapshot` remains authoritative. Prediction display writes go through
`GameState.applyPredictionDisplayOverlay({optimisticCommands?, predictedSnapshot?, diagnostics?,
smoothCorrections?})`, so controller bookkeeping and WASM render snapshots stay outside broad
snapshot mutation. Replay viewers, spectators, and dev-watch passive viewers keep prediction
disabled and clear this overlay instead of allocating gameplay prediction state.

`renderer/rigs/schema.js`
```js
export const RIG_SCHEMA_VERSION = 1
export const REQUIRED_ANCHORS = ["origin", "selection", "hp"]
export const TINT_SLOTS = [
  "team", "team-light", "team-light-soft", "team-light-strong", "team-light-08",
  "team-light-10", "team-light-14", "team-light-24", "team-stroke",
  "team-fill-stroke", "neutral", "fixed",
]
export const GEOMETRY_TYPES = ["rect", "circle", "ellipse", "line", "polygon", "polyline", "path"]
export const ANIMATION_INPUTS = [
  "now", "teamColor", "recoilProgress", "recoilPx", "recoilKickX", "recoilKickY",
  "setupVisual", "vehicleMotion", "selected", "damaged", "shotRevealAlpha",
  "visibility", "mapTileSize", "facing", "weaponFacing", "weaponFacingCos",
  "weaponFacingSin", "weaponVisualFacing", "carriageVisualFacing",
  "weaponVisualDoubleCos", "weaponVisualDoubleSin", "weaponRecoilX", "weaponRecoilY",
  "scoutGunnerX", "scoutGunnerY", "scoutMountX", "scoutMountY", "setupVisible",
  "setupMostlyDeployed", "setupBarrelVisible", "busy", "breakthroughTicks",
  "lowOil", "oilStarved", "fuelCueVisible",
]
export const ANIMATION_PROPERTIES = [
  "transform.x", "transform.y", "transform.rotation", "transform.scaleX", "transform.scaleY",
  "transform.localX", "transform.localY", "geometry.scaleX", "geometry.scaleY",
  "alpha", "visible", "tintSlot",
]
export function validateRigDefinition(definition, options?)
  // Pure validator. Returns { ok: true, definition, errors: [] } or { ok: false, errors }.
  // options.expectedKind rejects rigs whose kind does not match the importer/runtime caller.
```
Normalized rig definitions are plain objects with `id`, `kind`, `schemaVersion`, ordered `parts`,
semantic `anchors`, semantic `bounds`, optional `animations`, and `requiredRuntimeInputs`. Parts
use stable ids, integer `drawOrder`, normalized primitive geometry, local `transform`, `pivot`, one
tint slot, and optional normalized paint `{fill, stroke, strokeWidth, opacity}` for SVG-authored
literal colors. The validator is independent of Pixi and SVG DOM APIs; it fails closed with
path-addressed structured errors for missing required anchors, duplicate part ids, unsupported
geometry or transforms, non-finite coordinates, invalid tint slots or paint, invalid animation
bindings, and unit-kind mismatches.

`renderer/rigs/svg_importer.js`
```js
export function compileSvgRig(svgText, metadata?)
  // Pure SVG authoring importer. Returns validated normalized rig data or structured errors.
  // metadata.id/kind may override the authored id/kind; metadata.expectedKind enforces callers.
```
The SVG importer accepts only the Phase 3 authored rig subset: root `<svg>` with `viewBox`,
`data-rts-rig-kind`, `data-rts-rig-version="1"`, and `data-rts-origin="center"`; geometry elements
`g`, `path`, `polygon`, `polyline`, `rect`, `circle`, `ellipse`, `line`, and `metadata`; direct
hex `fill`/`stroke`, numeric `stroke-width`/`opacity`, `data-rts-tint`, `data-rts-pivot`, and
semicolon-separated `data-rts-animation` bindings. Part ids use `part.*`, anchors use `anchor.*`,
and bounds use `bounds.*`; required anchors remain `origin`, `selection`, and `hp`, with weapon
fixtures adding semantic anchors such as `muzzle`, `bipod`, or `turret`. The importer rejects
scripts, foreign objects, images/use/external hrefs, filters, masks, clip paths, gradients,
patterns, CSS classes or style attributes, percentage units, duplicate ids, lowercase or unsupported
path commands, non-finite values, and transforms that cannot decompose into translate/rotate/scale.

`renderer/rigs/animation.js`
```js
export function createRigRenderContext(entity, options?)
export function sampleRigAnimation(definition, entity, renderContext?)
```
Rig animation sampling is pure data math: it derives a narrow render context from existing client
entity state and renderer-local visual state, then applies normalized animation bindings to part
transforms, alpha, visibility, and tint slots without creating Pixi objects. The sampler accepts
only the schema-approved runtime inputs such as `facing`, `weaponFacing`, `recoilProgress`,
`setupVisual`, `vehicleMotion`, Scout Car gunner/mount offsets, selected/damaged flags,
shot-reveal alpha, map tile size, worker busy state, breakthrough ticks, and oil cue flags.

`renderer/rigs/runtime.js`
```js
export function createDefaultPixiFactory(pixi?)
export function createUnitRigInstance(kind, definition, pixiFactory?)
export function renderLiveUnitRig(renderer, entity, colorByOwner, state, definition, options?)
export function renderRigLegacyComparison(renderer, entity, colorByOwner, state, definition)
export class UnitRigInstance {
  update(entity, renderContext)
  destroy()
}
```
`UnitRigInstance` owns one Pixi container and one graphics child per normalized rig part, redraws
primitive geometry with sampled transforms and tint slots, and tears down all owned children through
`destroy()`. Live rig routing is per-kind through `_liveRigDefinitionsByKind` and currently enables
Worker, Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team, Artillery, Scout Car, Tank,
Command Car, and Ekat; missing or invalid definitions fall back to legacy procedural drawing. Temporary
SVG migration guardrails live in `tests/fixtures/svg/unit_migration_manifests.mjs` and
`tests/svg_migration_guardrails.mjs`; a live-routed kind must have a manifest and passing
part-level plus full-composition pixel gates before it is added. Shadow and body parts route
through separate live pools so normal unit and shot-reveal layer ordering stays intact.
`renderer/units.js` also keeps a test-gated side-by-side comparison seam: `_rigComparisonEnabled`
must be set and a definition must exist in `_rigDefinitionsByKind`; otherwise comparison rendering
stays dormant.

`renderer/feedback_view_model.js`
```js
export function buildRendererFeedbackView(state, options?)
  // Per-frame read model builder. Returns placement, command feedback,
  // selected entities, resource mining previews, support-weapon previews,
  // ability target previews, ability objects, smokes, transient projectile/
  // target markers, relationship helpers, and entity lookup for renderer
  // feedback drawing without exposing the full mutable GameState.
```

`branch_staging.js`
```js
export class BranchStaging {
  constructor(rootEl, net)
  show()
  hide()
  destroy()
  render(msg)                            // branchStaging payload
}
```

`observer_analysis_overlay.js`
```js
export const OBSERVER_ANALYSIS_TABS
shouldMountObserverAnalysisOverlay({ payload, replayViewer })
createObserverAnalysisOverlayPreferences(storage?)
export class ObserverAnalysisOverlay {
  constructor({ root, preferences, getEntities, getCameraBounds, getPlayers, stats })
  applyObserverAnalysis(payload)            // renders server-backed production, unit, and losses tabs
  update()                                // refreshes viewport army value from camera/snapshot state
  destroy()
}
```
`App` owns one observer analysis preference object and passes it through replay and live spectator
`Match` rebuilds so selected tab, visible state, and collapsed state survive replay
seek-triggered `start` messages and spectator rematches. Preferences are stored under
`rts.observerAnalysisOverlay`; clients still read the old `rts.replayAnalysisOverlay` key for
compatibility. The overlay owns its generated DOM and is read-only. The Army Value tab is
client-side and viewport-specific; Production, Units, Units Lost, and Resources Lost render the
latest server-authored `replayAnalysis` payload. Resources Lost follows the protocol's narrow
definition: spent steel/oil value of units that died, excluding buildings, stockpile changes,
harvesting, refunds, and cancelled queues.

`settings_container.js`
```js
export class SettingsContainer {
  constructor({ button, menu, title })
  setContext({ kind, spectator, replay, actions, tabs }) // mounts context-specific tabs/actions
  setTabs(tabs)                         // [{id,label,visible,render(panel, context)}]
  open({ focus }), close({ restoreFocus }), toggle()
  isOpen()
  destroy()
}
```

`settings_panels.js`
```js
buildSettingsTabs({ audio, hotkeyProfiles, game, debug })
buildGiveUpAction({ visible, onOpen })
```

`lab_client.js`
```js
export class LabClient {
  constructor(net, options?)
  setInitialState(state)
  subscribeState(handler)                // returns unsubscribe
  subscribeResult(handler)               // returns unsubscribe
  setVision(vision)                      // sends {op:"setVision", vision}
  request(op, options?)                  // allocates requestId, resolves with labResult/timeout
  destroy()
}
export function labVisionLabel(vision)
export const labVision                   // fullWorld(), team(teamId), teams(teamIds)
```

`lab_panel.js`
```js
export class LabPanel {
  constructor({ root, labClient, launch, startPayload })
  destroy()
}
```

`lab_control_policy.js`
```js
export function createLabControlPolicy({ labClient, metadata })
export function createDefaultControlPolicy()
```

`App` owns `LabClient`, `LabPanel`, and lab control policy lifetimes when a `start` payload carries
`lab` metadata. `Match` receives `labMetadata`, `labClient`, and `labControlPolicy` through
constructor options only; renderer, HUD, input, and minimap do not import lab modules. The shipped
MVP exposes room-local vision, setup mutations, issue-as commands, and scenario import/export
through those collaborators while keeping the normal match screen authentic.

`hotkey_profiles.js`
```js
export class HotkeyProfileService {
  constructor({storage?, catalog?, profilesKey?, activeKey?})
  allProfiles()
  getActiveProfile()
  profileById(id)
  setActiveProfile(id)
  createCustomFromPreset(presetId, metadata?)
  saveCustomProfile(profile)
  validateDraftProfile(profile)
  runtimeDiagnostics(profile?)
  importProfile(payload, {targetId?, activate?}?)
  exportProfile(id?)
  exportProfileJson(id?)
  parseImportText(text, options?)
  resolveCard(card, profile?)
  resolveSlot(slot, profile?)
}

buildHotkeyCommandCatalog(cards)
normalizeHotkey(value)
```

Exported hotkey JSON is intentionally client-local: `schemaVersion`, `profileId`, `mode`, `name`,
`description`, `createdWithBuild`, `basePreset`, `bindings`, and `factionBindings`. Direct-mode
`bindings` hold global commands such as `unit.move`, `unit.attack`, `unit.holdPosition`,
`unit.stop`, `worker.buildMenu`, `worker.return`, support-weapon setup, and production cancel. Faction catalog
actions are stored under `factionBindings[factionId]` with namespaced command ids shaped as
`kriegsia.build.<kind>`, `kriegsia.train.<kind>`, `kriegsia.research.<upgrade>`, and
`kriegsia.ability.<ability>`. Ekat uses the same `ekat.*` namespace for its exposed ability
commands, currently `ekat.ability.ekatTeleport`, `ekat.ability.ekatLineShot`, and
`ekat.ability.ekatMagicAnchor`. Imports migrate old flat Kriegsia ids like `build.city_centre`
into the Kriegsia binding set, preserve structurally valid unavailable faction commands with
warnings, ignore unknown non-faction commands with warnings, reject invalid keys and same-context
duplicates, and store accepted payloads as custom profiles. Untargeted imports rewrite ids/names to
avoid local collisions; targeted imports replace the whole target profile payload instead of
merging individual bindings.

The long-lived `SettingsContainer` is constructed by `App` with `#settings-button` and the
`#settings-menu` mount point. `App` mounts the lobby context; `Match`/`ReplayViewer` remount live,
spectator, and replay contexts through dependency-injected collaborators. The stable rendered ids
inside the settings mount point are `#pointer-lock-toggle`, `#debug-path-toggle`, and
`#give-up-open`; they may not exist until their owning tab/action is visible.

`state.js`
```js
export class GameState {
  playerId
  startInfo                              // §2.3 payload
  map                                    // {width,height,tileSize,terrain}
  players                                // [{id,teamId,name,color,startTileX,startTileY}]
  playerById(id)
  teamIdForPlayer(id)
  isOwnOwner(owner)
  isAllyOwner(owner)
  isEnemyOwner(owner)
  isNeutralOwner(owner)
  // snapshot buffering for interpolation:
  applySnapshot(msg)                     // pushes msg, keeps prev+current, stamps recvTime
  entitiesInterpolated(alpha)            // -> entities with lerped x,y,facing,weaponFacing
  get prevRecvTime() / get currRecvTime()// recv timestamps of the two buffered snapshots
                                         //   (null until two exist); main.js derives interp alpha
  resources                             // {steel,oil,supplyUsed,supplyCap} (latest)
  events                                 // latest snapshot's events
  // selection (client-only):
  selection                              // Set<entityId>; playable own selections are admitted by command supply
  selectionBudgetOverflow               // null | {used, cap, seq}; short-lived HUD feedback after ignored overflow
  setSelection(ids), addToSelection(ids), clearSelection()
  selectedEntities()                     // resolved entity objects from current snapshot
  entityById(id)
  // control groups (client-only):
  controlGroups                          // ten budget-admitted Array<entityId> slots; slot 9 maps to key 0
  setControlGroup(slot, ids), addToControlGroup(slot, ids)
  selectControlGroup(slot), controlGroupEntities(slot)
  setOptimisticCommandState(state)        // production/rally optimism display overlay
  setPredictedSnapshot(snapshot, diagnostics, options), clearPredictedSnapshot()
}
```

`client_intent.js`
```js
export class ClientIntent {
  placement                              // null | { building, valid, tileX, tileY, lineSites? }
  commandCardMode                        // null | "workerBuild"
  openWorkerBuildMenu(), closeCommandCardMenu()
  beginPlacement(buildingKind), updatePlacement(tileX,tileY,valid,options?), endPlacement()
  commandTarget                          // null | "move" | "attack" | "setupAntiTankGuns" | ability target object
  beginCommandTarget(kind, options), issueCommandTarget(ev), endCommandTarget()
  holdCommandTarget(kind, key, shiftKey), releaseCommandTargetKey(key, shiftKey)
  releaseCommandTargetShift()
  commandFeedback, liveCommandFeedback(now)
  resourceMiningPreview, updateResourceMiningPreview(preview)
  antiTankGunSetupPreview, updateAntiTankGunSetupPreview(preview)
  abilityTargetPreview, updateAbilityTargetPreview(preview)
}
```

#### Client Boundary Migration Target

`Match` remains the app-shell composer and owner of cross-area dependency injection. It constructs
`GameState` for authoritative snapshot display data and constructs `ClientIntent` for browser-local
cursor/command intent, then injects the intent facade into HUD, input, minimap, and renderer
feedback. Runtime modules should not gain direct imports across the model, input, UI, minimap,
renderer, and prediction areas except for pinned mirrors such as `protocol.js` and `config.js`, or
for explicitly documented architecture-check exceptions.

`GameState` is the authoritative browser view of server snapshots, interpolation, selected ids,
control groups, relationship helpers, fog-facing visibility data, and display overlays derived from
authoritative snapshots. `ClientIntent` owns placement intent, command-card submenu state,
command-target arming, hover previews, command feedback, and ability previews. `GameState` must not
grow compatibility accessors for those intent fields; HUD, input, minimap, and renderer feedback
use the injected facade or a narrow read model.

Renderer feedback should consume a narrow read model containing placement, command feedback,
support-weapon setup previews, ability targeting previews, ability objects, and selected entities,
rather than relying on the full mutable `GameState`. HUD and input should exchange command intent
through descriptor/facade methods, while gameplay command emission continues to flow through
`commandIssuer.issueCommand`. `PredictionController` owns client sequence allocation and optimistic
bookkeeping; `GameState` applies named display overlays but does not own prediction policy.

`camera.js`
```js
export class Camera {
  x, y, zoom                             // world coords of viewport top-left, zoom factor
  update(dt, input)                      // apply pan (keys/edge/virtual pointer-lock cursor) & clamp
  worldToScreen(wx, wy) -> {x,y}
  screenToWorld(sx, sy) -> {x,y}
  centerOn(wx, wy)
  setBounds(worldW, worldH, viewW, viewH)
}
```

`renderer/index.js`
```js
export class Renderer {
  constructor(canvasParent)              // creates PIXI.Application, layers
  resize(w,h)
  buildStaticMap(map)                    // draw terrain once into a cached layer
  render(state, camera, fog, alpha)      // per-frame; draws entities, fog, selection, placement
  app                                    // the PIXI.Application (for ticker/stage if needed)
  // exposes screen->world hit info if helpful; selection box drawing lives here too:
  drawSelectionBox(rectOrNull)
}
```

`fog.js`
```js
export class Fog {
  constructor(mapWidth, mapHeight)
  update(ownEntities, tileSize, serverVisibleTiles?) // copy server visibility when provided; accumulate explored
  isVisible(tileX,tileY), isExplored(tileX,tileY)
  // renderer reads the grids to draw the black/dim overlay
  visibleGrid, exploredGrid              // Uint8Array length w*h
}
```
`match.js` must exclude `visionOnly` and shot-reveal entities from `ownEntities` before calling
`fog.update`; those views are rendered as intel, not as local fog sources. Normal match snapshots
provide `visibleTiles`, so the overlay follows server-authoritative fog including smoke blockers;
local stamping remains a fallback for older/dev object snapshots.

Playable own selections and human multi-unit commands use the mirrored command-supply budget from
`command_budget.js`: 24 base command supply plus 12 and the Command Car's own command weight per
admitted Command Car, with unit supply as weight and a fallback weight of 1. Drag selection,
shift-add, double-click same-kind selection, and control-group save/add/recall preserve their normal
candidate ordering, except Command Cars in the
candidate set are admitted first so their budget bonus is reliable. Overflow candidates are ignored
client-side and surface `selectionBudgetOverflow` for the HUD; outgoing commands that still exceed
the budget are blocked before `Net.command`.

`input/index.js`
```js
export class Input {
  constructor(domElement, camera, state, commandIssuer, renderer, fog, audio?, inputRouter?, hotkeyProfiles?, clientIntent?)
  // installs listeners; translates gestures into selection + protocol commands.
  // number keys recall control groups; double-tap jumps the camera to the largest
  // local cluster. Alt/Ctrl/Cmd+number replaces a group, Shift+number adds to it.
  // On Windows, tabbed browser saves use Alt+number and installed-app saves use Ctrl+number.
  // optional pointer-lock mode traps the browser cursor and drives a visible
  // virtual cursor for edge pan on multi-monitor setups.
  update(dt)                             // continuous handling (edge scroll handled by camera)
  // emits nothing to return; mutates state.selection / clientIntent and calls commandIssuer.issueCommand
}
```
`input/camera_navigation.js`
```js
export class CameraNavigationInput {
  constructor(domElement, camera, options?)
  // shared command-free camera gesture state for live input and replay/observer wrappers:
  // viewport mouse tracking, mouse-wheel cursor-anchored zoom, configured pan keys,
  // middle-mouse drag panning, optional Space+left-drag panning, blur release, and teardown.
  // exposes keys + mouse for Camera.update(dt, input)
  static replayPanKeyCodes()
  install()
  destroy()
}
```
Live `Input` composes `CameraNavigationInput` for camera gestures, then layers pointer lock,
selection, placement, command-card targeting, command hotkeys, minimap routing, and gameplay command
issuance on top. Replay viewers use the same helper through `ReplayCameraInput`, with replay WASD
pan-key aliases and no gameplay command issuer API. Replay middle-drag and Space+left-drag pan
through `Camera.panByScreenDelta`; mouse-wheel zoom, keyboard pan state, edge scroll state, and blur
release are shared observer navigation behavior. Live spectators still use the live `Input` path so
read-only selection inspection remains available while command emission stays gated by local-owner
checks.

Shift-right-click appends queued orders only for selected units: move, attack-move, attack,
gather, build/resume, and placement build commands set `queued: true` and rely on the server
snapshot's owner-only `orderPlan` for accepted markers. Production-building-only right-clicks set
or append building rally stages and rely on owner-only `rallyPlan` for accepted markers. Attack
targeting with only production buildings selected creates `attackMove` rally stages.
Selection and targeting use `GameState` relationship helpers where the distinction is own/ally/enemy:
single-click may select an allied entity for read-only inspection, box selection and same-kind
selection stay own-only, and right-clicking own or allied entities with own units selected falls
through to ordinary move-to-point behavior instead of attack. Command emission, prediction,
optimistic production/rally, control groups, build/gather/train/research/cancel, and ability
execution remain strict local-owner checks.
Shift-confirmed build placement keeps placement mode armed while Shift is physically held, allowing
multiple queued building placements; releasing Shift or losing window focus clears placement mode.
Tank Trap placement uses the same local placement intent, with optional `lineSites` preview data:
the first valid sites dispatch as one immediate single-worker build per selected worker, and any
remaining valid sites dispatch as queued standard build commands against the selected worker set.
Line placement only offers vehicle-closing Tank Trap steps: exact diagonal adjacency `(1,1)` or
one-tile orthogonal gaps `(2,0)` / `(0,2)`. Invalid intermediate sites break the line instead of
letting dispatch skip ahead across a larger gap. The renderer draws Tank Traps larger than their
1x1 build footprint so these sparse vehicle-blocking gaps read as closed barrier segments.

`command_composer.js` owns command-target arming lifetime for command-card targets. HUD, input, and
minimap receive `ClientIntent` from `Match`; input and minimap clicks call
`ClientIntent.issueCommandTarget`, so held keys, Shift preservation, and repeated queued target
clicks use one composer path instead of command-specific sticky flags. A plain
targeted-order command-card hotkey tap arms the target after keyup; pressing the same resolved
hotkey again inside the quick-cast window issues it at the current cursor world point. Shift does
the same with `queued: true` and keeps the target armed until Shift is released.

`input/router.js`
```js
export class MatchInputRouter {
  constructor(viewportEl)
  registerZone(zone)                     // zone: {priority?, contains(ev), pointerDown?, pointerMove?, pointerUp?}
                                         // returns unregister()
  pointerDown(ev) -> boolean             // routes to highest-priority matching zone
  pointerMove(ev) -> boolean             // captured zone receives moves until release
  pointerUp(ev) -> boolean               // releases capture after the originating source handles up
}
```
Router events carry `viewportX`/`viewportY` plus `clientX`/`clientY`; pointer-lock input and DOM
input use the same zone contract so HUD interactions can work while the browser routes mouse events
to the locked viewport.

`audio.js`
```js
export class Audio {
  preload(manifest): Promise<void>        // decode sounds once the AudioContext is unlocked
  unlockFromGesture(ev?) -> Promise<boolean>
                                          // create/resume AudioContext from a user gesture
  isUnlocked() -> boolean                 // true when the AudioContext is running
  onUnlockChange(fn) -> unsubscribe       // notify settings UI after first successful unlock
  play(id, {x?, y?, priority?, category?, pitchVariance?, gain?})
                                           // x/y present -> StereoPanner + lowpass + distance gain
  playUI(id, opts)                        // non-spatial ui category convenience
  stopByKey(key) -> number                // stop tagged active voices, for sustained/abortable cues
  setListener(x, y, zoom, viewW?)          // camera center in world px; derives screen-width refDist
  pickVariant(ids) -> id|null             // seeded RNG variant choice
  setMasterVolume(v), getMasterVolume()
  setCategoryVolume(cat, v), getCategoryVolume(cat)
  destroy()
}
export const SOUND_MANIFEST
export function noticeSoundId(msg)
```

`hud.js`
```js
export class HUD {
  constructor(rootEl, state, commandIssuer, audio?, hotkeyProfiles?, clientIntent?)
  update()                               // refresh resources/supply, selected panel, command card
  // command card buttons call commandIssuer.issueCommand(...) or ClientIntent facade methods
}
```
The train command card is driven by the first selected production building type, but train clicks
are issued to the selected completed compatible production buildings in round-robin order so a
multi-building selection spreads queued units across its producers. Train and production-cancel
hotkeys honor native keyboard repeat: after the OS repeat delay, repeated `keydown` events activate
only those repeatable command-card buttons. Research buttons that unlock production appear directly
below the production button they unlock and disappear once complete. Cancel walks selected producing
buildings in reverse round-robin order for the displayed producer type.
Command identities are stable and split by scope: global tactical/navigation/production-control
buttons remain un-namespaced, while build, train, research, and ability buttons emitted for a
faction catalog use the local player's faction id as the command-id prefix.
`config.js` exposes the client-side faction catalog mirror used by command-card descriptors:
`workerBuildablesForFaction`, `trainableUnitsForFaction`, `researchableUpgradesForFaction`, and
`commandCardAbilitiesForFaction`. `scripts/check-faction-catalog-parity.mjs` compares those
descriptors with the Rust catalog dump for every client-exposed faction. Unknown valid faction ids
fail closed in command-card data, so future factions do not inherit Kriegsia build, train, research,
or ability buttons before their catalog is intentionally exposed. The client mirror is a checked
projection, not lifecycle admission: lobby selectors must expose only playable human choices,
fixture-only ids remain test harness data, public AI controls do not expose a faction selector, and
local prediction remains disabled for unsupported local faction ids such as the current Ekat slice.
Generation is not required as long as the parity check remains a required gate comparing every
client-exposed descriptor against the Rust dump.

`minimap.js`
```js
export class Minimap {
  constructor(canvasEl, state, camera, fog, commandIssuer, inputRouter?, {clientIntent?, commandsEnabled?})
  render()                               // draw terrain + fog + entity blips + viewport rect
  inputZone()                            // router zone for locked/unlocked minimap interaction
  // click/drag -> camera.centerOn or issue move command (right-click)
}
```

`lobby.js`
```js
export class Lobby {
  constructor(rootEl, net)
  show(), hide()
  // owns lobby state, joins, ready/start/spectator role, and delegates roster DOM to lobby_view.js.
  // Host lobby controls expose grouped team cards, per-seat team assignment, team-scoped AI add
  // buttons, and a map selector in the lobby summary row through Net setTeam/addAi/selectMap.
  // Teams are layout groups only; player colors come from each player record.
  onGameStart(cb)                        // main.js subscribes to transition to game screen
}
```

`main.js` starts `App`; `app.js` owns the persistent `Net` and `Audio`, derives the ws url from
`window.location`, and shows `Lobby`; on `start` it creates `Match`. `match.js` builds
`GameState`, `ClientIntent`, `Camera`, `Renderer`, `Fog`, `HUD`, `MatchInputRouter`, `Minimap`,
`Input`, starts the rAF loop
(compute `alpha` from snapshot timing, `camera.update`,
`audio.setListener`, `input.update`, `fog.update`, `renderer.render`, `hud.update`,
`minimap.render`); on each snapshot it applies state and triggers transient event audio exactly
once; on `gameOver` show the victory/defeat overlay with the frozen score table. The score table
includes a Team column, highlights every row matching `winnerTeamId`, and falls back to `winnerId`
for singleton FFA compatibility.
For spectator starts, `match.js` hides the command card and give-up action, computes local fog from
the server-filtered union snapshot, and keeps the ordinary renderer/minimap/HUD pointed at snapshots
with `playerResources`.

### 4.1a Targeted ability mode (Smoke, Mortar Fire, Point Fire)

`input/commands.js` exposes `_onAbilityTarget` and `_refreshAbilityTargetPreview` for world-point
abilities. When the HUD command card calls `ClientIntent.beginCommandTarget({ kind: "ability", ability })`,
the input module enters targeted cursor mode:
- Pointer moves call `_refreshAbilityTargetPreview`: compute which selected units are eligible
  carriers (`ABILITIES[ability].carriers`), test whether any carrier is within range of the cursor,
  update `ClientIntent.abilityTargetPreview` for renderer feedback.
- Left-click: build a `useAbility` command with the ability name, filtered carrier ids, world
  coords, and the `queued` flag (from Shift). Clear cursor mode unless the resolved command-card
  hotkey is still held for repeated world-point targeting.
- If the selected unit's owner-only ability affordance includes an active return object, the command
  card sends `recastAbility(ability, readyIds, targetObjectId, queued)` directly instead of arming a
  world-point cursor. The server remains authoritative for the availability tick and destination
  validity.
- While the resolved hotkey remains held, repeated left-clicks keep the current selection intact and
  keep targeted mode armed so multi-selected Mortar Teams and Scout Cars can distribute repeated
  point commands without the next click falling back to normal selection.
- Right-click / Escape: cancel cursor mode through `ClientIntent.endCommandTarget()`.
- Minimap right-click also fires an ability command if in targeted mode.
Selected owned Mortar Teams also draw dotted firing-range circles even when the Fire command is not
armed. The Mortar Team Fire command-card button shows an autocast swirl while any selected mortar's
owner-only `mortarFire` affordance has `autocastEnabled`; right-clicking that button sends
`setAutocast(mortarFire, enabled=false)` and does not arm manual targeting.

`client_intent.js` holds `commandTarget` (null or `{ kind, ability }`) and `abilityTargetPreview`
(null or `{ ability, mouseX, mouseY, carriers, rangeOrigins, pathOrigins, returnMarkers,
hoverInRange }`). `commandTarget` is a transient UI state; `abilityTargetPreview` is rebuilt every
mouse move from the cursor world position and the current selection. Server-projected complex
ability world objects are stored separately as `state.abilityObjects` from
`Snapshot.abilityObjects`. They are authoritative, fog-filtered data for return-marker, Magic
Anchor, and line-projectile rendering, so the client must not infer gameplay authority from local
preview state.

Range preview rendering (`renderer/feedback.js`, `_drawAbilityTargetPreview`):
- While in targeted ability mode, draws a dotted range ring (radius = `rangeTiles × tileSize`) around
  each eligible carrier.
- `rangeOrigins` keeps normal range rings tied to carrier units, while `pathOrigins` can add
  server-projected origins such as Magic Anchors for multi-origin line-shot previews.
- `returnMarkers` can draw owner-visible dash-return markers while the dash ability is armed.
- At the cursor position, draws the ability-specific target feedback: smoke uses a 2-tile cloud
  radius, Magic Anchor uses the configured anchor radius, and Ekat Line Shot draws projected path
  segments from every current origin. Feedback is colored green when in range of at least one
  carrier, grey when out of range.

Ability object rendering (`renderer/feedback.js`, `_drawAbilityObjects`; drawn on the same ground
overlay container as smoke clouds, below selection rings and HP bars):
- Each frame, iterates `state.abilityObjects` (the latest snapshot's fog-filtered object list).
- Return markers draw as small blue ground marks; Magic Anchors draw as persistent diamond-shaped
  ground objects; line-projectile/debug objects draw as small red circles when projected.
- Ability objects are never routed through entity selection, minimap blips, HUD command-card state,
  or local prediction. They disappear when absent from the next authoritative snapshot.

Smoke rendering (`renderer/feedback.js`, `_drawSmokes`; layer `smokes` between `selectionRings`
and unit layer):
- Each frame, iterates `state.smokes` (the latest snapshot's fog-filtered cloud list).
- Each cloud is rendered as layered translucent grey/white circles (overlapping offset blobs) with
  a dark semi-transparent core so the cloud reads as a LOS blocker without obscuring own unit
  selection rings or HP bars above the fog overlay.
- Transient `smokeLaunch` events add a local fast black canister arc from the scout car's launch
  position to the target point. The canister lifetime uses the server-provided `delayTicks`; the
  actual smoke still appears only when the authoritative cloud arrives in a later snapshot.
- Non-finite coordinates are skipped.
- The render layer is cleared each frame so expired clouds vanish automatically when they drop from
  the next snapshot.

### 4.2 Rendering & look (PixiJS, procedural art — neutral PS1 field-command style)
- Layers (back→front): terrain → resource nodes → building shadows → buildings → unit
  shadows → units → selection rings → health bars → fog overlay → shot-revealed units →
  command/hover feedback → placement ghost →
  selection drag-box → (HUD is DOM, not Pixi).
- Units: low-detail hard-edged silhouettes tinted by player color, with a dark drop shadow,
  dark outline, HP bar above when damaged/selected, and glowing selection ring when selected.
  Distinct silhouette per kind (engineer: compact block; rifleman / machine gunner: shared
  infantry body with oversized role weapons; Anti-Tank Gun: wheeled gun; mortar team: crewless
  M1938-inspired small wheeled mortar that travels low and deploys upright; scout car: boxy
  WW2-style truck silhouette with enclosed wheels and a rear-top machine-gunner; tank: chunky
  flat-shaded armor).
  Riflemen carry a rifle, Anti-Tank Guns field a wheeled anti-tank gun with a long recoiling barrel,
  carriage, two wheels, and animated deployment bracing, and machine gunners carry an MG42-style
  long machine gun across the body while packed that extends forward with bracing during
  setup/deployment. Units that fire from outside current vision are shown briefly above the fog
  as semi-transparent silhouettes with the same recoil animation and a yellow tracer to the hit
  point.
  Mortar launch events draw launch dust/recoil for recipients that can see the mortar, a black
  shell arcing from the mortar to the impact point, and a darker red dotted line/crosshair warning
  that lasts until the reported shell delay elapses or the impact event arrives. The shell
  compresses near launch and impact and stretches near mid-flight so it reads as an overhead round
  rather than a flat tracer. Mortar impacts draw a larger, denser, longer-lived dust cloud with an
  orange-yellow blast core that fades before the dust so battlefield state remains readable. Mortar
  Team art uses a small upright tube, slim pill-shaped wheels, and team-colored support legs.
  Mortar impact events that include a shooter reveal show the mortar briefly above fog for players
  whose units or buildings were hit by indirect fire.
  Entities marked `visionOnly` by the server are drawn on the ordinary building/unit layers below
  the fog overlay, excluded from local fog-source computation and hit-testing, and treated as
  visual intel only.
- Buildings: footprint-sized blocky field structures with neutral geometry and plain
  two-letter stencils; under construction → translucent with a progress bar; production →
  small progress arc. Owned scaffolds may locally extrapolate `buildProgress` only while the
  latest authoritative snapshot marks them `buildActive`; the display clamps below completion and
  never unlocks supply, tech, production, pathing, or command behavior before the server snapshot.
- Resource nodes: steel = tan supply crates; oil = olive fuel drums; show last-known remaining
  from `resourceDeltas` via size/opacity. When a worker is selected and the cursor hovers a
  resource, draw a blue circle on the resource when the nearest completed own City Centre
  is inside mining range; draw a red/dashed line to the City Centre when too far.
- Tanks: render the hull from mirrored client `TANK_BODY` constants (`50.4px` length, `28.8px` width,
  `1.5px` clearance) so the visible body, selection ring, click target, and advisory build
  preview match the server's oriented vehicle body. Track tread offsets advance from actual
  interpolated movement and hull turn deltas: both tracks forward/backward for drive/reverse and
  opposite track motion for pivot turns. Own tanks show a small amber/red fuel cue when oil is low
  or movement is oil-starved; the selected-entity panel also exposes lifetime movement `oilUsed`
  as `Oil Used:` when exactly one tank is selected.
- Scout cars render from mirrored client `SCOUT_CAR_BODY` constants (`40.8px` length, `21.6px` width,
  `1px` clearance), matching the authoritative oriented vehicle body used for collision and
  click targeting.
- Terrain: muted grass/field/mud, rock, and water tiles with deterministic coarse dithering
  so movement is readable and the map has a PlayStation 1-era low-resolution texture feel.
- Fog: unexplored = 80% dark overlay so terrain remains faintly readable; explored-but-not-visible =
  48% dark overlay; visible = clear. Use a single overlay sprite/graphics updated from `fog`
  grids; soften edges if cheap.
- Selection: green for own, red tint for enemy, yellow for neutral. Drag-box translucent green.
- Keep a cohesive muted palette; define colors in `config.js`.
- Art must stay faction-agnostic: no Soviet, German, Nazi, imperial, national, or unit-branch
  iconography. Avoid flags, stars, crosses, eagles, skulls, sickles, hammers, and historically
  specific insignia.

### 4.3 Client architecture workflow

Client modules are organized by area and checked by `node scripts/check-client-architecture.mjs`.
The checker classifies every `client/src/**/*.js` file, reports the largest files and fan-in/out
baseline, rejects unclassified files, and rejects cross-area imports that are not allowed by rule or
by an explicit allowlist reason in the script. It also rejects production reads or writes through
removed `GameState` intent shims such as `state.commandTarget`, `state.placement`, and preview
update methods; use injected `ClientIntent` or a renderer read model instead.

Current areas:
- `app-shell`: `main.js`, `app.js`, `match.js`, `match_health.js`,
  `observer_analysis_overlay.js`, `replay_controls.js`, `replay_viewer.js`,
  `lab_control_policy.js`.
- `model`: `state.js`, `client_intent.js`, `command_budget.js`, `command_composer.js`,
  `progress_extrapolator.js`, `prediction_controller.js`, `prediction_compatibility.js`,
  `sim_wasm_adapter.js`.
- `transport`: `net.js`, `protocol.js`, `lab_client.js`.
- `rules-mirror`: `config.js`.
- `ui`: HUD, command card, lobby controller/view, match history, minimap, resource icons,
  scoreboard, status badge, branch staging, lab panel, settings.
- `input`: `input/` plus `replay_camera_input.js`; `input/camera_navigation.js` is the shared
  command-free camera gesture helper for live input and replay/observer wrappers.
- `renderer`: `renderer/`.
- `platform`: bootstrap, audio, combat audio, alerts, fog, camera, prediction settings.

Import rules:
- `protocol.js` and `config.js` are shared mirrors and may be imported where needed.
- Files in the same area may import each other.
- `app-shell` files may compose other areas; prefer adding new cross-area wiring in `match.js` or
  `app.js` instead of importing collaborators from feature modules.
- Lab UI and transport lifetimes stay in `App`: `match.js` may receive lab metadata/control policy,
  but must not import `lab_client.js` or `lab_panel.js`.
- Non-shell cross-area imports should normally become dependency injection through `Match`, `App`,
  or a facade. If one is intentional, update `ALLOWED_CROSS_AREA_IMPORTS` in
  `scripts/check-client-architecture.mjs` with a reason.

Future client changes should use this checklist:
- Did this add a DOM/window listener, timer, WebSocket handler, Pixi object, texture, or other GPU
  resource? Add or update `destroy()`, and make sure `Match.destroy()` or the owning shell calls it.
- Did this add a non-shell cross-area import? Prefer dependency injection through `Match`/`App`; if
  an import is still the right tradeoff, add a checker allowlist reason.
- Did this change command-card behavior? Add descriptor coverage or DOM contract coverage for the
  visible command-card contract.
- Did this change rendering? Run the client smoke suite and add a targeted check where practical.
- Did this touch `protocol.js` or `config.js`? Update the mirrored server file and the relevant
  design/context docs in the same change.

Large-file handling is ratcheted, not churn-driven. Do not split a large file only to reduce byte
count. When adding behavior to an already large file, first look for a focused collaborator,
descriptor, or area-local helper that reduces coupling. Update checker baselines or allowlists only
with a written reason in the script and the change handoff.

Client-related suite selection lives in `tests/select-suites.mjs`. For `client/src/` changes it
selects `client-architecture`, `js-protocol-contracts`, `node-minimap-input-contracts`, and
`client-smoke`; client transport/protocol changes also select `node-server-integration`.
Architecture-policy files such as `scripts/check-client-architecture.mjs`,
`tests/select-suites.mjs`, and `plans/archive/client-arch/*` select `client-architecture`.
Docs-only changes select `docs-only` unless another rule applies.
