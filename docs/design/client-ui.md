## 4. JS client — modules & exported APIs

`client/` (ES modules, no bundler; `index.html` imports `src/main.js` as a module).
PixiJS is loaded globally from CDN as `PIXI`.

```
index.html        # PINNED — CDN + #app + module entry + screens markup
map-editor.html   # standalone handcrafted-map editor; terrain/base symmetry plus JSON load/save
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
  renderer/       # Pixi app facade plus layers, terrain, entities, units, buildings,
                  # decals, resources, fog overlay, feedback, rig schema/import, and renderer-local palette helpers
  renderer/decals.js # GroundDecalLayer permanent decal texture, stamping, diagnostics, teardown
  renderer/decals/ # SVG decal atlas manifest, loader, and deterministic stamp selection
  renderer/trenches.js # Authoritative trench terrain pass and deterministic nearby-trench connectors
  renderer/feedback_view_model.js # Builder for renderer feedback's narrow per-frame read model
  renderer/lab_tool_preview.js # Armed Lab tool cursor ghosts plus persistent local map-draft markers
  renderer/observer_map_analysis.js # Observer-only static AI map-analysis world overlay drawer
  fog.js          # Fog overlay: accumulate explored, compute visible from own entities
  input/          # lifecycle facade plus selection, commands, placement, shared camera navigation, UI input routing
  audio.js        # Audio: Web Audio context, buses, one-shots, spatialization
  sound_manifest.js # Stable sound ids and asset URLs
  hud.js          # HUD: resources/supply bar, selected panel, command card (build/train)
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
  observer_analysis_resources.js # resources tab renderer and wire normalization for observer analysis
  observer_analysis_rows.js # observer analysis player row metadata joiner
  observer_analysis_signatures.js # dirty-body signatures for observer analysis DOM updates
  match_observer_diagnostics.js # Match-owned observer/AI diagnostics surface composer
  client_perf_report.js # bounded client frame-profiler upload field shaping
  match_health.js # match network/render health reporter
  frame_profiler.js # bounded client frame phase profiler and debug summary API
  live_pause_overlay.js # live-match pause state overlay and unpause affordance
  branch_staging.js # replay branch staging panel
  lab_catalog.js # LabCatalogScreen: app-owned `/lab` setup/blank selector
  lab_client.js  # LabClient: lab request ids, pending results, state/result subscriptions
  lab_scenario_authoring.js # pure lab setup metadata defaults, slugging, and local validation
  lab_scenario_submission_capability.js # HTTP capability probe with transient-failure retry
  lab_scenario_submission_flow.js # LabPanel scenario validation/submission orchestration
  lab_panel.js   # LabPanel: app-owned lab controls/status UI mounted around Match
  lab_tool_detail.js # Pure armed-tool instruction text for LabPanel status
  lab_panel_window.js # draggable/resizable chrome helper for the app-owned LabPanel
  lab_map_editor_session.js # persistent 25-state authored-map draft, compatible layouts, player-owned bases, and undo/redo history
  lab_map_editor_panel.js # floating draft-first Lab map editor, built-in map loading, explicit test restart, and map JSON export
  lab_control_policy.js # Lab control collaborator placeholder injected into Match
  lab_map_reset.js # in-place authoritative Lab map/player/fog/terrain refresh collaborator
  visual_profiles.js # Lab-scoped visual experimentation profile registry and resolver
  settings_container.js # Reusable settings shell: opener, tabs, focus, teardown
  settings_panels.js # Portable settings tab panel descriptors
  main.js         # Entry point: starts App
  app.js          # Lobby/app shell lifecycle and persistent Net/Audio ownership
  launch_url.js   # Namespaced rtsLaunch URL parsing and pure lobby automation decisions
  match.js        # Match lifecycle, module dependency wiring, render loop, transient events
  match_combat_audio.js # Match-owned combat sound routing and machine-gunner sound cleanup
  match_live_pause.js # live pause state actions and prediction visual suspension
  match_net_reporter.js # Match ping cadence and client net-report upload collaborator
  match_settings_context.js # Match settings action/tab context builder
  frame_recovery.js # Frame-loop soft-failure logging and rescheduling diagnostics
  frame_entity_views.js # One-RAF entity view builder shared by render, fog, HUD, minimap, analysis
  replay_controls.js # Capability-driven RoomTimeControls plus replay-only vision/branch controls
  room_time_panel.js # Floating, draggable chrome around shared room-time controls
  room_capabilities.js # Client-side room capability parser for controls/diagnostics affordances
  alerts.js       # Notice/toast alert ids and viewport alert behavior constants
  bootstrap.js    # DOM lookup, ws/dev-watch/lab launch config, startup helpers
```

The standalone map editor mirrors new terrain edits left-to-right, top-to-bottom, or by a 180°
rotation. Selecting a symmetry mode clears its target half to grass and removes target-side sites
and affected spawn slots: Left ↔ Right keeps the left half, while Top ↔ Bottom and Rotate 180°
keep the top half. If that would remove every complete source-side spawn layout, the editor moves
the required sites to free tiles on the kept side before clearing. With symmetry enabled, moving a
base site also creates or updates its reflected site and the matching slot in the selected spawn
layout. A slot has one main and one to three naturals, for at most four bases per player.

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
  selectMap(map)
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
compatibility. The overlay owns its generated DOM and is read-only. The Army Value tab is
client-side and viewport-specific; Production, Units, Units Lost, and Resources Lost render the
latest server-authored `observerAnalysis` payload. Resources Lost follows the protocol's narrow
definition: spent steel/oil value of units that died, excluding buildings, stockpile changes,
harvesting, refunds, and cancelled queues.

`ai_diagnostics_panel.js`
```js
shouldMountAiDiagnosticsPanel({ capabilities })
createAiDiagnosticsPanelPreferences(storage?)
export class AiDiagnosticsPanel {
  constructor({ root, preferences, getPlayers, onMapLayerVisibilityChange })
  applyObserverAnalysis(payload)          // renders optional per-player aiDiagnostics trace rows
  mapLayerVisibility()                    // current map-analysis overlay layer switches
  destroy()
}
```
`Match` mounts the AI diagnostics panel beside the observer analysis overlay when the room advertises
observer-analysis diagnostics. The panel consumes the same server-authored `observerAnalysis`
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
buildFrameEntityViews(state, { alpha }) // frame-local interpolated/current/authoritative/selected entity arrays
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
  exportScenario(name?)                  // compatibility wire name for checkpoint setup export
  importScenario(scenario)               // compatibility wire name for checkpoint/legacy setup import
  validateScenario(metadata)             // sends {op:"validateScenario", metadata}
  submitScenario(metadata, options?)      // sends {op:"submitScenario", metadata}
  resetScenario()                        // seeks lab room time to the current setup baseline
  request(op, options?)                  // allocates requestId, resolves with labResult/timeout
  destroy()
}
export function labVisionLabel(vision)
export const labVision                   // fullWorld(), team(teamId), teams(teamIds)
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
  constructor({ root, labClient, launch, startPayload, match?, mapEditorSession?, applyLabMapReset?, setLabMapDraftOverlay?, setLabMapDraftTerrainPreview?, submissionCapability?, openWindow? })
  applyLabToolChange(change)             // syncs active/cancelled tool status from Match callbacks
  armSpawnPaletteTool(kind?)             // arms a Match-owned completed spawnEntity world-click tool
  armBuildingSpawnPaletteTool(kind?)     // arms a Match-owned completed building spawnEntity tool
  cancelActiveTool()
  validateScenario(), submitScenario(), exportScenario(), importScenario()
  saveLabReplay(), openLabReplay()        // distinct replay affordances; not legacy scenario ops
  destroy()
}
```
`lab_map_editor_session.js`
```js
export const LAB_MAP_HISTORY_LIMIT       // 25
export class LabMapEditorSession {
  initializeFromStart(startPayload, options?)
  initializeFromScenario(scenario, options?)
  loadAuthoredMap(source, options?), selectLayout(layoutId)
  mutate(label, mutation), undo(), redo()
  materialized(), exportMap(), saveLocal(key), loadLocal(key)
  markCurrentDraftAsTested(), playerSlots(), mapOverlay()
}
```
`LabPanel` renders separate floating, collapsible Options and Tools windows. Options owns room
status, lab vision, command-limit policy, setup authoring metadata, validation, PR submission,
setup import/export/reset, and result status; Tools owns target player, player state, spawn palettes,
active tool status, and the remove setup tool. App passes the HTTP
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
`importScenario` lab operations.
For the Lab map-editor proof of concept, `LabPanel` also composes a third floating Map Editor window.
It is draft-first: the window can load built-in authored maps from `/maps/catalog` (with standard maps
as a fallback) and fetch the selected JSON from `/maps/<file>`, but loading, terrain painting, and base
editing all change only the browser-local `LabMapEditorSession`. A loaded map must match the active Lab
map size; the editor selects a compatible player-count layout while preserving every authored layout for
local save and normal map-JSON export. The editor exposes players and their base locations directly
rather than anonymous main/natural site IDs and slot pickers. A persistent browser-local overlay draws
each drafted start and natural in that player's colour, and a local terrain preview redraws cached ground
without touching authoritative `GameState`; the live Lab test remains unchanged. The terrain controls
paint one tile at a time by click or drag and use matching grass, stone, and water swatches. Local draft
save/load likewise does not alter the test.

`Restart test with this draft` is the one explicit draft-to-test transition. It creates a fresh Lab test
at tick zero with the same players and seed, so it deliberately clears the prior test's units, orders,
resources, and elapsed time even for terrain-only changes. The requesting client consumes the
authoritative map/player payload from `labResult` through the app-owned `applyLabMapReset` collaborator,
replacing static `GameState`, fog, minimap inputs, and the cached terrain texture in place instead of
tearing down and rebuilding the whole match. Ordinary Lab unit/building palettes remain the playtest
setup surface after that explicit restart.
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
operator-controllable owner as a feedback owner so full-world lab overlays such as rally/order
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
selectable ids in the drag box instead. World-click callbacks receive the active tool payload, exact
world coordinates, and any selectable hit entity id. `ClientIntent.labToolPreview` tracks the armed
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
  activeLabTool, labToolPreview, labMapDraftOverlay
  beginLabTool(tool), updateLabToolPreview(preview), setLabMapDraftOverlay(overlay), cancelLabTool(reason?)
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
command-target arming, hover previews, command feedback, ability previews, and the short-lived local
planned-order stages used only for previews while the server echo is pending. Contextual oil
right-clicks compose a Pump Jack build intent on the clicked oil patch rather than a gather
command. `GameState` must not grow compatibility accessors for those intent fields; HUD, input,
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
weapon range metadata. Frame-local entity views may carry bounded render diagnostics for local
profiling consumers without changing the authoritative snapshot model. Visible unit death events are normalized by `GameState`
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
  render(state, camera, fog, alpha, options?) // per-frame; draws entities, fog, selection, placement, Tank Traps
  app                                    // the PIXI.Application (for ticker/stage if needed)
  // exposes screen->world hit info if helpful; selection box drawing lives here too:
  drawSelectionBox(rectOrNull)
}
```

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
  constructor(domElement, camera, state, commandIssuer, renderer, fog, audio?, inputRouter?, hotkeyProfiles?, clientIntent?, labToolController?, desktopCursor?)
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
through `Camera.panByScreenDelta`; touch drag/pinch pan and zoom the camera without emitting
gameplay commands. Mouse-wheel zoom, keyboard pan state, edge scroll state, and blur release are
shared observer navigation behavior. Live spectators still use the live `Input` path so read-only
selection inspection remains available while command emission stays gated by local-owner checks.

Shift-right-click appends queued orders only for selected units: move, attack-move, attack,
gather, build/resume, Tank Trap deconstruct, and placement build commands set `queued: true` and
rely on the server snapshot's owner-only `orderPlan` for accepted markers. Production-building-only right-clicks set
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
If re-enabled, Tank Trap placement uses the same local placement intent, with optional `lineSites`
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
  constructor(rootEl, state, commandIssuer, audio?, hotkeyProfiles?, clientIntent?, controlPolicy?, camera?)
  update(frameViews?)                    // refresh resources/supply, selected panel, command card
  // command card buttons call commandIssuer.issueCommand(...) or ClientIntent facade methods
}
```
The train command card is driven by the first selected production building type, but train clicks
are issued to the selected completed compatible production buildings in round-robin order so a
multi-building selection spreads queued units across its producers. Train and production-cancel
hotkeys honor native keyboard repeat: after the OS repeat delay, repeated `keydown` events activate
only those repeatable command-card buttons. Research buttons that unlock production appear directly
below the production button they unlock and disappear once complete. Cancel walks selected producing
buildings in reverse round-robin order for the displayed producer type. The Scout Plane affordance
is a Command Car world-point ability on the `C` grid slot, beside Breakthrough. It costs 50 steel
and 50 oil, requires a completed owned City Centre, disables while the player has an active Scout
Plane or the global 30-second cooldown is running, and issues immediately rather than entering a
building production queue. Scout Planes are hit-testable for hover/readout purposes but normal
selection, box selection, control groups, right-click commands, and command-card descriptors filter
them out, so they are unselectable and uncontrollable in live play.
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

`match_history.js` launches persisted match replays by POSTing `/api/matches/{id}/replay`, then
hands the returned `__match_replay__:*` room to `App`/`Lobby.joinReplayLobby` instead of redirecting
the page into replay playback. Direct `replayArtifact` URLs still auto-join the saved artifact
playback path; `replayRoom` URLs represent replay staging lobbies.
The replay lobby UI is group-watch only: future playable resume work needs separate seat-claim
controls and must not infer playable seats from replay lobby occupants or hidden active rows.

`main.js` starts `App`; `app.js` owns the persistent `Net` and `Audio`, derives the ws url from
`window.location`, and shows `Lobby`; on `start` it creates `Match` or `ReplayViewer`. `match.js` builds
`GameState`, `ClientIntent`, `Camera`, `Renderer`, `Fog`, `HUD`, `MatchInputRouter`, `Minimap`,
`Input`, starts the rAF loop
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
remains the command authority. Spectators still receive notice toasts and minimap alert pings, but
`match.js` suppresses notice alert audio so observers do not hear player alert callouts.
`artilleryFiring` events are forwarded directly to `Minimap.markArtilleryFiring`; the minimap draws
the artillery rig icon above fog for every recipient without using it as entity visibility.

### 4.1a Targeted ability mode (Smoke, Mortar Fire, Point Fire, Blanket Fire)

`input/commands.js` exposes `_onAbilityTarget` and `_refreshAbilityTargetPreview` for world-point
abilities. When the HUD command card calls `ClientIntent.beginCommandTarget({ kind: "ability", ability })`,
the input module enters targeted cursor mode:
- Pointer moves call `_refreshAbilityTargetPreview`: compute which selected units are eligible
  carriers (`ABILITIES[ability].carriers`), test whether any carrier is within range of the cursor
  or can lock the raw cursor into the Artillery range band from the authoritative or locally planned
  origin, update `ClientIntent.abilityTargetPreview` for renderer feedback.
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
- While in targeted ability mode, draws a dotted range ring (radius = `rangeTiles × tileSize`) around
  each eligible carrier.
- `rangeOrigins` keeps normal range rings tied to carrier units, while `pathOrigins` can add
  server-projected origins such as Magic Anchors for multi-origin line-shot previews.
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
- `GameState` queues only unpainted death ids and received impact records, and `Renderer` consumes
  the pending queue once per frame. A skipped snapshot or reconnect may miss older decals; the
  client must not infer them.
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
- Layers (back→front): terrain → ground decals → trench terrain → local visual samples → resource nodes → building shadows → buildings →
  building overlays → unit shadows → occupied-trench shadows → occupied-trench lips → units → smoke/ability ground effects → selection rings →
  health bars → fog overlay → local visual-sample labels → shot-revealed units → observer map-analysis diagnostics → command/hover feedback and miss toasts → placement ghost →
  selection drag-box → (HUD is DOM, not Pixi). Selected unit range rings, minimum-range rings, and
  support-weapon field-of-fire overlays use higher-opacity rendering for readability.
- Spatial combat audio keeps full volume for nearby emitters, uses stronger attenuation after the
  listener reference distance for distant emitters, and keeps the same hard drop distance.
  Panzerfaust launch and impact events use dedicated low-gain spatial cues with coarse cooldown
  buckets; generic Panzerfaust attack events, projectile travel, reload, and legacy conversion
  events stay silent so the weapon does not reuse Tank/Rifleman/artillery sounds or spam clustered
  fights.
  Tank coax `weaponKind` feedback uses machine-gun burst audio instead of Tank cannon audio, and it
  does not register as a sustained Machine Gunner loop.
- Buildings: SVG-authored rig definitions are compiled at Renderer startup and rendered on the
  buildings layer; shadows remain imperative draws, production progress bars, queue labels, and
  icons remain imperative draws on the building overlays layer, and construction/deconstruction
  status uses the shared HP bar layer.
- Units: SVG-authored rig parts rendered into Pixi containers, with low-detail hard-edged
  silhouettes tinted by player color, a dark drop shadow, dark outline, HP bar above when
  damaged/selected, and glowing selection ring when selected. When the in-match Game settings
  tab enables unit ranges, selected ordinary units draw dotted firing-range circles, deployed
  Anti-Tank Guns and artillery draw field-of-fire wedges, and their packed states do not draw
  field-of-fire overlays. In Lab scenario authoring, deployed Anti-Tank Gun and artillery
  field-of-fire wedges remain visible for the currently selected owner even when the broad unit
  range overlay is off.
  Distinct silhouette per kind (engineer: compact block; rifleman: enabled PNG frame-strip
  experiment with frame 0 idle, frames 1-4 moving, and frame 5 standing recoil; machine gunner: enabled PNG frame-strip
  experiment with carried movement frames and setup/deployed frames; Panzerfaust: shared SVG
  infantry body with a launcher tube whose warhead is hidden while reloading; Anti-Tank Gun: wheeled gun; mortar team: crewless
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
  before the server snapshot. Completed damaged/selected buildings use the same HP-layer bar for
  normal health.
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
  `match_net_reporter.js`, `match_observer_diagnostics.js`, `match_settings_context.js`, `match_settings_toggles.js`, `client_perf_report.js`, `match_health.js`,
  `frame_profiler.js`, `frame_recovery.js`, `frame_entity_views.js`, `live_pause_overlay.js`,
  `ai_diagnostics_panel.js`, `observer_analysis_overlay.js`, `observer_analysis_signatures.js`, `replay_controls.js`,
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
  staging, lab panel, lab setup authoring/submission helpers, settings. Command-card tooltips render optional unit descriptions when descriptor metadata provides them. Lab research controls render direct per-upgrade toggle buttons for the selected Lab target player; completed upgrades render as pressed buttons with a check-mark background. The Lab panel window toggle button shows Collapse when expanded and Expand when collapsed. The settings panel uses the in-match header action slot for Give Up
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
