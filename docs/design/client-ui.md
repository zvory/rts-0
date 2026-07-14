## 4. JS client — modules & exported APIs

`client/` (ES modules, no bundler; `index.html` imports `src/main.js` as a module).
PixiJS is loaded globally from CDN as `PIXI`. The default backend bundle creates the existing
orthographic semantic camera and Pixi presentation adapter. The explicit live-player/Lab
`rtsRenderer=babylon` selector lazily loads the pinned Babylon dependency and creates the fixed
perspective camera before its renderer; ordinary spectators and replays remain on Pixi, and
`Match` still owns the only animation-frame loop.

```
index.html        # PINNED — CDN + #app + module entry + screens markup
styles.css        # HUD, lobby, menus, command card
assets/decals/    # SVG alpha-mask sources for client-only permanent ground decals
src/
  protocol.js     # PINNED — message tag constants + builder helpers (mirror of §2)
  config.js       # PINNED — stable public facade for render/UI constants and balance mirrors
  config/         # timing.js, presentation.js, rules_mirror.js, factions.js
  net.js          # Net: WebSocket wrapper, typed send helpers, event emitter
  report_window_aggregate.js # bounded rolling-window aggregation helper for telemetry reports
  prediction_controller.js # PredictionController: local command sequence/buffer bookkeeping
  prediction_compatibility.js # server/client prediction-build compatibility guard
  prediction_settings.js # localStorage-backed prediction toggle
  unit_range_settings.js # localStorage-backed selected-unit range overlay toggle
  sim_wasm_adapter.js # optional WASM prediction adapter
  state.js        # GameState: holds prev+current snapshot, selection, control groups, display overlays
  state_ground_decals.js # client-only death/impact decal queue, classification, owner/facing recovery, building-footprint sizing
  client_intent.js # ClientIntent: browser-local placement, command targeting, lab tools, previews, feedback
  command_budget.js # client mirror of command-supply selection admission and outgoing command guard
  progress_extrapolator.js # local display extrapolation for active construction progress
  camera.js       # Camera: pan/zoom, world<->screen transforms, edge/keyboard/pointer-lock scroll
  auto_spectator.js # spectator/replay battle director: tick-paced combat clustering and camera framing
  auto_spectator_settings.js # persisted opt-in preference for automatic spectator framing
  spectator_controls_panel.js # floating live-spectator/replay camera controls
  match_auto_spectator.js # Match availability, camera-limit, and director-construction wiring
  renderer/       # Pixi app facade plus layers, terrain, entities, units, buildings,
                  # decals, resources, fog overlay, feedback, rig schema/import, and renderer-local palette helpers
  renderer/decals.js # GroundDecalLayer permanent decal texture, stamping, diagnostics, teardown
  renderer/decals/ # SVG decal atlas manifest, loader, and deterministic stamp selection
  renderer/trenches.js # Authoritative trench terrain pass and deterministic nearby-trench connectors
  renderer/feedback_view_model.js # Builder for renderer feedback's narrow per-frame read model
  renderer/lab_tool_preview.js # Armed Lab unit/remove-tool cursor ghosts
  renderer/observer_map_analysis.js # Observer-only static AI map-analysis world overlay drawer
  fog.js          # Fog overlay: accumulate explored, compute visible from own entities
  input/          # lifecycle facade plus selection, commands, placement, shared camera navigation, UI input routing
  audio.js        # Audio: Web Audio context, buses, one-shots
  audio_spatial.js # renderer-neutral distance, pan, low-pass, and priority profiles
  sound_manifest.js # Stable sound ids and asset URLs
  hud.js          # HUD: resources/supply bar, minimap status row, selected panel, command card
  hud_command_card.js # Command-card descriptors, faction command ids, and grid hotkeys
  hud_train_card_helpers.js # Train/research command-card slotting, affordability, and one-unit limits
  hud_selection_panel.js # Selected-unit strip/details panel
  hud_unit_commands.js # Unit tactical command descriptors
  hotkey_profiles.js # Local hotkey presets, custom profile storage, import/export
  hotkey_editor.js # Settings Hotkeys tab editor
  resource_icons.js # Shared DOM resource icon helpers for HUD and observer analysis
  minimap.js      # Minimap: draw terrain+entities+viewport; click to move camera/command
  lobby.js        # Lobby screen controller: browser polling, joins, ready/start, host controls
  lobby_browser_view.js # Pre-join lobby browser rows, state rendering, and age/status formatting
  lobby_view.js   # Lobby roster renderer: team columns, seat rows, spectators
  match_history.js # Lobby match-history table and replay launch affordance
  scoreboard.js   # Shared score/result formatting helpers
  status_badge.js # Build/network/frame status badge with copyable diagnostics
  ai_diagnostics_panel.js # dedicated live/replay AI decision diagnostics panel
  observer_analysis_overlay.js # replay/live spectator analysis overlay
  observer_analysis_preferences.js # persisted observer analysis tab/visibility/window preferences
  observer_analysis_resources.js # resources tab renderer and wire normalization for observer analysis
  observer_analysis_rows.js # observer analysis player row metadata joiner
  floating_panel_positioner.js # shared app-shell move-only panel interaction and placement
  observer_analysis_signatures.js # dirty-body signatures for observer analysis DOM updates
  match_observer_diagnostics.js # Match-owned observer/AI diagnostics surface composer
  client_perf_report.js # bounded client frame-profiler upload field shaping
  match_health.js # match network/render health reporter
  frame_profiler.js # bounded client frame phase profiler and debug summary API
  live_pause_overlay.js # live-match pause state overlay and unpause affordance
  branch_staging.js # replay branch staging panel
  lab_catalog.js # LabCatalogScreen: app-owned `/lab` setup/blank selector
  interact_bridge.js # InteractBridge: launch-gated narrow local automation facade
  interact_game_bridge.js # Isolated normal-match inspection/move/surrender automation facade
  clean_presentation.js # app-shell reversible DOM chrome mode for Interact capture
  lab_client.js  # LabClient: lab request ids, pending results, state/result subscriptions
  lab_scenario_authoring.js # pure lab setup metadata defaults, slugging, and local validation
  lab_scenario_submission_capability.js # HTTP capability probe with transient-failure retry
  lab_scenario_submission_flow.js # LabPanel scenario validation/submission orchestration
  lab_panel.js   # LabPanel: app-owned lab controls/status UI mounted around Match
  lab_tool_detail.js # Pure armed-tool instruction text for LabPanel status
  lab_panel_window.js # draggable/resizable chrome helper for the app-owned LabPanel
  lab_control_policy.js # Lab control collaborator placeholder injected into Match
  visual_profiles.js # Lab-scoped visual experimentation profile registry and resolver
  settings_container.js # Reusable settings shell: opener, tabs, focus, teardown
  settings_panels.js # Portable settings tab panel descriptors
  main.js         # Entry point: starts App
  app.js          # Lobby/app shell lifecycle and persistent Net/Audio ownership
  launch_url.js   # Namespaced rtsLaunch URL parsing and pure lobby automation decisions
  map_editor_app.js # Dedicated `/map-editor` lifecycle; never constructs Net, Match, or GameState
  map_editor_launch.js # Bounded editor route/handoff/workspace query parsing
  map_editor_handoff.js # Short-lived HTTP map handoff create/consume client
  map_editor_session.js # Flat authored-map state, local storage, undo/redo, stroke transactions
  map_editor_panel.js # Dedicated editor controls for maps, terrain, start/base locations, save/export, and Lab launch
  map_editor_viewport.js # Pixi renderer/camera composition plus editor-only pointer/keyboard input
  match.js        # Match lifecycle, module dependency wiring, render loop, transient events
  match_combat_audio.js # Match-owned combat sound routing and machine-gunner sound cleanup
  match_notice_presenter.js # Match-owned existing-notice fanout and under-attack incident admission
  match_live_pause.js # live pause state actions and prediction visual suspension
  match_net_reporter.js # Match ping cadence and client net-report upload collaborator
  match_settings_context.js # Match settings action/tab context builder
  frame_recovery.js # Frame-loop soft-failure logging and rescheduling diagnostics
  visual_clock.js # Render-only normal/capture clocks; never used for networking, health, input, or timeouts
  frame_entity_views.js # One-RAF entity view builder shared by render, fog, HUD, minimap, analysis
  presentation/    # Frozen semantic layers, opaque GridSnapshot accessors, static map, and frame assembly
  replay_controls.js # Capability-driven RoomTimeControls plus replay-only vision/branch controls
  room_time_panel.js # Floating, draggable chrome around shared room-time controls
  room_capabilities.js # Client-side room capability parser for controls/diagnostics affordances
  alerts.js       # Notice/toast alert ids and viewport alert behavior constants
  bootstrap.js    # DOM lookup, ws/dev-watch/lab launch config, startup helpers
```

`/map-editor` is a separate frozen client session. `main.js` constructs `MapEditorApp` for that route,
so it never opens a WebSocket or constructs `App`, `Match`, `GameState`, Lab controls, fog, resources,
orders, replay controls, or a simulation clock. It reuses the normal Pixi `Renderer`, terrain cache,
`Camera`, map schema, and player palette through an editor-owned viewport and panel. The removed
`map-editor.html` implementation and Lab-embedded editor are not compatibility routes.

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
  addAi(teamId?, aiProfileId?)           // AI profile id
  setAiProfile(id, aiProfileId)          // AI profile id
  removeAi(id)
  setSpectator(spectator, id?)
  command(cmd, clientSeq)                // lower-level sequenced gameplay command envelope
  giveUp()
  pauseGame()
  unpauseGame()
  returnToLobby()
  ping()
  netReport(report)
  createSnapshotReportStats()
  consumeSnapshotReportStats()
  noteSnapshotFrame({bytes, parseMs, decodeMs, snapshotCodec, snapshotCodecVersion, frameKind})
  setRoomTimeSpeed(speed)                // room-controlled replay/speed-only live/dev-scenario/lab time
  stepRoomTime()                         // paused dev-scenario/lab room time
  seekRoomTime(ticksBack)                // room-controlled replay/lab time; pass huge N for full reset
  seekRoomTimeTo(tick)
  setVisionSelection(selection)
  lab(requestId, op)                     // lab rooms only; request id allocated by LabClient
  requestBranchFromTick()
  claimBranchSeat(playerId)
  releaseBranchSeat(playerId)
  startBranch()
  selectMap(map)                         // host map selection; capacity limits Add AI, spectator return, and the optional empty team target while preserving reassignment for same-team two-player lobbies
  get playerId()
  get bufferedAmount()
}
```

`prediction_controller.js`
```js
export class PredictionController {
  constructor({sendCommand, predictor?, enabled, now?, commandTimeoutMs?, uiConfirmationSnapshots?})
  issueCommand(cmd)                      // allocates clientSeq, records pending, calls sendCommand(cmd, seq)
  applyAuthoritativeSnapshot(snapshot, {allowStale?}?)
                                         // consumes snapshot.netStatus sim-consumption ack metadata
  applySimAcknowledgement(clientSeq, serverTick?)
  recordSocketReceipt(clientSeq, detail?)// diagnostic only; idempotent for duplicate receipts; does not reconcile
  recordCommandRejection(clientSeq, reason?)
  recordAckSnapshotApplied(clientSeq, snapshotReceivedAt)
  enterPredicting(), beginResync(correction?), finishResync()
  predictionDisplayOverlay()             // view data for optimistic production/rally display only
  reset({enabled?, preserveClientSeq?, reason?})
  debugSummary()                         // pending count/seqs, latest authoritative tick, ack/correction metrics
  consumeCommandReportStats(now?)
  peekCommandReportStats(now?)
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
export class UnitRigInstance {
  update(entity, renderContext)
  destroy()
}
```
`UnitRigInstance` owns one Pixi container and one graphics child per normalized rig part, redraws
primitive geometry with sampled transforms and tint slots, and tears down all owned children through
`destroy()`. Live rig routing is per-kind through `_liveRigDefinitionsByKind` and covers Worker,
Rifleman, Machine Gunner, Anti-Tank Gun, Mortar Team, Artillery, Scout Car, Tank, Command Car, and
Ekat. Missing or invalid unit rig definitions fail through the renderer's soft missing-texture guard
rather than falling back to a procedural unit branch. Shadow and body parts route through separate
live pools so normal unit and shot-reveal layer ordering stays intact.

Local lab visual profiles may supply per-entity unit rig overrides to `Renderer.render` through
`visualUnitOverrides`. The renderer resolves those rules against the current frame's real unit
entities with local-only selectors, validates candidate ids through the checked-in
`renderer/rigs/visual_override_rigs.js` registry, and then passes the candidate SVG rig definition
through the same `renderLiveUnitRig` runtime path. Overrides never change `entity.kind`, snapshots,
selection ids, command targeting, HP bars, fog, minimap inputs, or scenario authoring data; broken
selectors or candidate rigs publish local diagnostics and fall back to the normal live rig for that
unit.

SVG unit art workflow:
- Author the SVG under the approved importer subset: root rig metadata, stable `part.*` ids,
  `anchor.*` ids for at least `origin`, `selection`, and `hp`, semantic `bounds.*`, direct hex
  paint, approved tint slots, and schema-approved `data-rts-animation` bindings.
- Keep source SVGs mirrored by the checked-in runtime strings in `renderer/rigs/*_svg.js` and
  fixture SVGs in `tests/fixtures/svg/` when a fixture is useful for importer/runtime tests.
- Verify new or changed rigs with `node tests/rig_schema.mjs`, `node tests/svg_rig_importer.mjs`,
  `node tests/rig_runtime.mjs`, and `node scripts/check-client-architecture.mjs`.
- Runtime contract changes are accepted through schema, importer, animation, renderer smoke, and
  architecture tests rather than static design previews.

Prototype raster rig workflow:
- Support-weapon and vehicle rendering may opt into PNG atlases through `renderer/rigs/*_png_atlas.js`,
  `png_routing.js`, and `png_runtime.js`. The SVG rig remains authoritative for anchors,
  animation bindings, part ids, recoil, facing, and route split; the PNG atlas only supplies
  pixels for those sampled parts. The current tank atlas is an enabled visual experiment, not final
  art: it uses the pass-11 white-painted Tiger I hull/body, turret/coax, and separate main-barrel
  cells while transparent track frames suppress track rendering. The separate barrel cell maps to
  `part.barrel`, so the PNG rig keeps the original SVG cannon recoil scale instead of merging that
  motion into the turret. The active `pass11-white-dim30` atlas is an imagegen repaint of pass 10;
  it keeps the no-guide semantic sheet, bakes 30% lower brightness and 20% lower saturation, relies
  on visible-alpha postprocessing plus 1.2x world-scale compensation to size generated components
  against the SVG rig bounds, and intentionally applies runtime team tint over the dimmed white base
  using the semantic atlas tint slots. Mortar Team rendering uses a generated three-cell M2
  4.2-inch-inspired wheeled mortar sheet: assembled reference, team-tinted carriage/frame and
  tube/barrel assembly, plus fixed-color tire overlays. The carriage sprite follows the SVG
  carriage recoil binding while the separate tube sprite follows the stronger weapon recoil
  binding. See
  [raster-unit-art-handoff.md](raster-unit-art-handoff.md) for the methodology, rejected imagegen
  passes, and next validation work.
- `scripts/art/tank-raster-pipeline.mjs` builds the tank contact sheet, records the exact prompt
  under `client/assets/rigs/tank-ps1/metadata/prompt.md`, and rewrites the atlas metadata after an
  image-generation pass. The current prototype uses semantic grouped cells: complete tank reference
  without tracks, drop shadow, or fuel icon; an empty track suppressor; hull assembly; turret/coax
  assembly; separate main barrel; and one unused empty guide cell.
- Keep the source sheet, generated pass, alpha atlas, prompt, and manifest together under
  `client/assets/rigs/tank-ps1/` so raster iterations remain reproducible. The renderer falls back
  to the SVG rig until the atlas texture loads or if the atlas load fails.

`renderer/feedback_view_model.js`
```js
export function buildRendererFeedbackView(state, options?)
  // Per-frame read model builder. Returns placement, command feedback,
  // selected entities, resource mining previews, support-weapon previews,
  // ability target previews, ability objects, smokes, transient projectile/
  // target markers, relationship helpers, and entity lookup for renderer
  // feedback drawing without exposing the full mutable GameState. `options`
  // may inject frame-local entities and selectedEntities arrays.
```

`state_ground_decals.js`
```js
export class GroundDecalBuffer {
  applySnapshotEvents(events, context)
  reconcilePending()                    // stage shared pre-assembly batch used by Match
  acknowledgeReconciled()               // release it after a successful backend frame
  consumePending()
  get pendingCount()
  clear()
}
export function normalizeGroundDecalEvent(ev, context?)
export function groundDecalClassForKind(kind)
export function groundDecalClassForImpactEvent(eventKind)
```
`GameState.applySnapshot` feeds fog-filtered transient death, mortar-impact, and artillery-impact
events into this browser-local buffer. The buffer dedupes deaths by id and impact events by their
received snapshot identity, recovers owner/facing from the prior visible entity snapshot only for
death marks, and never infers a hidden death or impact from missing entities.

`renderer/decals.js`
```js
export const GROUND_DECAL_TEXTURE_WORLD_SCALE
export class GroundDecalLayer {
  resetForMap(map)
  stampBatch(decals, options?)
  displayObjectCount()
  diagnostics()
  destroy()
}
```
`GroundDecalLayer` owns one downsampled canvas-backed Pixi texture and one sprite on the `decals`
world layer. New visible death and impact decals are stamped into that texture in batches from SVG
alpha-mask assets under `assets/decals/`; historical decals are pixels, not retained display objects or
per-frame records. `diagnostics()` exposes total stamped decals, queued decals, texture update
count, texture dimensions/downsample, child count, and asset-load status for stress checks. The
renderer tears down the decal sprite, texture, canvas, tint scratch canvas, loaded atlas masks, and
late async asset loads through `Renderer.destroy()` / rematch cleanup.

`renderer/trenches.js`
```js
export function normalizedTrenches(trenches, tileSize?)
export function _drawTrenches(state)
```
`_drawTrenches` renders the latest authoritative `state.trenches` into one persistent Pixi
`Graphics` object on the `trenches` world layer. It clears and redraws that object each frame,
skips malformed records, and draws deterministic connectors between nearby trench footprints so
clustered neutral trenches read as continuous ground without allocating per-trench display objects.
`Renderer.destroy()` removes and destroys the trench graphics object during rematch teardown.

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
shouldMountObserverAnalysisOverlay({ capabilities })
createObserverAnalysisOverlayPreferences(storage?)
export class ObserverAnalysisOverlay {
  constructor({ root, preferences, getEntities, getCameraBounds, getPlayers, stats })
  applyObserverAnalysis(payload)            // renders server-backed production, unit, and losses tabs
  update(frameViews?)                     // refreshes viewport army value from camera/snapshot state
  destroy()
}
```
`App` owns one observer analysis preference object and passes it through replay and live spectator
`Match` rebuilds so selected tab, visible state, and collapsed state survive replay
seek-triggered `start` messages and spectator rematches. Preferences are stored under
`rts.observerAnalysisOverlay`; clients still read the old `rts.replayAnalysisOverlay` key for
compatibility. Its titlebar is draggable, keyboard-nudgeable, viewport-clamped, and retains its
desktop placement through replay seeks and spectator rematches; `Home` restores the default
placement. The overlay owns its generated DOM and is read-only. The Army Value tab is
client-side and viewport-specific; Production, Units, Units Lost, and Resources Lost render the
latest server-authored `observerAnalysis` payload. Resources Lost follows the protocol's narrow
definition: spent steel/oil value of units that died, excluding buildings, stockpile changes,
harvesting, refunds, and cancelled queues.

`ai_diagnostics_panel.js`
```js
shouldMountAiDiagnosticsPanel({ capabilities, players })
createAiDiagnosticsPanelPreferences(storage?)
export class AiDiagnosticsPanel {
  constructor({ root, preferences, getPlayers, onMapLayerVisibilityChange })
  applyObserverAnalysis(payload)          // renders optional per-player aiDiagnostics trace rows
  mapLayerVisibility()                    // current map-analysis overlay layer switches
  destroy()
}
```
`Match` mounts the AI diagnostics panel beside the observer analysis overlay only when the room
advertises observer-analysis diagnostics and the start roster contains at least one `isAi`
participant. The panel consumes the same server-authored `observerAnalysis`
payload, but normalizes and renders `aiDiagnostics` separately so high-churn AI trace lines do not
dirty-update the general replay/spectator analysis tabs. It owns its generated DOM, persists
visible/collapsed/selected-AI state plus map-analysis layer toggles under
`rts.aiDiagnosticsPanel`, uses the shared lab-panel window chrome for drag, resize, collapse,
keyboard nudge, and viewport clamping, and renders one tab per AI diagnostics row with profile id,
trace tick, status metrics, and bounded decision trace lines for AI-vs-AI debugging. When
`observerAnalysis.mapAnalysis` is present, the panel also shows region/choke/base/resource/label
switches that drive the passive world overlay without writing to `GameState`, command targeting,
selection, prediction, or fog.
`match_observer_diagnostics.js` composes both observer surfaces for `Match`, forwards
`observerAnalysis` messages to each mounted panel, retains the latest optional map-analysis payload
for renderer consumption, updates the viewport-dependent observer analysis frame surface, and
centralizes teardown.

`renderer/observer_map_analysis.js`
```js
_drawObserverMapAnalysisOverlay(model, { camera })
```
The renderer draws server-provided observer map-analysis primitives on a dedicated Graphics/Text
pair mounted below ordinary command feedback but above fog. It supports tile-rect region fills,
choke segment bands, base/resource/approach markers, labels, and per-layer visibility from the AI
diagnostics panel. The overlay is observer-only visual state and does not contribute to hit testing,
entity pools, minimap blips, local fog sources, or game state.

`frame_entity_views.js`
```js
buildFrameEntityViews(state, { alpha }) // frozen SharedFrameContextV1 outer record with frame-local entity arrays
```
`frame_recovery.js` builds this object once per requestAnimationFrame after prediction display has
advanced and before fog, renderer, HUD, minimap, and observer analysis run. The object is not
authoritative state and must not be retained after the frame; it exists only to share common
`GameState.entitiesInterpolated()` and `selectedEntities()` results across frame consumers.
`interpolatedEntities` uses the render alpha and prediction display for the Pixi renderer,
`currentEntities` uses the latest predicted display positions for minimap blips and HUD tech
checks, `authoritativeEntities` uses latest no-prediction positions for local fog-source filtering
and observer Army Value rows, and `fogSourceEntities` removes shot-reveal/vision-only entries plus
non-vision neutral resources.

`presentation/layers.js`, `presentation/grid_snapshot.js`, and `presentation/frame.js`
```js
PRESENTATION_LAYER_DESCRIPTORS       // exact frozen back-to-front semantic descriptors
createGridSnapshot(input)            // opaque immutable revisioned grid accessor
new PresentationFrameAssembler({map, generation?, entityStats?})
assembler.staticMap                  // StaticMapPresentationV1, rebuilt only on map revision/reset
assembler.assemble(inputs)           // one detached frozen PresentationFrameV1
assembler.reset({map, generation?})  // replay/Lab/rematch generation reset seam
```
`frame_recovery.js` updates authoritative fog before it assembles this sidecar. It samples one
projection, one visual time, one renderer feedback view, and one observer/screen-overlay model for
the frame; the same projection drives `SelectionSceneV1`. Match then makes exactly one
`renderer.render(presentationFrame)` call. `PixiPresentationAdapter` reconstructs only its
ratcheted frame-local compatibility facade, copies static/fog grids into Pixi-owned staging, and
does not expose its adapter to another backend. The sidecar contains no mutable state/intent,
selection proxy, mutable typed array, Pixi object, or transport record. Static terrain/resource
locations are separately revisioned; visible/explored grids reuse opaque snapshots by revision.
`settings_container.js`
```js
export class SettingsContainer {
  constructor({ button, menu, title? })
  setContext({ kind, spectator, replay, actions, tabs }) // mounts context-specific tabs/actions
  setTabs(tabs)                         // [{id,label,visible,render(panel, context)}]
  open({ focus }), close({ restoreFocus }), toggle()
  isOpen()
  activateTab(id)
  destroy()
}
```

`settings_panels.js`
```js
buildSettingsTabs({ audio, hotkeyProfiles, game, debug })
buildGiveUpAction({ visible, onOpen })
buildPauseAction({ visible, disabled, label, title, onPause })
```

`live_pause_overlay.js`
```js
export class LivePauseOverlay {
  constructor({ root, onUnpause })
  applyLivePauseState(state)
  destroy()
}
```

`replay_controls.js`
```js
export class RoomTimeControls {
  constructor({ net, state, replayViewer?, capabilities, label? })
  applyRoomTimeState(state)
  noteSnapshotTick(tick)
  destroy()
}
export class ReplayControls extends RoomTimeControls
```
`RoomTimeControls` renders pause/resume, speed, step, relative seek, absolute timeline seek, tick
status, and keyframe marks only from `capabilities.roomTime`. The AI-only live route advertises the
speed-only room-time profile, so the same component renders no seek, step, or timeline affordance
for those rooms. Replay fog-perspective controls and the replay-branch button remain gated by
replay-specific visibility/action capabilities, not by lab or URL identity.

The shared control surface is the `dom.roomTimeControls` root (`#room-time-controls`). Static
pause/step controls use `.room-time-pause-btn` and `.room-time-step-btn`; generated room-time status
and timeline markup use `.room-time-tick-status` and `.room-time-timeline*` selectors. The exported
`ReplayControls` alias remains only for existing replay imports while composition should construct
`RoomTimeControls` for any room that advertises room-time capabilities.

`room_capabilities.js`
```js
createRoomCapabilities({ startPayload })
```
`Match` and app-shell controls consume this parsed `startPayload.capabilities` and
`startPayload.diagnostics` record for room-time controls, diagnostic settings, observer analysis,
vision-selection controls, live pause controls, replay branch actions, and read-only/gameplay command
affordances. Product shells may still use product metadata for launch/routing and owned controls
such as lab setup tools, but shared affordances must not be inferred from replay/dev/lab
identity.

`lab_client.js`
```js
export class LabClient {
  constructor(net, options?)
  setInitialState(state)
  subscribeState(handler)                // returns unsubscribe
  subscribeResult(handler)               // returns unsubscribe
  setVision(vision)                      // sends {op:"setVision", vision} for this operator only
  setPlayerGodMode(playerId, enabled)    // sends {op:"setPlayerGodMode", playerId, enabled}
  exportMap()                            // authoritative map-only editor transition payload
  exportScenario(name?)                  // compatibility wire name for checkpoint setup export
  importScenario(scenario)               // compatibility wire name for checkpoint/legacy setup import
  validateScenario(metadata)             // sends {op:"validateScenario", metadata}
  submitScenario(metadata, options?)      // sends {op:"submitScenario", metadata}
  resetScenario()                        // seeks lab room time to the current setup baseline
  request(op, options?)                  // allocates requestId, resolves with labResult/timeout
  destroy()
}
export function labVisionLabel(vision)
export const labVision                   // all(), team(teamId)
```

`lab_catalog.js`
```js
export function normalizeLabScenarioEntry(entry)
export class LabCatalogScreen {
  constructor({ root, fetchImpl?, initialRoom?, onStart })
  mount()                                // fetches GET /api/lab-scenarios and renders choices
  setConnected(connected)
  setStatus(status, options?)
}
```
`LabCatalogScreen` is app-owned and used only for the bare `/lab` route. It renders a blank lab row
plus bundled checkpoint setup metadata from `GET /api/lab-scenarios`; clicking a row builds the existing
hidden `__lab__:<room>:map=<map>:scenario=<id>` join room and lets `App` start the normal lab flow.
Direct `/lab?scenario=lategame`, `/lab?scenario=blank`, map, and seed URLs still bypass the selector
and auto-join for compatibility.

`interact_bridge.js`
```js
export const INTERACT_BRIDGE_KEY = "__rtsInteract"
export class InteractBridge {
  status()                            // readiness only; no internal references
  call(method, input)                 // status/catalog/spawn/update/remove/order/time/inspect/camera/reset/presentation/captureReadiness
  destroy()
}
export function interactLaunchEnabled(locationLike?)
```
`App` composes this bridge only when the `/lab` URL includes `interact=lab`. Its global surface is a
frozen `{version, status, call}` object; it never returns `App`, `Match`, `Net`, `Renderer`, or
`GameState`. Calls delegate through existing `LabClient`, normal `issueCommandAs`, room-time,
semantic camera, and `GameState` projection seams. Catalog includes the mirrored command and ability ids;
inspection can restrict results to the current camera viewport, while camera focus accepts bounded
padding, defaults to a close 32-world-pixel frame for readable single-unit captures (and retains
48 world pixels for multi-subject and non-unit framing), and returns `CameraSnapshotV1` plus
semantic CSS viewport and ground bounds. Camera set accepts only `CameraSnapshotV1`; status,
readiness, screenshot manifest v2, recording manifest v2, and fixed-capture manifest v2 carry the
same versioned shape. The Lab bridge surface version is 4. Setup mutations
wait for the server's immediate authoritative snapshot without advancing paused simulation. Order
calls also wait for a new snapshot and request one bounded tick when paused so the queued command is
consumed before success.
`presentation` calls the app-owned `CleanPresentation` helper, which hides only DOM chrome and never
hides Pixi world layers; it is removed on capture completion, rematch, and app teardown. `captureReadiness`
reports bounded live PNG/frame-strip/profile/decal asset status, font status, render frames, frame-loop
errors, renderer errors, and subject missing-texture fallbacks without exposing renderer references.
The local `scripts/interact/driver.ts` owns the selected-worktree server, headless browser, logs,
clean viewport clipping, readiness wait, PNG/JSON artifacts, and profile cleanup. The bounded command
service owns aliases and exact input contracts. Its per-worktree daemon preserves that state across
machine-readable CLI calls, expires after 30 idle minutes, and returns screenshot paths and metadata
without embedding image content. Portable setup export/import uses the bridge's narrow
`exportSetup`/`importSetup` methods and keeps checkpoint bytes out of CLI results. Replay bytes bypass
the browser and normal WebSocket result through the capability-gated private-server handoff; the
daemon writes only bounded artifacts and alias sidecars under `target/interact/lab/`.
Visual delivery is deliberately not owned by that per-worktree lifecycle. Before returning a
Tailnet URL, the daemon validates the artifact and copies it into the machine-level
`tailnet-preview` service on stable port 8091. The preview server has no idle timeout, and each
copied artifact has at least 24 hours of retention, so Lab close/shutdown, idle expiry, and worktree
removal do not invalidate issued links. A later publisher can restart the service and continue
serving unexpired copies from its OS-temporary root.
Operational aliases, inspection, camera focus, screenshot subjects, and corresponding bridge
entity-id inputs accept at most 400 references. Screenshot readiness validates the complete
requested subject set, but returned and persisted subject detail is capped at 24 rows with total and
`truncated` metadata; recording and fixed-capture alias detail is similarly capped at 40 rows.
Successful bulk spawn and artifact-import responses default to counts, a `truncated` flag, and at
most 12 ordered detail rows. Callers may explicitly request `details: true` when they need every
spawned entity/raw outcome or every restored and stale alias row; rejection details remain complete
and actionable regardless of that success-response option.
The daemon publishes its startup checkout commit as optional IPC v1 state/probe metadata. The CLI
refreshes a mismatched daemon only through an atomic idle-only shutdown request; active scenes are
preserved behind `daemonCheckoutMismatch`, while `status` and `shutdown` remain usable.
Real-time recording consumes raw Chrome DevTools screencast frames and assigns them to cumulative
30 FPS monotonic-wall-clock slots before streaming H.264. Its manifest records raw event/timestamp
gaps and exact source-frame reuse; it warns below 80% source-slot coverage. `record-start` can also
resume authoritative time atomically after the initial frame through its bounded `resumeSpeed`.
Fixed capture likewise streams up to 1,800 rendered PNG buffers into H.264, retains at most six
representative PNGs, and keeps per-frame ticks/hashes in the manifest instead of the CLI response.

`interact_game_bridge.js`
```js
export class InteractGameBridge {
  status()                            // isolated-match readiness and bounded semantic UI state
  call(method, input)                 // status/inspect/move/giveUp/time/camera/presentation/captureReadiness
  destroy()
}
export function interactGameLaunchEnabled(locationLike?)
```
`App` composes this bridge only for a root `rtsLaunch=match` URL whose room begins
`interact-game-`, role is `player` or `spectator`, and `interact=game`. The CLI creates either one
local human plus one AI or exactly two AI seats through ordinary lobby automation. The
bridge observes only the recipient's normal fog-filtered `GameState`, projects a fixed semantic UI
schema (HUD resources, timer, selection, command-card labels, give-up dialog, and score screen),
and never returns internal object references. Its only gameplay command is `move`, which validates
1–100 unique visible locally owned unit ids plus an in-map destination and delegates to the normal
`Match.commandIssuer`. `giveUp` delegates to `Match.requestGiveUp` and waits for the ordinary score
screen. It exposes no arbitrary protocol command, DOM selector, browser evaluation, attack,
production, economy, or ability surface. Spectators cannot call move or give-up; only AI-only room
speed control is exposed for sampled time-lapse capture. Camera `overview` disables the automatic
spectator director and fits authoritative map bounds. Game screenshots and recordings default to
normal presentation and accept full-viewport, live-minimap, or bounded custom regions; clean
presentation is an explicit opt-in.

`lab_scenario_authoring.js`
```js
export const LAB_SCENARIO_AUTHORING_LIMITS
export function createLabScenarioAuthoringState(options?)
export function slugifyLabScenario(value)
export function validateLabScenarioAuthoringState(state)
export function labScenarioPreviewLabel(preview)
```
`lab_scenario_authoring.js` is a pure UI helper for the app-owned lab panel. It owns checkpoint
setup authoring field defaults, slug generation, client-side metadata limits that mirror the server
catalog limits, comma-separated tag parsing, and local blocking errors before the panel sends a
server dry-run validation request.

`lab_scenario_submission_capability.js`
```js
export const LAB_SCENARIO_SUBMISSION_CAPABILITY_PATH
export function fetchLabScenarioSubmissionCapability({ fetchImpl?, retryDelaysMs?, sleep? })
```
`lab_scenario_submission_capability.js` is the app-owned HTTP probe for
`/api/lab-scenarios/submission`. It returns the server capability JSON when available and retries
transient network/proxy failures before reporting `capabilityCheckFailed` to the lab panel.

`lab_scenario_submission_flow.js`
```js
export function createLabScenarioSubmissionState()
export function defaultLabScenarioSubmissionWindow(url)
export function setLabScenarioSubmissionCapability(panel, source)
export function updateLabScenarioTitle(panel, value)
export function captureLabScenarioAuthoringFields(panel)
export function renderLabScenarioOptions(panel)
export function labScenarioSubmissionDisabledReason(panel)
export function validateLabScenario(panel)
export function submitLabScenario(panel)
export function destroyLabScenarioSubmission(panel)
```
`lab_scenario_submission_flow.js` is the LabPanel-owned UI helper for checkpoint setup dry-run validation,
submission capability normalization, duplicate-click guarding, draft PR progress/result rendering,
and teardown of pending submission state.

`lab_panel.js`
```js
export function labSpawnFactionOptions()
export function labSpawnUnitKindsForFaction(factionId)
export function labBuildingSpawnFactionOptions()
export function labSpawnBuildingKindsForFaction(factionId)
export class LabPanel {
  constructor({ root, labClient, launch, startPayload, match?, onEditMap?, submissionCapability?, openWindow? })
  applyLabToolChange(change)             // syncs active/cancelled tool status from Match callbacks
  armSpawnPaletteTool(kind?)             // arms a Match-owned completed spawnEntity world-click tool
  armBuildingSpawnPaletteTool(kind?)     // arms a Match-owned completed building spawnEntity tool
  cancelActiveTool()
  validateScenario(), submitScenario(), exportScenario(), importScenario()
  saveLabReplay(), openLabReplay()        // distinct replay affordances; not legacy scenario ops
  destroy()
}
```
`map_editor_session.js`
```js
export const MAP_EDITOR_HISTORY_LIMIT    // 25
export class MapEditorSession {
  initializeBlank(options?)
  initializeFromScenario(scenario, options?)
  loadAuthoredMap(source, options?)
  mutate(label, mutation), undo(), redo()
  beginTerrainStroke(label?), paintTerrainTiles(tiles, terrain), commitTerrainStroke()
  materialized(), exportMap(), saveLocal(key), loadLocal(key)
  mapOverlay()
}
```
`LabPanel` renders separate floating, collapsible Options and Tools windows. Options owns room
status, lab vision, command-limit policy, setup authoring metadata, validation, PR submission,
setup import/export/reset, and result status; Tools owns target player, player state, spawn palettes,
active tool status, and the remove setup tool. Vision presents one `Full` button plus one button per
team; `Full` requests the authoritative union of every current team's fog, and Lab exposes no
omniscient/no-fog control. App passes the HTTP
`/api/lab-scenarios/submission` capability probe into the panel; unavailable deployments keep the
submit action disabled and leave local setup JSON export visible. Successful submissions open the draft PR
when allowed and always render the PR URL in-panel so popup blocking does not hide the review link.
The supported author workflow starts at `/lab`: choose a bundled catalog setup or blank setup,
edit authoritative state with lab tools, fill in setup name/title/slug/description/tags/review
notes, run validation, then submit a draft PR when the backend capability is available. After human
review and merge, the merged setup becomes selectable from the catalog on the next deployed build
or local server restart that includes the new manifest entry. The browser never supplies setup
JSON, target paths, branch names, credentials, or commit text for PR submission; local JSON
export/import remains the fallback for setup iteration and for deployments with submission
disabled. The lab replay controls are visually separate from setup checkpoint JSON controls; replay
save/open uses the bounded lab replay artifact path instead of the legacy `exportScenario` and
`importScenario` lab operations. Lab now exposes one `Edit map` action. It requests the current
authoritative map-only payload, creates a server-validated editor handoff, and navigates away; no Lab
entity, resource, order, timeline, or replay state crosses that boundary.

`MapEditorApp` owns the dedicated editor. The panel loads bundled JSON from `/maps/catalog` and
`/maps/<file>`, creates the fixed-size blank map, edits name/description plus flat start and base
locations, and provides undo/redo, local save/load, and JSON export. Start locations set map player
capacity; every base location is permanent and its resources spawn even when no player starts there.
Editor drafts may temporarily contain zero start locations so authors can clear and rebuild the player
layout. Adding symmetric starts reuses any base sites already present at the target locations. There is no
active layout, player slot, or per-player natural assignment. The viewport draws blue start
markers and neutral base markers over the shared Pixi terrain and owns editor-only pan/zoom/paint/site input. Terrain tools support brush
and inclusive drag-box fills, plus none, horizontal, vertical, half-turn, four-way radial, or either
single-diagonal symmetry; grass is the erase material. Symmetry expands every terrain tile before it is
painted, moves matching start locations, removes matching neutral base locations when moving a selected base,
and adds all symmetric locations. The selected neutral base has a pale map ring. The viewport draws the selected
centre axis, a centre marker for half-turn symmetry, a cross for radial symmetry, or the selected diagonal.
Grass, bare road, and the four marked road orientations are passable paint materials; roads may
cross protected start/base areas while rock and water remain rejected there. Authored map rows
encode bare, horizontal-marked, vertical-marked, NW-SE diagonal-marked, and NE-SW diagonal-marked
roads with `=`, `-`, `|`, `\`, and `/`, respectively.
Editor status stays above the scrolling controls; failures use a high-contrast alert treatment.
A terrain pointer stroke clones once for undo,
mutates rows in place, records dirty tiles, and commits once. The renderer patches those tiles plus their
edge-sharing neighbours into the existing canvas texture and calls
`baseTexture.update()`; it does not recreate the canvas, fingerprint/serialize the map, or replace a Pixi
texture per tile.

`Open in Lab` posts the authored map plus its flat materialized locations to `/api/map-handoffs`.
The bounded server record expires after two minutes and is consumed once. Lab consumption creates a
private Lab whose first `start` payload already contains the edited map at tick zero; returning through
`Edit map` transfers only an authoritative exported map. A bounded `workspace` id keeps the editor's
local map workspace available across the round trip when browser storage is available.
`lab_panel_window.js` owns local drag, resize, collapse/expand, reset, keyboard nudge,
viewport-clamping, and localStorage geometry hints for those app-owned lab windows. It has no
transport or match authority.

`lab_control_policy.js`
```js
export function createLabControlPolicy({ labClient, metadata })
export function createDefaultControlPolicy()
```

`App` owns `LabCatalogScreen` before joining a lab and owns `LabClient`, `LabPanel`, and lab control
policy lifetimes when a `start` payload carries `lab` metadata. `Match` receives `labMetadata`,
`labClient`, and `labControlPolicy` through constructor options only; renderer, HUD, input, and
minimap do not import lab modules. The shipped MVP exposes catalog/blank lab selection,
per-operator lab vision, per-player asset god mode, setup mutations, issue-as commands, and
setup checkpoint import/export through those collaborators while keeping the normal match screen authentic.
Lab operator starts are still spectator-shaped for projection and prediction, and
`LabClient` treats `start.lab.vision` plus `labState.vision` as the recipient's server-authoritative
choice; `start.lab.godModePlayers` plus `labState.godModePlayers` mirror room-scoped player god
mode. The injected control policy exposes `canUseCommandSurface(state)` and local
`ignoreCommandLimitsEnabled()`/`setIgnoreCommandLimits(enabled)` controls so `Match` and HUD can
keep selection plus the real command card available for operators while read-only lab viewers,
replay viewers, and normal spectators remain passive. Lab selection itself is not toggled by this
control. Operator gameplay commands still flow through `commandIssuer.issueCommand`, where
`LabControlPolicy` wraps them as lab `issueCommandAs` requests for the single controllable selected
owner and includes whether that command should bypass the normal command-supply limit.
Mixed-owner lab selections remain command-blocked, but renderer inspection treats every selected
operator-controllable owner as a feedback owner so all-team Lab overlays such as rally/order
markers and selected support-weapon field-of-fire wedges can be compared across players.
Every client surface that needs ownership semantics must read through the injected control policy
or the `GameState` helpers that delegate to it: command-card resources/faction/upgrades,
right-click enemy classification, control groups, renderer feedback ownership, rally/order overlays,
range/setup previews, minimap commands, and combat audio categories. Raw `state.playerId` remains
the local viewer id and is not the lab command owner.

Lab setup tools use `ClientIntent.activeLabTool` for browser-local armed tool state. `LabPanel`
may ask `Match.armLabTool(tool, { onWorldClick, onBoxSelection })` to arm a tool, and normal
`Input` consumes a completed left world click before selection, command targeting, or placement.
Tools that opt into `paintOnDrag` sample and interpolate each crossed map tile, delivering repeated
world-click callbacks without disarming the tool or falling through to selection; unit spawning,
building spawning, and draft terrain painting use that path. Other left drags normally promote to box
selection and cancel the active lab tool, while tools that opt into `consumeBoxSelection` receive the
selectable ids intersecting the real screen drag box instead. Box callbacks also receive the screen
rectangle and, for diagnostics only, any available ground polygon plus conservative bounds.
World-click callbacks receive the active tool payload, an exact nullable-ground world position (or
the selected proxy anchor for an entity-only hit), and any selectable hit entity id. `ClientIntent.labToolPreview` tracks the armed
tool at the world cursor for the renderer: unit and building spawn ghosts, the chosen terrain tile,
and a large removal X make the pending action visible before a click.
`Match.cancelLabTool(reason)` clears the tool and preview for Esc, right-click, teardown, ordinary
box selection, or panel-driven cancellation. Window blur releases camera/input transient state but
does not cancel the active lab tool. `Input` routes those cancellations through the injected lab tool
controller so `Match` can publish an active/cancelled change back to the app-owned `LabPanel`,
keeping the panel status and cancel affordance synchronized with keyboard, pointer, world-click,
box-selection, and teardown paths. Starting ordinary placement, command targeting, or command-card
build menus cancels the active lab tool so setup tools do not share state with gameplay command
modes. Unit and building spawning are lab panel palettes backed by the client faction catalog mirror
and playable faction labels; each palette arms a persistent completed `spawnEntity` lab tool and
clicking or dragging sends the chosen world positions through `LabClient` until cancelled. The lab does
not expose a secondary advanced spawn fallback; the panel spawn affordance is limited to playable
faction unit and building palettes. The visible map-editing surface is limited to the remove tool:
it arms a persistent `removeSelectableUnits` setup tool; clicking deletes the selectable unit or
building under the cursor, and dragging deletes selectable units and buildings in the box without
changing the current selection.

`hotkey_profiles.js`
```js
export class HotkeyProfileService {
  constructor({storage?, catalog?, profilesKey?, activeKey?})
  allProfiles()
  getActiveProfile()
  hasProfile(id)
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
  storedProfilePayload(profile)
}

buildHotkeyCommandCatalog(cards)
normalizeHotkey(value)
profileBindingForCommand(profile, commandId)
setProfileBindingForCommand(profile, commandId, key)
```

`hotkey_editor.js`
```js
export function renderHotkeyEditor(root, hotkeyProfiles, context?)
export class HotkeyEditor {
  constructor(root, hotkeyProfiles, context?)
  render()
  destroy()
}
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
`#give-up-open` plus live-match action `#live-pause-open`; they may not exist until their owning
tab/action is visible. `Match` owns `LivePauseOverlay` under `#game-screen` for reliable
`livePauseState` messages; the overlay exposes resume only when the server grants
`canUnpause` and is destroyed with the match.

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
  get tick()                             // latest authoritative simulation tick; 0 before snapshots
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
  setProgressPredictionPaused(paused)    // freezes/resumes wall-clock progress display prediction
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
  holdCommandTarget(kind, key, shiftKey, options?), releaseCommandTargetKey(key, shiftKey)
  releaseCommandTargetShift()
  commandFeedback, addCommandFeedback(kind, x, y, append?, radiusTiles?, now?), liveCommandFeedback(now)
  resourceMiningPreview, updateResourceMiningPreview(preview)
  antiTankGunSetupPreview, updateAntiTankGunSetupPreview(preview)
  abilityTargetPreview, updateAbilityTargetPreview(preview)
  recordPlannedCommand(command, selectedEntities, result?)
  plannedOrderPlanForEntity(entity), entityWithPlannedOrder(entity)
  reconcilePlannedOrders(entities, options?), clearPlannedOrdersForUnits(ids), clearPlannedOrders()
  activeLabTool, labToolPreview
  beginLabTool(tool), updateLabToolPreview(preview), cancelLabTool(reason?)
}
```

#### Client Boundary Migration Target

`Match` remains the app-shell composer and owner of cross-area dependency injection. It constructs
`GameState` for authoritative snapshot display data and constructs `ClientIntent` for browser-local
cursor/command intent, then injects the intent facade into HUD, input, minimap, and renderer
feedback. Map-editor symmetry repair rolls back partial site relocation when no complete spawn slot
can be preserved and tries alternate split slots before rejecting symmetry selection. The app shell owns DOM-presentation resize handling. Renderer teardown destroys renderer-owned
adjusted canvas textures and clears its texture maps without destroying raw Pixi asset-cache
textures; raster loads completing after teardown are not cached or displayed. Artillery landing audio is scheduled from the authoritative impact delay,
and match teardown cancels pending landing timers. Mobile camera pan and pinch gestures track only touch identifiers whose touches begin on
the viewport; HUD and off-viewport touches do not join those gestures. Minimap gestures use native
Pointer Events with capture; touch and pen activate targets only on clean taps so inspection does
not issue accidental commands, while desktop right-click and queued-order behavior is preserved.
Camera instances own a
per-session maximum zoom with fallback handling for invalid options. Lab live and replay sessions
use an 8x maximum zoom, while non-Lab sessions retain the 2x cap; Lab initial-camera views are
restored under that limit during normal match initialization. Room-time controls de-duplicate matching touch and pen activation while preserving unrelated click
sources, retain server-confirmed state while requests are pending, and confirm time movement against
the authoritative baseline and controller identity. Blocked, failed, or unconfirmed sends are
exposed instead of applying optimistic selection. Read-only Lab viewers keep these
controls inactive with an authorization-specific status. Coarse-pointer layouts arrange existing mobile debug chrome around safe areas, keep low-priority detail scrollable, and keep controls reachable; room-time panel placement uses the same coarse-pointer gate. Desktop layout is unchanged, and no touch world-command behavior is added. Runtime modules should not gain direct imports across the model, input, UI, minimap,
renderer, and prediction areas except for pinned mirrors such as `protocol.js` and `config.js`, or
for explicitly documented architecture-check exceptions.

`GameState` is the authoritative browser view of server snapshots, interpolation, selected ids,
control groups, relationship helpers, fog-facing visibility data, and display overlays derived from
authoritative snapshots. `ClientIntent` owns placement intent, command-card submenu state,
command-target arming, hover previews, command feedback, ability previews, and the short-lived local
planned-order stages used only for previews while the server echo is pending. Smoke Plus world and
minimap targeting feedback uses the mirrored cloud radius and duration effect fields. An unqueued local order
replaces the stale authoritative plan when composing subsequent queued previews, and asynchronous
Lab command results are not recorded as durable local plans. Contextual oil
right-clicks compose a Pump Jack build intent on the clicked oil patch rather than a gather
command. Advisory building placement ignores unit types whose client configuration marks them as
non-ground placement blockers. The Scout Plane stays out of the shared ground vehicle-body
classifier, so its body does not block build previews. Normal gameplay selection and control-group
commands exclude it, while Lab and spectator inspection paths allow selecting and grouping it. Its generated Fw 189-style top-down PNG frame-strip rig is scaled to the mirrored aircraft body so
the art, hit testing, and selection ring remain aligned. It uses the existing team-light tint
pipeline with a darker, desaturated color target and has a render-preview visual profile for lab
comparison. Normal contextual right-click hover
refreshes `ClientIntent.attackTargetPreview` from the same rules used to compose orders. Visible
Scout Planes are not classified as attackable client targets, so contextual right-click and explicit
attack targeting route through valid move or attack-move commands instead of target attacks. Scout
Plane command cards expose only retarget and dismiss, including the hidden dismiss ability; mixed
selections keep land-unit commands scoped to land units. Renderer feedback draws a red ring around an enemy when the
selected units would attack it; gather, build, deconstruct, and targeted-command modes suppress the
preview. `GameState` must not grow compatibility accessors for those intent fields; HUD, input,
minimap, and renderer feedback use the injected facade or a narrow read model. Lab Unit Spawn and
Building Spawn panels expose the target player's color through DOM data/style hooks before map
placement. In Lab, visual and audio
feedback for controlled-side selections and commands is issued as the selected controlled player
instead of the raw local player id. Lab command-card and targeted-command policy resolves resources,
faction catalogs, per-owner upgrades when present, prerequisites, production helpers, self-ability
hover origins, and hostility from the selected issue-as owner rather than the spectator/viewer id.
Lab Options, Lab Tools, and floating room-time panel headers preserve drag, collapse, and keyboard
geometry handling without visible reset actions.

Frame-local entity views belong to the app-shell frame loop, not to `GameState`. Rendering, local
fog fallback, minimap blips, HUD selection/tech checks, renderer feedback, and observer Army Value
should accept the injected frame view when called from the RAF path and fall back to `GameState`
queries only for direct module tests or event handlers outside the frame. Static resource nodes with
no remaining resources are omitted from frame-local entity views and minimap blips. Minimap
artillery firing indicators render as 30x24 SVG rig images without an extra surrounding ring.
Selected worker units do not draw weapon range indicators, even when their frame-local view exposes
weapon range metadata. Entrenched units render as smaller, trench-tinted rig instances without a
separate occupied-infantry trench ring in the selection layer. Trench ground decals render at half
the authoritative trench radius; snapshot data and the gameplay radius remain unchanged. Frame-local entity views may carry
bounded render diagnostics for local profiling consumers without changing the authoritative snapshot model. Visible unit death events are normalized by `GameState`
into deduped, browser-local pending ground decal stamps and rendered below resources and fog as
visual-only decals. Death decals use SVG-authored mask assets from the client asset path when they
load, queue stamps while the atlas is loading, and fall back to procedural masks when SVG loading is
unavailable or fails; they do not change server protocol, simulation, or balance.

Renderer feedback should consume a narrow read model containing placement, command feedback,
support-weapon setup previews, ability targeting previews, ability objects, and selected entities,
rather than relying on the full mutable `GameState`. Queued support-weapon setup previews use
accepted move or attack-move order-plan endpoints as their field-of-fire origin, plus local pending
move/setup stages when the command has been sent but no owner-only `orderPlan` echo has arrived;
unqueued setup previews use the current support-weapon position. Minimap hover and click targeting
feed support-weapon setup previews and commands from minimap world coordinates for Anti-Tank Guns
and Artillery. HUD command-target arming preserves Shift, and input hover previews track Shift so
queued previews match the command that will be issued. Armed Point Fire and Blanket Fire previews
compute advisory per-artillery locked effective points from the current gun origin, the authoritative
or local pending planned origin, the 25-to-55 tile range band, the current or planned setup facing
fallback, and same-ray map clamping when map bounds are available; command feedback marks those
locked points when the client can compute them while the server still receives and authoritatively
validates the raw clicked point. Local planned setup/fire stages are reconciled on snapshots using
the owner-only `orderPlan` and command acknowledgement metadata, and are cleared on deselection,
Stop/Hold, replacement commands, rejected command receipts, and match teardown. If Tank Traps are
re-enabled, their placement previews keep normal terrain, resource, building, and map-bounds checks,
allow infantry overlap, and reject vehicle-body units. Their line dragging treats terrain, building,
and map-bounds blockers as skipped sites,
omits illegal build commands for those sites, and resumes on the far side; vehicle-body unit blockers
  still break the line. HUD and input should exchange command intent through descriptor/facade
  methods, while gameplay command emission continues to flow through
  `commandIssuer.issueCommand`. HUD selected-unit strip cells support direct selection refinement:
  left-click selects only that unit, Shift-click removes it from the selection, and Ctrl/Meta-click
  or control context-click filters the current selection to that unit kind. Unit command-card
  descriptors include Stop on S and Hold Position on W; Command Car selections also expose
  Breakthrough on E. In lab rooms, injected lab control policy owns selection, inspection, and
  single-owner issue-as routing without changing the client player id. `PredictionController` owns
  client sequence allocation and optimistic bookkeeping; `GameState` applies named display overlays
  but does not own prediction policy.

`camera.js`
```js
export class Camera {
  // Semantic renderer-neutral API; all screen values are viewport-local CSS px.
  project(point) -> {x,y,depth,clip,visible}
  groundAtScreen(screen) -> {x,y}|null
  projectedExtent(point, worldW, worldH) -> {width,height,scaleX,scaleY,visible}
  viewportGroundPolygon() -> [{x,y}, ...]
  viewportGroundBounds() -> {minX,minY,maxX,maxY}|null
  containsProjected(point, marginCssPx?) -> boolean
  focusAt(point), framingForWorldPoints(points, options?), fitWorldPoints(points, options?)
  panByScreenDelta(delta), dollyBy(factor, anchorScreen?)
  resize(viewW, viewH), setMapBounds(worldW, worldH)
  snapshot(), restore(snapshotOrLegacy), projectionSnapshot()
  audioListener(), subscribe(listener) -> unsubscribe

  // Private orthographic compatibility used only by Camera and named Pixi adapters.
  x, y, zoom                             // world coords of viewport top-left, framing scale
  update(dt, input)                      // apply pan (keys/edge/virtual pointer-lock cursor) & clamp
  worldToScreen(wx, wy) -> {x,y}
  screenToWorld(sx, sy) -> {x,y}
  centerOn(wx, wy)
  setZoom(zoom, anchorSx?, anchorSy?), setBounds(worldW, worldH, viewW, viewH), setView(view)
}
```

`auto_spectator.js`
```js
export class AutoSpectatorDirector {
  constructor({camera, state, enabled?})
  setEnabled(enabled)
  observeSnapshot(snapshot)              // ingest positioned combat activity and decide at most once per simulated second
  update(dt)                             // advance the active smooth camera transition
  diagnostics(), destroy()
}
```

Replay viewers and ordinary live spectators receive a floating, draggable **Spectator Controls**
panel with one persisted, default-off `Follow active fights` switch. The control is deliberately
kept out of the gear-menu settings. Lab sessions do not mount the director. While enabled, the
director retains three simulated seconds of attack, death, and
positioned-impact activity, groups samples within ten tiles, and frames the highest-weight group;
deaths count as four attacks, impacts as two, and the current fight receives a small stickiness
bonus. When combat activity expires, the director prefers a likely contact between opposing-team
units: units already within 28 tiles qualify, as do movement tracks whose inferred closest approach
comes within eight tiles over the next six simulated seconds. Nearby members of both formations are
included in the shot, same-team pairs and scout planes are ignored, and worker-only contacts receive
a ranking penalty. Combat and likely-contact shots reserve 50% more screen-space context than the
initial director tuning. If no contact is plausible, the camera preserves its focus and widens by 6%
per second, finishing each small widening before beginning another. The local overview never
widens past 70% of either map dimension, so a large display cannot turn it into a whole-map shot.
Decisions occur no more than once every 30 simulation ticks. Nearby reframes pan and zoom with a
one-second smooth transition, distant combat/contact reframes cut immediately, and backward replay
seeks clear future combat and motion tracking before the rebuilt timeline is evaluated.

`renderer/index.js`
```js
export class Renderer {
  constructor(canvasParent)              // creates PIXI.Application, layers
  resize(w,h)
  buildStaticMap(map)                    // draw terrain once into a cached layer
  render(stateFacade, cameraFacade, fogFacade, alpha, options?) // Pixi-private engine seam
  captureReadiness({subjectIds?, subjectKinds?}) // bounded visual asset/error state for Interact capture
  app                                    // the PIXI.Application (for ticker/stage if needed)
  // Pixi implementation used by PixiPresentationAdapter's screenOverlay reconciliation:
  drawSelectionBox(rectOrNull)
}
```

`renderer/pixi_compatibility_adapter.js`
```js
export const PIXI_LEGACY_READ_ALLOWLIST // frozen ids plus concrete review triggers
export class PixiPresentationAdapter {
  constructor(canvasParent, {renderClock,state,profiler,visualProfile,staticMap})
  render(presentationFrame) -> {presented:boolean}
  resize(w,h), enterFixedCapture(clock), presentFixedCaptureFrame(), exitFixedCapture(clock)
  captureReadiness(query), destroy()
}
```
Normal Match rendering uses this adapter. The direct `Renderer` surface remains Pixi-private and
is also owned separately by Map Editor, which has no Match or simulation frame.

`fog.js`
```js
export class Fog {
  constructor(mapWidth, mapHeight, terrain?)
  update(ownEntities, tileSize, serverVisibleTiles?) // copy server visibility when provided; accumulate explored
  isVisible(tileX,tileY), isExplored(tileX,tileY)
  setRevealAll(enabled)
  // renderer reads the grids to draw the black/dim overlay; minimap caches against revision
  visibleGrid, exploredGrid              // Uint8Array length w*h
  revision, visibleRevision, exploredRevision
}
```
`match.js` must exclude legacy/special `visionOnly` and shot-reveal entities from `ownEntities`
before calling `fog.update`; those views are rendered as intel, not as local fog sources. Normal
match snapshots provide `visibleTiles`, so the overlay follows server-authoritative fog including
smoke blockers and five-second lingering death sight; local stamping remains a fallback for
older/dev object snapshots.

Playable own selections and human multi-unit commands use the mirrored command-supply budget from
`command_budget.js`: 24 base command supply plus `COMMAND_CAR_SUPPLY_CAP_BONUS = 20` and the
Command Car's own command weight per admitted Command Car, with unit supply as weight and a fallback
weight of 1. Drag selection, shift-add, double-click same-kind selection, and control-group
save/add/recall preserve their normal
candidate ordering, except Command Cars in the
candidate set are admitted first so their budget bonus is reliable. Overflow candidates are ignored
client-side and surface `selectionBudgetOverflow` for the HUD; outgoing commands that still exceed
the budget are blocked before `Net.command`.

`input/index.js`
```js
export class Input {
  constructor(domElement, camera, state, commandIssuer, drawMarquee, fog, audio?, inputRouter?, hotkeyProfiles?, clientIntent?, labToolController?, desktopCursor?)
  // installs listeners; translates gestures into selection + protocol commands.
  // number keys recall control groups; double-tap jumps the camera to the largest
  // local cluster. Alt/Ctrl/Cmd+number replaces a group, Shift+number adds to it.
  // On Windows, browser saves use Alt+number, including browser fullscreen;
  // installed-app/standalone saves accept Alt/Ctrl/Cmd+number.
  // optional pointer-lock mode traps the browser cursor and drives a visible
  // virtual cursor for edge pan on multi-monitor setups. In the macOS Tauri
  // spike, the optional desktopCursor bridge replaces browser Pointer Lock
  // while keeping the same selection, command, HUD, minimap, wheel, and Escape
  // routing contracts. Match auto-requests that native bridge for Tauri matches
  // and retries on focus/visibility return. Native desktop cursor visuals are
  // painted directly from the native event handler and diagnostics expose backend,
  // native/JS event counts, dropped events, and delivery latency.
  update(dt)                             // continuous handling (edge scroll handled by camera)
  publishSelectionScene(scene)           // after a successful presented frame only
  // emits nothing to return; mutates state.selection / clientIntent and calls commandIssuer.issueCommand
}
```
`input/camera_navigation.js`
```js
export class CameraNavigationInput {
  constructor(domElement, camera, options?)
  // shared command-free camera gesture state for live input and replay/observer wrappers:
  // viewport mouse tracking, mouse-wheel cursor-anchored zoom, configured pan keys,
  // middle-mouse drag panning, optional Space+left-drag panning, touch drag/pinch
  // pan/zoom for mobile viewing, blur release, and teardown.
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
through semantic `Camera.panByScreenDelta({x,y})`; touch drag/pinch pan and dolly without emitting
gameplay commands. Wheel/pinch anchors and every drag delta are viewport-local CSS pixels, including
under non-1 DPR. Mouse-wheel dolly, keyboard pan state, edge scroll state, and blur release are shared
observer navigation behavior. The minimap draws `Camera.viewportGroundPolygon()` and recenters with
`Camera.focusAt()`, so perspective ground footprints may be partial or empty without invented bounds.
Live spectators still use the live `Input` path so read-only selection inspection remains available
while command emission stays gated by local-owner checks.

Shift-right-click appends queued orders only for selected units: move, attack-move, attack,
gather, build/resume, Tank Trap deconstruct, and placement build commands set `queued: true` and
rely on the server snapshot's owner-only `orderPlan` for accepted markers. Shift-clicking or
Shift-hotkeying Hold Position appends its terminal hold stance instead of replacing those queued
orders. Production-building-only right-clicks set
or append building rally stages and rely on owner-only `rallyPlan` for accepted markers. Resource
patches suppress rally command emission. The minimap applies the same resource-target
classification against known live map resources. Attack
targeting with only production buildings selected creates `attackMove` rally stages.
Selection, hover, and entity targeting use the last successfully presented detached
`SelectionSceneV1`; move/ground abilities/placement use its nullable ground query. Screen clicks and
marquees project plain mirrored proxies rather than reading current state or renderer geometry.
Selection and targeting still use `GameState` relationship helpers where the distinction is own/ally/enemy:
single-click may select an allied entity for read-only inspection, box selection and same-kind
selection stay own-only, and right-clicking own or allied entities with own units selected falls
through to ordinary move-to-point behavior instead of attack. Command emission, prediction,
optimistic production/rally, control groups, build/gather/train/research/cancel, and ability
execution remain strict local-owner checks.
Shift-confirmed build placement keeps placement mode armed while Shift is physically held, allowing
multiple queued building placements; releasing Shift or losing window focus clears placement mode.
Tank Trap placement uses the same local placement intent, with optional `lineSites`
preview data: the first valid sites dispatch as one immediate single-worker build per selected
worker, and any remaining valid sites dispatch as queued standard build commands against the
selected worker set. Line placement only offers vehicle-closing Tank Trap steps: exact diagonal adjacency `(1,1)` or
one-tile orthogonal gaps `(2,0)` / `(0,2)`. Invalid intermediate sites break the line instead of
letting dispatch skip ahead across a larger gap. The renderer draws Tank Traps larger than their
1x1 build footprint so these sparse vehicle-blocking gaps read as closed barrier segments.

`command_composer.js` owns command-target arming lifetime for command-card targets. HUD, input, and
minimap receive `ClientIntent` from `Match`; input and minimap clicks call
`ClientIntent.issueCommandTarget`, so held keys, Shift preservation, and repeated queued target
clicks use one composer path instead of command-specific sticky flags. A plain
targeted-order command-card hotkey tap arms the target after keyup; pressing the same resolved
hotkey again inside the quick-cast window issues it at the current cursor world point. Shift does
the same with `queued: true` and keeps the target armed until Shift is released. World-point
ability hotkeys follow the same tap contract: tapping and releasing the key keeps targeting armed
until the first unqueued world click, while physically holding the key only extends targeting for
repeated clicks. After an unqueued quick-cast consumes the armed target, the next near, still
viewport left-click is ignored as an accidental confirmation click; moving far enough to become a
drag restores normal selection.

`input/router.js`
```js
export class MatchInputRouter {
  constructor(viewportEl)
  registerZone(zone)                     // zone: {priority?, contains(ev), pointerDown?, pointerMove?, pointerUp?}
                                         // returns unregister()
  pointerDown(ev) -> boolean             // routes to highest-priority matching zone
  pointerMove(ev) -> boolean             // captured zone receives moves until release
  pointerUp(ev) -> boolean               // releases capture after the originating source handles up
  wheel(ev) -> boolean                   // routes locked/native wheel events to DOM surfaces before camera zoom
}
```
Router events carry `viewportX`/`viewportY` plus `clientX`/`clientY`; pointer-lock input and DOM
input use the same zone contract so DOM overlays can work while the browser routes mouse events
to the locked viewport. `Match` registers one game-screen DOM zone that ignores the viewport
subtree; explicit zones such as the minimap keep higher priority, and any future interactive panel
above the battlefield receives pointer/mouse/click/wheel events without being separately listed.

`audio.js`
```js
export class Audio {
  preload(manifest): Promise<void>        // decode sounds once the AudioContext is unlocked
  unlockFromGesture(ev?) -> Promise<boolean>
                                          // create/resume AudioContext from a user gesture
  isUnlocked() -> boolean                 // true when the AudioContext is running
  onUnlockChange(fn) -> unsubscribe       // notify settings UI after first successful unlock
  play(id, {x?, y?, directionalOnly?, priority?, category?, pitchVariance?, gain?, duck?, key?, loop?, fadeInMs?})
                                           // x/y spatialize; directionalOnly omits distance gain
  playUI(id, opts)                        // non-spatial ui category convenience
  stopByKey(key, {fadeOutMs?}?) -> number // stop or fade tagged sustained/abortable voices
  setVoicePosition(key, x, y) -> number   // repan active keyed spatial voices without restart
  setListener({x,y,referenceDistancePx})   // consumes semantic AudioListenerV1
  pickVariant(ids) -> id|null             // seeded RNG variant choice
  setMasterVolume(v), getMasterVolume()
  setCategoryVolume(cat, v), getCategoryVolume(cat)
  destroy()
}
export const SOUND_MANIFEST
export function noticeSoundId(msg)
```

`match_notice_presenter.js`
```js
export class MatchNoticePresenter {
  constructor({toast, minimap, audio, isReplay, isSpectator, pointInViewport, now?})
  present(event) -> boolean              // fan out one existing server Notice when admitted
}
```
`Match` owns one presenter per match and injects the toast, minimap, persistent audio engine, and
dynamic viewer/viewport predicates. The presenter owns only existing server `Notice` events; it
does not create advisory or economy notices. Under-attack incidents use 960 px map buckets and a
10-second match-scoped cooldown, with one admission decision gating toast, minimap, and voice
together. An admitted in-viewport incident still toasts and pings but stays silent and consumes the
same cooldown; a distinct bucket remains immediately eligible. Replay viewers and live spectators
still receive admitted toast/minimap presentation but never player notice audio.

Every existing notice voice selected by the presenter passes `duck: true`, including informational
voices that retain the `ui` category and informational visual severity. The audio engine also keeps
`alert` as a backward-compatible default ducking category. Duck depth is counted per active ducking
voice; ambient drops by 12 dB and combat drops by 10 dB over 0.08 seconds, then both restore over
2.0 seconds only after the last ducking voice ends. Presenter-admitted under-attack voices bypass
the generic spoken cooldown because the presenter is the sole owner of their incident admission.

Spatial voices in `combat_self` and `combat_other` share a combat-only radial profile. Its acoustic
reference distance `a` is the renderer-neutral listener reference distance capped at 1280 world
pixels, so zooming farther out changes visual framing without expanding the foreground combat mix.
Gain stays at 1.0 through `0.4a`; beyond that, effective distance grows four times as fast, yielding
0.5 gain at `0.5a` and about 0.143 at `1.0a`. Low-pass interpolation and a 0-to-30 voice-priority
penalty both advance from `0.4a` to the `1.2a` hard-drop boundary. Active voices retain their
category so camera updates recompute the same profile with the existing 30 ms ramps. Non-combat
spatial voices retain the original renderer-relative envelope.

The authoritative snapshot's 32-tile-quantized `worldCombatPosition` gates one fixed
`combat_distant_bed_01` loop on the `combat_other` bus. Its direction-only spatial profile pans
toward that coarse world point while holding distance gain at 1.0, so camera zoom and map distance
never change its 0.035 voice gain. It has no pitch variation, fades in over 750 ms, and fades out
over 2500 ms. One stable key prevents voice-pool multiplication and allows 30 ms repanning without
restarting; pause, ended-room-time, teardown, and match replacement stop it. The fixed loop reveals
broad battle direction but not exact position, weapon mix, cadence, ownership, or number of fights.
The current derived asset is a first-pass listening placeholder.

`hud.js`
```js
export class HUD {
  constructor(rootEl, state, commandIssuer, audio?, hotkeyProfiles?, clientIntent?, controlPolicy?, camera?)
  update(frameViews?)                    // refresh resources/supply, minimap status, selection, commands
  // command card buttons call commandIssuer.issueCommand(...) or ClientIntent facade methods
}
```
The minimap status row shares its width between the game timer and an idle-worker tab capped at
half the minimap width. The tab counts live local Workers whose authoritative activity is `idle`,
disables when none are available or the command surface is read-only, and selects the current idle
set through normal command-supply admission when clicked.

The train command card is driven by the first selected production building type, but train clicks
are issued to the selected completed compatible production buildings in round-robin order so a
multi-building selection spreads queued units across its producers. Train and production-cancel
hotkeys honor native keyboard repeat: after the OS repeat delay, repeated `keydown` events activate
only those repeatable command-card buttons. Legal manual build, train, and research buttons remain
actionable while their red cost is unaffordable: build enters placement and relies on the worker's
authoritative wait at the site, while train/research append an unpaid queue entry. Selected producers
render `prodWaiting` as a striped zero-progress bar labeled `waiting for resources / supply`, and
progress extrapolation stays disabled until the server reports the item paid. Alt-clicking a train button or pressing Alt with its
resolved hotkey adds that unit to one selected compatible producer's ordered standing repeat list;
holding Shift with the same gesture removes it from one producer. The server applies each signed
adjustment atomically so rapid inputs allocate distinct producers from current authoritative state.
Each train button shows the authoritative active/compatible producer count (for example `2/3`) and
renders one gold rotating autocast swirl per active producer, evenly phase-offset around the ring.
The server spreads additions toward the least-loaded producer and removes from the most-loaded one,
which balances mixed unit ratios while preserving another automatic order when possible. When more
than one unit is active on a building, it cycles through that list after each successful automatic
enqueue. A repeated unit already inserted in the FIFO stays ahead of later manual clicks, and any
Cancel clears the affected producer's repeat state. Standing repeat controls never create unpaid
queue entries; their swirl remains a policy indicator until a fully funded item is admitted.
Research buttons that unlock production appear directly
below the production button they unlock and disappear once complete. Cancel walks selected producing
buildings in reverse round-robin order for the displayed producer type. The Scout Plane affordance
is a Command Car world-point ability on the `C` grid slot, beside Breakthrough. It costs 50 steel
and 75 oil, has no City Centre requirement, disables while that Command Car has an active Scout
Plane or its 30-second cooldown is running, and issues immediately rather than entering a
building production queue. Scout Planes are hit-testable for hover/readout purposes but normal
selection, box selection, control groups, right-click commands, and command-card descriptors filter
them out, so they are unselectable and uncontrollable in live play. While the Scout Plane ability is
armed, the ground overlay draws an advisory line from the launching Command Car to the cursor and a
dotted ring around the car for the plane's maximum 20-second travel distance; targets outside that
ring remain valid and produce sorties that expire before arrival.
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

Internally, `config.js` is a facade over `config/timing.js`, `config/presentation.js`,
`config/rules_mirror.js`, and `config/factions.js`. Runtime modules should keep importing
`config.js`; internal config modules are in the pinned `rules-mirror` area and may import only
`protocol.js` or same-area config modules.

`minimap.js`
```js
export class Minimap {
  constructor(canvasEl, state, camera, fog, commandIssuer, inputRouter?, {clientIntent?, commandsEnabled?})
  render(frameViews?)                    // draw terrain + fog + entity blips + viewport rect
  markArtilleryFiring(event)             // transient global artillery icon from artilleryFiring events
  inputZone()                            // router zone for locked/unlocked minimap interaction
  // click/drag -> camera.centerOn or issue move command (right-click)
}
```
`commandsEnabled` may be a boolean or a zero-argument predicate. When `state.controlPolicy` is the
lab policy, the minimap uses that policy so lab operators can issue minimap commands even though
their start payload remains spectator-shaped.

`lobby.js`
```js
export class Lobby {
  constructor(rootEl, net)
  show(), hide()
  // owns lobby state, pre-join browser polling, latest-row join preflight,
  // ready/start/spectator role, and delegates browser DOM to lobby_browser_view.js and
  // joined-roster DOM to lobby_view.js.
  // Host lobby controls expose grouped team cards, per-seat team assignment, team-scoped AI add
  // buttons, and a map selector in the lobby summary row through Net setTeam/addAi/selectMap.
  // Replay lobbies are keyed by explicit `kind: "replay"` metadata: the joined view hides
  // Ready, team, faction, AI, map-selection, and active-seat controls, then shows only
  // spectator occupants plus the host start control while the server reports canStart.
  // The normal product lobby exposes an Open Lab route affordance instead of a debug setup toggle.
  // Teams are layout groups only; player colors come from each player record.
  joinReplayLobby(room)                  // join a persisted replay staging lobby as spectator
  onGameStart(cb)                        // main.js subscribes to transition to game screen
}
```

`lobby_browser_view.js`
```js
export const LOBBY_BROWSER_POLL_MS
export function sortLobbySummaries(rows)
export function formatLobbyAge(createdAtUnixMs, nowMs?)
export function lobbyStatusLabel(joinState)
export function lobbyActionLabel(joinState)
export function lobbyJoinIntent(row)
export function validateLobbyName(rawName)
export function suggestLobbyName(playerName)
export class LobbyBrowserView {
  constructor(rootEl)
  render({ rows?, loading?, connected?, error?, nowMs?, actionsDisabled?, onCreateLobby?, onJoinLobby? })
  destroy()
}
export class LobbyCreateModal {
  constructor(hostEl, { onSubmit? })
  open(trigger?, { initialValue? }?)
  close({ restoreFocus? }?)
  setError(message)
  setPending(pending)
  destroy()
}
```
The pre-join lobby browser keeps in-progress rows labeled `In match` and exposes a spectator
action for `joinState: "inGame"`; clicks still preflight against `GET /api/lobbies` before sending
`join` with `spectator: true`. Replay rows are detected from explicit `kind: "replay"` metadata,
labelled `Replay`, and joined as spectators without setting `replayOk`, so rooms that race into
playback still use the normal replay join confirmation. Countdown, stale, and unknown rows remain
disabled.

`match_history.js` renders the API-provided **Replay #** for each visible Recent Matches row. The
server calculates that one-based sequence across the full filtered history, oldest first, while
the table remains newest first; therefore the latest visible replay has the highest number. Saved
debug or solo replays do not consume a number. It launches persisted match replays by POSTing
`/api/matches/{id}/replay`, then hands the returned
`__match_replay__:*` room to `App`/`Lobby.joinReplayLobby` instead of redirecting the page into
replay playback. Direct `replayArtifact` URLs still auto-join the saved artifact playback path;
`replayRoom` URLs represent replay staging lobbies.
The replay lobby UI is group-watch only: future playable resume work needs separate seat-claim
controls and must not infer playable seats from replay lobby occupants or hidden active rows.

`main.js` starts `App`; `app.js` owns the persistent `Net` and `Audio`, derives the ws url from
`window.location`, and shows `Lobby`; on `start` it creates `Match` or `ReplayViewer`. `match.js` builds
`GameState`, `ClientIntent`, `Camera`, `Renderer`, `Fog`, `HUD`, `MatchInputRouter`, `Minimap`,
`MatchNoticePresenter`, `Input`, starts the rAF loop
(compute `alpha` from snapshot timing, `camera.update`,
`audio.setListener`, `input.update`, `buildFrameEntityViews`, `fog.update`, `renderer.render`,
`hud.update`, `minimap.render`); on each snapshot it applies state and triggers transient event
audio exactly once; on `gameOver` show the victory/defeat overlay with the frozen score table. The score table
includes a Team column, highlights every row matching `winnerTeamId`, and falls back to `winnerId`
for singleton FFA compatibility.
For spectator starts without command-surface permission, `match.js` hides the command card and
give-up action, computes local fog from the server-filtered union snapshot, and keeps the ordinary
renderer/minimap/HUD pointed at snapshots with `playerResources`. Lab operators are the exception:
their projection remains spectator-shaped, but `LabControlPolicy.canUseCommandSurface(state)` keeps
the selected-unit panel and real command card visible while prediction stays disabled and issue-as
remains the command authority. Spectators still receive admitted notice toasts and minimap alert
pings, but the match-owned notice presenter suppresses notice audio so observers do not hear player
callouts. Repeated under-attack events in one match-scoped incident are admitted once across toast,
minimap, and audio together.
`artilleryFiring` events are forwarded directly to `Minimap.markArtilleryFiring`; the minimap draws
the artillery rig icon above fog for every recipient without using it as entity visibility.

`Match` composes a render-only clock and injects it into `Renderer`. Normal play reads monotonic
`performance.now()` with the prior semantics. Interact fixed capture may explicitly suspend
the ordinary rAF loop, replace only that render clock with a monotonically advanced capture clock,
render with interpolation disabled, and then restore the normal clock and rAF ownership. Renderer
rig sampling, deployed-weapon transitions, frame strips, recoil, command feedback, smoke,
projectiles, impacts, muzzle flashes, and miss toasts use this clock. Snapshot receipt stamps,
network latency, frame health/profiling, input deadlines, audio, daemon idle, and browser/server
timeouts deliberately remain on real monotonic time; fixed capture never patches
`performance.now()` globally.

The initial fixed-capture inventory intentionally excludes minimap pings/artillery markers,
pointer/selection debounce feedback, HUD wall-clock labels, and audio. Those surfaces continue to
use real time and are hidden or out of scope for the clean Pixi viewport artifact. Any future
fixed capture of them must extend the injected seam explicitly rather than changing their clocks
as a side effect.

### 4.1a Targeted ability mode (Smoke, Mortar Fire, Point Fire, Blanket Fire, Scout Plane)

`input/commands.js` exposes `_onAbilityTarget` and `_refreshAbilityTargetPreview` for world-point
abilities. When the HUD command card calls `ClientIntent.beginCommandTarget({ kind: "ability", ability })`,
the input module enters targeted cursor mode:
- Pointer moves call `_refreshAbilityTargetPreview`: compute which selected units are eligible
  carriers (`ABILITIES[ability].carriers`), test whether any carrier is within range of the cursor
  or can lock the raw cursor into the Artillery range band from the authoritative origin or the
  origin projected from queued movement and setup stages, update `ClientIntent.abilityTargetPreview`
  for renderer feedback. The Smoke preview radius uses the active command owner's completed
  upgrades, including during Lab control, so Smoke Plus previews the server-created 4-tile cloud.
- Left-click: build a `useAbility` command with the ability name, filtered carrier ids, world
  coords, and the `queued` flag (from Shift). Artillery Point Fire and Blanket Fire still send the
  raw clicked world coords; the server owns effective target locking. The local feedback marker uses
  the client-computed locked point when available. Clear cursor mode unless the resolved
  command-card hotkey is still held for repeated world-point targeting.
- Tapping and releasing the resolved world-point ability hotkey before clicking keeps targeting
  armed until the first unqueued world click. That click issues the ability and clears targeting
  unless Shift is still preserving queued targeting.
- If the selected unit's owner-only ability affordance includes an active return object, the command
  card sends `recastAbility(ability, readyIds, targetObjectId, queued)` directly instead of arming a
  world-point cursor. The server remains authoritative for the availability tick and destination
  validity.
- While the resolved hotkey remains held, repeated left-clicks keep the current selection intact and
  keep targeted mode armed so multi-selected Mortar Teams and Scout Cars can distribute repeated
  point commands without the next click falling back to normal selection.
- Right-click / Escape: cancel cursor mode through `ClientIntent.endCommandTarget()`, including while cursor lock is active.
- Minimap right-click or targeted left-click also fires the same ability command if in targeted
  mode, including Shift queueing and the artillery raw-click/locked-feedback split. Minimap hover is
  allowed to omit full per-gun artillery cone or blanket previews rather than drawing simplified
  feedback that could disagree with world targeting.
Selected owned Mortar Teams also draw dotted firing-range circles even when the Fire command is not
armed. The Mortar Team Fire command-card button shows an autocast swirl while any selected mortar's
owner-only `mortarFire` affordance has `autocastEnabled`; right-clicking that button or pressing
Alt plus its resolved command-card hotkey sends `setAutocast(mortarFire, enabled=<toggle>)` and
does not arm manual targeting.

`client_intent.js` holds `commandTarget` (null or `{ kind, ability }`), ability previews, and a
small local planned-stage map keyed by unit id. The local map stores only pending move, attack-move,
setup, Point Fire, and Blanket Fire stages needed for queued preview origins; it is not serialized
and never replaces server `orderPlan`. `abilityTargetPreview` is null or `{ ability, mouseX, mouseY,
carriers, rangeOrigins, pathOrigins, returnMarkers, hoverInRange, artilleryLocks? }`.
`artilleryLocks` is advisory client data for selected Artillery only: per-gun origin, locked
effective point, future/redeploy facing, and whether the locked point is inside the deployed current
cone. `commandTarget` is a transient UI state;
`abilityTargetPreview` is rebuilt every mouse move from the cursor world position and the current
selection. Server-projected complex
ability world objects are stored separately as `state.abilityObjects` from
`Snapshot.abilityObjects`. They are authoritative, fog-filtered data for return-marker, Magic
Anchor, and line-projectile rendering, so the client must not infer gameplay authority from local
preview state.

Range preview rendering (`renderer/feedback.js`, `_drawAbilityTargetPreview`):
- While in targeted ability mode, draws a dotted range ring (normally `rangeTiles × tileSize`) around
  each eligible carrier. Scout Plane instead derives its advisory ring from plane speed multiplied
  by its total lifetime; the ring does not clamp or reject farther targets.
- `rangeOrigins` keeps normal range rings tied to carrier units, while `pathOrigins` adds the
  Command Car-to-cursor Scout Plane route and can add server-projected origins such as Magic Anchors
  for multi-origin line-shot previews.
- `returnMarkers` can draw owner-visible dash-return markers while the dash ability is armed.
- Point Fire and Blanket Fire draw the current artillery cone when the locked point is inside a
  deployed gun's cone, otherwise they draw the future setup/redeploy cone toward the locked point.
  Blanket Fire also draws the 15-tile blanket radius around each locked center.
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

Ground decal rendering (`state_ground_decals.js`, `renderer/decals.js`; layer `decals` between
terrain and resources):
- Decals are client-only, best-effort visual state derived only from received fog-filtered `death`,
  `mortarImpact`, and `artilleryImpact` events. They are not persisted in the protocol, replay
  artifacts, match history, or server sim.
- Infantry deaths stamp translucent player-tinted SVG paint masks. Vehicle and support-weapon
  deaths stamp neutral charcoal hull-shaped scorch masks with smaller, subdued player-colored paint
  fragments. Destroyed buildings stamp neutral charcoal rectangles exactly matching their rendered
  `footW` × `footH` footprint at the map's tile size, with a charcoal core eroded by deterministic
  soot, edge bites, and ash breakup rather than straight fade bands, but no player-color fragments.
- Mortar impacts stamp a compact, air-burst-style starburst with a small dark center; artillery
  impacts stamp a larger starburst scaled to their authoritative impact radius. Both are neutral
  earth/charcoal marks, with no source owner or hidden-source recovery.
- `GameState` queues only unpainted death ids and received impact records. Match stages the pending
  batch before presentation assembly and releases it only after a successful backend presentation;
  Pixi stamps detached frame records and never consumes the shared queue. A skipped snapshot or
  reconnect may miss older decals; the client must not infer them.
- The renderer stamps each new-decal batch into one downsampled texture, updates that texture once
  per stamped batch, and draws the accumulated marks as one sprite. Old decals are not iterated or
  redrawn during normal frames.
- `Renderer.groundDecalDiagnostics()` exposes the permanent layer's stamped count, pending count,
  texture update count, texture dimensions/downsample, layer child count, and asset-load status for
  contract tests and local profiling.
- `Renderer.destroy()` clears the decal texture/canvases and cancels late atlas loads so rematches
  start with a fresh blank decal layer.

Trench terrain rendering (`renderer/trenches.js`; layer `trenches` between ground decals and
resources):
- The latest authoritative `state.trenches` snapshot is rendered into one downsampled canvas texture
  and mounted as one sprite, following the permanent ground-decal path used for blood and scorch
  marks. Reconnects, replay seeks, and fog-memory updates therefore restore trench ground from
  snapshots instead of relying on client-only historical effects.
- The trench texture is redrawn only when the normalized trench snapshot changes. Normal frames do
  not iterate or redraw old trench pixels, so hundreds of accumulated foxholes stay one display
  object and one cached texture.
- Trenches draw as separate neutral brown circular foxhole decals: opaque low-poly dirt footprints
  with interior dark facets for depth and subdued dirt highlights. They do not draw yellow rim
  strokes, exterior drop shadows, oval footprints, or deterministic connection strokes between
  nearby foxholes.
- The renderer skips invalid records and does not create per-trench display objects.
- Occupied eligible infantry also draw a small brown rim marker around the unit using the existing
  selection-ring pool. This marker is a provisional readability pass and does not replace selection
  color, HP, ownership, or fog rules.

Smoke rendering (`renderer/feedback.js`, `_drawSmokes`; layer `smokes` between unit bodies and
selection rings):
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

### 4.2 Rendering & look (PixiJS, SVG rigs — neutral PS1 field-command style)

This section owns the current Pixi look and module behavior. Renderer-neutral camera,
presentation, ownership, capture, backend, parity-gate, and benchmark contracts live in
[client-rendering.md](client-rendering.md) and its active
[rendering parity ledger](rendering-parity.md).

- Minimap player-owned unit and building blips render above resource blips with a merged one-pixel white outline mask for clustered-icon readability. Their 1.6× maximum size scales linearly from 50% to 100% using supply for units (Rifleman/Worker through Tank) and total Steel + Oil cost for buildings (Tank Trap through City Centre), clamped at both ends; resource blips retain their original size. Legacy vision-only intel uses the same kind-specific scale but renders below the fog overlay and does not use the foreground outline/resource-overlap pass.
- Layers (back→front): terrain → ground decals → trench terrain → local visual samples → resource nodes → building shadows → buildings →
  building overlays → unit shadows → occupied-trench shadows → occupied-trench lips → units → smoke/ability ground effects → selection rings →
  health bars → fog overlay → local visual-sample labels → shot-revealed units → observer map-analysis diagnostics → command/hover feedback and miss toasts → placement ghost →
  selection drag-box → (HUD is DOM, not Pixi). Occupied-trench shadows and lips render only when the
  snapshot's `occupiedTrenchId` matches authoritative trench terrain; orphaned ids do not synthesize
  client-only trench geometry. The below-unit occupied-trench berm cue uses stroked irregular
  outlines, while the foreground lip uses a filled front-half arc so the berm does not cover unit
  bodies. Miss toasts use reduced text and stroke sizing, a smaller horizontal offset, and less
  upward drift so they remain closer to the receiving unit. Selected unit range rings,
  minimum-range rings, and support-weapon field-of-fire overlays use higher-opacity rendering for
  readability.
- Spatial combat audio caps its acoustic reference distance at 1280 world pixels so extreme
  zoom-out cannot expand the foreground mix. It stays full volume through 0.4 acoustic reference
  distances, attenuates and muffles toward a hard drop at 1.2 distances, and receives a monotonic
  distance-priority penalty outside the near region. A separate global, 32-tile-quantized combat
  point gates and pans one quiet generic combat-bed loop at constant gain, so distant activity
  remains perceptible and broadly directional without preserving exact event location or
  composition. Non-combat spatial behavior and the global 48-voice pool remain unchanged.
  Panzerfaust launch and impact events use dedicated low-gain spatial cues with coarse cooldown
  buckets; generic Panzerfaust attack events, projectile travel, reload, and legacy conversion
  events stay silent so the weapon does not reuse Tank/Rifleman/artillery sounds or spam clustered
  fights.
  Existing spoken server notices explicitly duck ambient and combat buses while they play; nested
  notice voices hold the duck until the final voice ends, then release over two seconds.
  Tank coax `weaponKind` feedback uses machine-gun burst audio instead of Tank cannon audio, and it
  does not register as a sustained Machine Gunner loop. Tank coax tracers use a bright machine-gun
  line with a hot core, smaller muzzle flash, and tail styling. The coax barrel is a short separated
  stub. Artillery `weaponKind` feedback uses the artillery firing sound instead of the generic rifle
  fallback, and the optional weapon hint is forwarded to recoil timing. Authored main-cannon and
  coax muzzle anchors use sampled rig-part transforms so feedback origins follow the visible barrel
  tip during recoil scale and kick.
- Buildings: SVG-authored rig definitions are compiled at Renderer startup and rendered on the
  buildings layer; shadows remain imperative draws, production progress bars, queue labels, and
  icons remain imperative draws on the building overlays layer, and construction/deconstruction
  status uses the shared HP bar layer.
- Units: SVG-authored rig parts rendered into Pixi containers, with fully covered routes optionally
  rendered from a PNG atlas. Rifleman and Machine Gunner PNG movement frames advance only when
  a fresh authoritative movement sample arrives or their rendered position changes. Observed movement
  remains latched for 100 ms so 60 FPS rendering does not alternate movement and idle art between
  30 Hz snapshots; paused, blocked, or otherwise stationary units then return to idle art while firing
  recoil frames remain active. The Anti-Tank Gun uses a composed white-base PNG atlas for its
  carriage, barrel assembly, and deployed trail legs while retaining the SVG rig as its animation
  anchor source. It uses toned-down team tinting, with most firing recoil on the barrel assembly
  and only subtle kick on the frame and legs. Adjusted frame-strip color texture loading falls back to the raw Pixi
  asset path when image, canvas, pixel-read, or texture creation fails. When browser image
  dimensions are unavailable, full strip dimensions come from frame metadata. Deployed Machine Gunners use `firingFrames` during active recoil, with the visual-effect buffer's linear recoil phase advancing the clip through rest, recoil, and reset frames. Setup and deployed frame-strip poses take priority over movement frames
  while support weapons are deployed or tearing down. When a setup/deployed Machine Gunner snapshot
  lacks `weaponFacing`, the frame-strip setup forward-angle offset is applied to the unit body facing. When an enabled atlas is rendering, routes omitted from a partial PNG
  atlas continue through the SVG runtime; otherwise the normal SVG route remains intact. If visual
  override registry or selector resolution throws, the renderer records diagnostics, publishes
  local visual-profile errors, and renders the normal unit art for that frame. SVG fallback entries
  are removed when an entity id no longer needs them. Both runtimes share one
  sampled render context per entity draw so renderer-local motion state advances once. Units use low-detail hard-edged silhouettes tinted by player color, a dark
  drop shadow, dark outline, HP bar above when damaged/selected, and glowing selection ring when
  entrenched units retain their player-color tint while scaling down. Occupied trenches add shadow
  and lip overlays around live units; empty trenches retain only the base decal.
  selected. When the in-match Game settings
  tab enables unit ranges, selected ordinary units draw dotted firing-range circles, deployed
  Anti-Tank Guns and artillery draw field-of-fire wedges, and their packed states do not draw
  field-of-fire overlays. In Lab scenario authoring, deployed Anti-Tank Gun and artillery
  field-of-fire wedges remain visible for the currently selected owner even when the broad unit
  range overlay is off.
  Distinct silhouette per kind (engineer: compact block; rifleman: enabled PNG frame-strip
  experiment with frame 0 idle, frames 1-4 moving, and frame 5 standing recoil; machine gunner: enabled PNG frame-strip
  experiment with carried movement frames and setup/deployed frames; Panzerfaust: distinct loaded infantry
  rig; Anti-Tank Gun: wheeled gun; mortar team: crewless
  M1938-inspired small wheeled mortar that travels low and deploys upright; scout car: boxy
  WW2-style truck silhouette with enclosed wheels and a rear-top machine gun; tank: chunky
  flat-shaded armor with movement-facing tracks, hull, nose, and shadow plus weapon-facing turret,
  main barrel, coax barrel, recoil, nose tick, and low-oil/oil-starved fuel cues; artillery: SVG-authored
  support-weapon rig routed through the live renderer).
  Riflemen carry a rifle, loaded Panzerfaust infantry carry a tube launcher with a team-colored
  band, Anti-Tank Guns field a wheeled anti-tank gun with a long recoiling barrel,
  carriage, two wheels, and animated deployment bracing, and machine gunners carry an MG42-style
  long machine gun across the body while packed that extends forward with bracing during
  setup/deployment. Units that fire from outside current vision are shown briefly above the fog
  as semi-transparent silhouettes with their normal art path, rig-authored recoil where available,
  and a yellow tracer to the hit point.
  PNG frame-strip units use a shared load-time color profile target in
  `renderer/rigs/frame_strip_color_profile.js`; each strip records any brightness/saturation already
  baked into its checked-in runtime PNG, and raw strips receive the missing delta once when the
  texture loads. Individual strips can set a lower or higher target when their generated source art
  needs unit-specific visual matching.
  PNG atlas rigs can also declare a small `runtimeColorAdjustment` so generated vehicle art can be
  matched in-game without rewriting the atlas source PNG.
  Attack `weaponKind` selects feedback scale and rig muzzle origin; TankCoax uses a small
  machine-gun flash/tracer/tail from the authored coax muzzle anchor and no Tank recoil, while
  TankCannon or default Tank attacks use a direct tracer from the main muzzle anchor plus the
  tank rig's half-scale artillery-style muzzle flare.
  Direct-shot `miss` events draw a tiny `Miss!` Pixi text toast above and to the right of the
  receiving unit on the feedback layer. The client anchors it only to the current projected entity,
  so hidden or no-longer-visible targets do not gain inferred positions.
  Mortar launch events draw launch dust/recoil for recipients that can see the mortar, a black
  shell arcing from the mortar to the impact point, and a darker red dotted line/crosshair warning
  that lasts until the reported shell delay elapses or the impact event arrives. The shell
  compresses near launch and impact and stretches near mid-flight so it reads as an overhead round
  rather than a flat tracer. Mortar impacts draw a larger, denser, longer-lived dust cloud with an
  orange-yellow blast core that fades before the dust so battlefield state remains readable. Mortar
  Team art uses the enabled PNG wheeled-mortar atlas with a team-tinted carriage/frame and
  tube/barrel assembly, fixed-color tire overlays, and separate carriage/tube recoil bindings.
  Mortar impact events that include a shooter reveal show the mortar briefly above fog for players
  whose units or buildings were hit by indirect fire.
  Entities marked `visionOnly` by the server are drawn on the ordinary building/unit layers below
  the fog overlay and excluded from local fog-source computation and selection/command hit-testing.
  Current death-vision entities are normal visible entities and do not use this flag.
- Buildings: footprint-sized blocky field structures with neutral geometry and plain
  two-letter stencils; under construction → translucent with a single HP-layer status bar;
  production → small top-edge progress bar. Tank Trap deconstruction uses that same HP-layer status
  bar and drains from full to empty via `deconstructProgress`. Tank Traps render as neutral steel I-beam
  hedgehogs with deterministic per-id rotation. Owned scaffolds may locally extrapolate
  `buildProgress` only while the latest authoritative snapshot marks them `buildActive`; the display
  clamps below completion and never unlocks supply, tech, production, pathing, or command behavior
  before the server snapshot. Live pause state freezes both construction and production progress
  extrapolation for every recipient, and the active display clock resumes from the first
  authoritative post-unpause snapshot without counting paused wall time. Completed damaged/selected
  buildings use the same HP-layer bar for normal health.
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
- Terrain: muted grass/field/mud, rock, water, and deep charcoal-brown road tiles with deterministic
  coarse dithering so movement is readable and the map has a PlayStation 1-era low-resolution
  texture feel. Every road side exposed to non-road terrain has a narrow brown earth shoulder with
  deterministic chips that break up the boundary without softening it into a modern blur. Road uses
  one bare tile plus horizontal, vertical, NW-SE diagonal, and NE-SW diagonal tiles with a simple
  yellow center-line segment. Authors intersperse marked tiles among bare centerline tiles to form
  dashed markings while bare road tiles fill the surrounding surface.
- Local lab visual profiles may pass renderer-only static samples to `Renderer.render`. Static
  trench samples draw on the local visual-samples layer, labels draw as world-anchored Pixi text
  above the fog overlay, and neither path writes to `GameState`, snapshots, fog-source entity lists,
  selection, command targeting, minimap blips, or scenario authoring data.
- Local lab visual profiles may also pass per-instance real-unit visual overrides to
  `Renderer.render`. These override only the candidate SVG rig definition used by real unit drawing,
  keep runtime inputs such as movement, facing, weapon facing, recoil, setup state, occupied-trench
  tint, selection, HP bars, fog, and shot reveals on the real entity, and are resolved from checked-in
  profile/candidate registries rather than URL-provided assets.
- Local lab visual profiles may pass checked-in frame-strip overrides by unit kind. These swap the
  PNG strip texture/configuration used for matching real units in that lab session only, while the
  entity ids, server state, selection, fog, HP bars, and fallback SVG shadow route remain unchanged.
- Fog: unexplored = 80% dark overlay so terrain remains faintly readable; explored-but-not-visible =
  48% dark overlay; visible = clear. Use a single overlay sprite/graphics updated from `fog`
  grids; soften edges if cheap.
- Selection: green for own, red tint for enemy, yellow for neutral. Drag-box translucent green.
- Renderer failures must fail soft: one broken entity or feedback effect should log a throttled
  `[RTS_RENDER]` error, skip that visual path, and let the rest of the frame continue. Broken
  entity art draws a magenta/black checkerboard fallback instead of stopping the match loop; the
  frame loop also reschedules after unexpected client errors and logs `[RTS_FRAME]`.
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
- `app-shell`: `main.js`, `app.js`, `match.js`, `match_combat_audio.js`,
  `match_notice_presenter.js`,
  `match_net_reporter.js`, `match_observer_diagnostics.js`, `match_settings_context.js`,
  `match_settings_toggles.js`, `match_auto_spectator.js`, `auto_spectator.js`,
  `client_perf_report.js`, `match_health.js`,
  `frame_profiler.js`, `frame_recovery.js`, `frame_entity_views.js`, `live_pause_overlay.js`,
  `ai_diagnostics_panel.js`, `observer_analysis_overlay.js`, `observer_analysis_ai.js`,
  `observer_analysis_preferences.js`, `observer_analysis_rows.js`, `observer_analysis_signatures.js`,
  `floating_panel_positioner.js`, `replay_controls.js`,
  `room_time_panel.js`, `replay_viewer.js`, `lab_control_policy.js`, `room_capabilities.js`,
  `visual_profiles.js`. App's browser leave confirmation is scoped to active running live-player matches; spectator, Lab, replay, and resolved/stopped sessions leave without the prompt.
- `model`: `state.js`, `state_queries.js`, `state_visual_effects.js`, `client_intent.js`,
  `command_budget.js`, `command_composer.js`, `progress_extrapolator.js`,
  `prediction_controller.js`, `prediction_compatibility.js`, `sim_wasm_adapter.js`.
- `transport`: `net.js`, `protocol.js`, `lab_client.js`.
- `rules-mirror`: `config.js` plus `config/timing.js`, `config/presentation.js`,
  `config/rules_mirror.js`, and `config/factions.js`.
- `ui`: HUD, command card descriptors/selection panels, hotkey profiles/editor, lobby
  controller/browser/roster views, match history, minimap, resource icons, scoreboard, status badge, branch
  staging, lab panel, lab setup authoring/submission helpers, settings. Command-card tooltips render optional unit descriptions when descriptor metadata provides them. Command discovery includes the completed-Medium-Guns command-card context so the direct-hotkey and settings catalogs include Heavy Guns after it replaces Medium Guns in the Q slot. Wait-until-ready ability descriptors remain available for queue-admissible carriers, including Mortar Fire while it is cooling down. Lab research controls render direct per-upgrade toggle buttons for the selected Lab target player; completed upgrades render as pressed buttons with a check-mark background. The Lab panel window toggle button shows Collapse when expanded and Expand when collapsed. Lab and room-time panel collapse controls activate immediately on touch release, suppress the follow-up synthesized click, cancel pending activation when the pointer leaves or cancels, and reset activation state on teardown or re-render. On narrow viewports, the default Lab Options and Tools headers sit below the expanded room-time controls, and their collapse buttons remain touch-friendly. Mobile restore ignores saved desktop coordinates for both Lab windows and room-time controls while preserving saved collapsed state. The settings panel uses the in-match header action slot for Give Up
  in live matches and Back to Lobby in Lab/replay sessions. After a finished match, App resets the
  Lobby controller to the root browser before showing the lobby screen again. Lobby AI creation is
  exposed from the roster's team context, not as a duplicate global sidebar action. The in-match
  debug status badge displays live and rolling one-minute FPS metrics from `MatchHealth`.
- `input`: `input/` plus `replay_camera_input.js`; `input/camera_navigation.js` is the shared
  command-free camera gesture helper for live input and replay/observer wrappers. Locked native-cursor routing sends clicks through both settings chrome and the game screen, and preserves pressed-button state across macOS drag events for routed pointer moves and gameplay selection drags.
- `renderer`: `renderer/`.
- `platform`: bootstrap, including the lobby Open Lab entry point to bare `/lab`, `/lab` catalog
  route detection, direct launch URL parsing for scenario/map/seed and sanitized lab visual profile
  ids, audio, combat audio, alerts, fog, camera, prediction settings, unit range settings,
  auto spectator settings,
  `report_window_aggregate.js`.

Import rules:
- `protocol.js` and the `config.js` facade are shared mirrors and may be imported where needed.
- Internal `config/` modules are for the facade and same-area config imports; runtime modules should
  not bypass `config.js`.
- Files in the same area may import each other.
- `app-shell` files may compose other areas; prefer adding new cross-area wiring in `match.js` or
  `app.js` instead of importing collaborators from feature modules.
- Lab UI and transport lifetimes stay in `App`: `match.js` may receive lab metadata/control policy,
  but must not import `lab_client.js` or `lab_panel.js`. Lab setup tools may include inspection
  spawn entries such as Panzerfaust; normal faction catalog membership and production command-card
  exposure are still owned by the Rust rules catalog and client rules mirror.
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
- Did this touch `protocol.js` or Rust-owned values behind the `config.js` facade? Update the
  mirrored server file and the relevant design/context docs in the same change.

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

### 4.4 Launch URL conventions

App-owned launch URLs use the `rtsLaunch` query parameter as the mode switch. `rtsLaunch=match`
drives a normal live lobby through existing lobby messages rather than introducing a debug protocol:
the browser joins the requested room, applies safe setup options, and starts only after ordinary
server `lobby.canStart` is true. The convention is intentionally namespaced so future launch modes
can coexist with replay, lab, and dev-scenario URLs.

Supported match-launch parameters:
- `rtsRoom=<room>` optional public room name. If omitted, the client generates a safe AI self-play
  room name. Reserved internal room prefixes are rejected.
- `rtsRole=spectator|player` defaults to `spectator`.
- `rtsName=<display name>` defaults to `Spectator` or `Commander` from the selected role.
- `rtsMap=<map display name>` optionally selects a server-advertised lobby map before seating AIs.
- Repeated `rtsAi=<team>:<profile>` entries seat AI opponents in order, for example
  `rtsAi=1:ai_2_1&rtsAi=2:ai_turtle`. The supported profile ids are `ai_2_1` and
  `ai_turtle`. A profile-only entry such as `rtsAi=ai_turtle` uses the next team slot.
  Unsupported values normalize to AI 2.1; if omitted, the launch defaults to two AI 2.1 seats on
  teams 1 and 2.
- `rtsStart=1|0` defaults to `1`. `0` prepares the lobby without pressing Start.

Example spectator self-play URL:

```text
/?rtsLaunch=match&rtsRoom=agent-ai-selfplay&rtsRole=spectator&rtsAi=1:ai_2_1&rtsAi=2:ai_turtle&rtsStart=1
```

When that launch produces an all-AI live match, the score screen retains the live `matchRunId`
through automatic post-match replay playback and renders it as an Observation ID. The id is the
handoff key for replay recovery and server lag-log inspection; it is not a player identity or a
new gameplay control.
