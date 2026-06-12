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
  state.js        # GameState: holds prev+current snapshot, selection, camera, placement
  camera.js       # Camera: pan/zoom, world<->screen transforms, edge/keyboard/pointer-lock scroll
  renderer/       # Pixi app facade plus layers, terrain, entities, units, buildings,
                  # resources, fog overlay, feedback, and renderer-local palette helpers
  fog.js          # Fog overlay: accumulate explored, compute visible from own entities
  input/          # lifecycle facade plus selection, commands, placement, camera controls, UI input routing
  audio.js        # Audio: Web Audio context, buses, one-shots, spatialization
  hud.js          # HUD: resources/supply bar, selected panel, command card (build/train)
  minimap.js      # Minimap: draw terrain+entities+viewport; click to move camera/command
  lobby.js        # Lobby screen: name entry, player list, ready/start buttons
  settings_container.js # Reusable settings shell: opener, tabs, focus, teardown
  settings_panels.js # Portable settings tab panel descriptors
  main.js         # Entry point: starts App
  app.js          # Lobby/app shell lifecycle and persistent Net/Audio ownership
  match.js        # Match lifecycle, module dependency wiring, render loop, transient events
  replay_controls.js # Replay/scenario speed, seek, vision, and timeline controls
  alerts.js       # Notice/toast alert ids and viewport alert behavior constants
  bootstrap.js    # DOM lookup, ws/dev-watch config, startup helpers
```

### 4.1 Module export contracts

`net.js`
```js
export class Net {
  constructor(url)                       // ws url; auto-derived from location in main.js
  connect(): Promise<void>
  on(type, handler)                      // type ∈ ServerMessage tags + "open"/"close"
  off(type, handler)
  join(name, room)
  ready(isReady)
  start()
  addAi()
  removeAi(id)
  setQuickstart(enabled)
  command(cmd, clientSeq)                // lower-level sequenced gameplay command envelope
  ping()
  setReplaySpeed(speed)                  // replay rooms and dev-watch scenarios
  seekReplay(ticksBack)                  // replay rooms; pass huge N for full reset
  requestReplayBranch()
  claimBranchSeat(playerId)
  releaseBranchSeat(playerId)
  startBranch()
  get playerId()
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
  reset({enabled?})
  debugSummary()                         // pending count/seqs, latest authoritative tick, ack/correction metrics
  get pendingCommandCount()
}
```
Live player command sources receive a `commandIssuer` seam from `Match` and call
`commandIssuer.issueCommand(cmd)`. The controller owns browser-local `clientSeq` allocation and
passes the sequenced envelope to `Net.command(cmd, clientSeq)`. Replay viewers, spectators, and
dev-watch passive viewers keep prediction disabled and do not allocate gameplay command sequence ids.
`GameState.applySnapshot` remains authoritative in Phase 2; prediction diagnostics are bookkeeping
only until a later WASM predictor supplies predicted render snapshots.

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
`description`, `createdWithBuild`, `basePreset`, and `bindings`. Imports validate known command ids
against the current command-card catalog, ignore unknown commands with warnings, reject invalid keys
and same-context duplicates, and store accepted payloads as custom profiles. Untargeted imports
rewrite ids/names to avoid local collisions; targeted imports replace the whole target profile
payload instead of merging individual bindings.

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
  players                                // [{id,name,color,startTileX,startTileY}]
  // snapshot buffering for interpolation:
  applySnapshot(msg)                     // pushes msg, keeps prev+current, stamps recvTime
  entitiesInterpolated(alpha)            // -> entities with lerped x,y,facing,weaponFacing
  get prevRecvTime() / get currRecvTime()// recv timestamps of the two buffered snapshots
                                         //   (null until two exist); main.js derives interp alpha
  resources                             // {steel,oil,supplyUsed,supplyCap} (latest)
  events                                 // latest snapshot's events
  // selection (client-only):
  selection                              // Set<entityId>
  setSelection(ids), addToSelection(ids), clearSelection()
  selectedEntities()                     // resolved entity objects from current snapshot
  entityById(id)
  // control groups (client-only):
  controlGroups                          // ten Array<entityId> slots; slot 9 maps to key 0
  setControlGroup(slot, ids), addToControlGroup(slot, ids)
  selectControlGroup(slot), controlGroupEntities(slot)
  // build placement (client-only):
  commandCardMode                       // null | "workerBuild"
  openWorkerBuildMenu(), closeCommandCardMenu()
  placement                              // null | { building, valid, tileX, tileY }
  beginPlacement(buildingKind), updatePlacement(tileX,tileY,valid), endPlacement()
  // resource hover preview (client-only):
  resourceMiningPreview                  // null | {resourceId, resourceX, resourceY, ccId, ccX, ccY, inRange}
  updateResourceMiningPreview(preview)
}
```

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

`input/index.js`
```js
export class Input {
  constructor(domElement, camera, state, commandIssuer, renderer, fog, audio?, inputRouter?)
  // installs listeners; translates gestures into selection + protocol commands.
  // number keys recall control groups; double-tap jumps the camera to the largest
  // local cluster. Alt/Ctrl/Cmd+number replaces a group, Shift+number adds to it.
  // On Windows, tabbed browser saves use Alt+number and installed-app saves use Ctrl+number.
  // optional pointer-lock mode traps the browser cursor and drives a visible
  // virtual cursor for edge pan on multi-monitor setups.
  update(dt)                             // continuous handling (edge scroll handled by camera)
  // emits nothing to return; mutates state.selection / state.placement and calls commandIssuer.issueCommand
}
```
Shift-right-click appends queued orders only for selected units: move, attack-move, attack,
gather, build/resume, and placement build commands set `queued: true` and rely on the server
snapshot's owner-only `orderPlan` for accepted markers. Production-building-only right-clicks set
or append building rally stages and rely on owner-only `rallyPlan` for accepted markers. Attack
targeting with only production buildings selected creates `attackMove` rally stages.
Shift-confirmed build placement keeps placement mode armed while Shift is physically held, allowing
multiple queued building placements; releasing Shift or losing window focus clears placement mode.

`command_composer.js` owns command-target arming lifetime for command-card targets. Input and
minimap clicks call `GameState.issueCommandTarget`, so held keys, Shift preservation, and repeated
queued target clicks use one composer path instead of command-specific sticky flags. A plain
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
  constructor(rootEl, state, commandIssuer, audio?, hotkeyProfiles?)
  update()                               // refresh resources/supply, selected panel, command card
  // command card buttons call commandIssuer.issueCommand(...) or state.beginPlacement(...)
}
```
The train command card is driven by the first selected production building type, but train clicks
are issued to the selected completed compatible production buildings in round-robin order so a
multi-building selection spreads queued units across its producers. Train and production-cancel
hotkeys honor native keyboard repeat: after the OS repeat delay, repeated `keydown` events activate
only those repeatable command-card buttons. Research buttons that unlock production appear directly
below the production button they unlock and disappear once complete. Cancel walks selected producing
buildings in reverse round-robin order for the displayed producer type.

`minimap.js`
```js
export class Minimap {
  constructor(canvasEl, state, camera, fog, commandIssuer, inputRouter?)
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
  // renders player list + ready/start/spectator role; calls net.join/ready/start/setSpectator.
  onGameStart(cb)                        // main.js subscribes to transition to game screen
}
```

`main.js` starts `App`; `app.js` owns the persistent `Net` and `Audio`, derives the ws url from
`window.location`, and shows `Lobby`; on `start` it creates `Match`. `match.js` builds
`GameState`, `Camera`, `Renderer`, `Fog`, `HUD`, `MatchInputRouter`, `Minimap`, `Input`, starts the rAF loop
(compute `alpha` from snapshot timing, `camera.update`,
`audio.setListener`, `input.update`, `fog.update`, `renderer.render`, `hud.update`,
`minimap.render`); on each snapshot it applies state and triggers transient event audio exactly
once; on `gameOver` show the victory/defeat overlay with the frozen score table.
For spectator starts, `match.js` hides the command card and give-up action, computes local fog from
the server-filtered union snapshot, and keeps the ordinary renderer/minimap/HUD pointed at snapshots
with `playerResources`.

### 4.1a Targeted ability mode (Smoke, Mortar Fire, Point Fire)

`input/commands.js` exposes `_onAbilityTarget` and `_refreshAbilityTargetPreview` for world-point
abilities. When the HUD command card calls `state.commandTarget = { kind: "ability", ability }`,
the input module enters targeted cursor mode:
- Pointer moves call `_refreshAbilityTargetPreview`: compute which selected units are eligible
  carriers (`ABILITIES[ability].carriers`), test whether any carrier is within range of the cursor,
  update `state.abilityTargetPreview` for the renderer.
- Left-click: build a `useAbility` command with the ability name, filtered carrier ids, world
  coords, and the `queued` flag (from Shift). Clear cursor mode unless the resolved command-card
  hotkey is still held for repeated world-point targeting.
- While the resolved hotkey remains held, repeated left-clicks keep the current selection intact and
  keep targeted mode armed so multi-selected Mortar Teams and Scout Cars can distribute repeated
  point commands without the next click falling back to normal selection.
- Right-click / Escape: cancel cursor mode, `state.commandTarget = null`.
- Minimap right-click also fires an ability command if in targeted mode.
Selected owned Mortar Teams also draw dotted firing-range circles even when the Fire command is not
armed. The Mortar Team Fire command-card button shows an autocast swirl while any selected mortar's
owner-only `mortarFire` affordance has `autocastEnabled`; right-clicking that button sends
`setAutocast(mortarFire, enabled=false)` and does not arm manual targeting.

`state.js` holds `commandTarget` (null or `{ kind, ability }`) and `abilityTargetPreview`
(null or `{ ability, x, y, rangeCenters, inRange }`). `commandTarget` is a transient UI state;
`abilityTargetPreview` is rebuilt every mouse move from the cursor world position and the current
selection.

Range preview rendering (`renderer/feedback.js`, `_drawAbilityTargetPreview`):
- While in targeted ability mode, draws a dotted range ring (radius = `rangeTiles × tileSize`) around
  each eligible carrier.
- At the cursor position, draws the cloud radius preview (2-tile circle) colored green when in
  range of at least one carrier, grey when out of range.

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
  infantry body with oversized role weapons; AT team: wheeled gun; mortar team: crewless
  M1938-inspired small wheeled mortar that travels low and deploys upright; scout car: boxy
  WW2-style truck silhouette with enclosed wheels and a rear-top machine-gunner; tank: chunky
  flat-shaded armor).
  Riflemen carry a rifle, AT teams field a wheeled anti-tank gun with a long recoiling barrel,
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
  small progress arc.
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
by an explicit allowlist reason in the script.

Current areas:
- `app-shell`: `main.js`, `app.js`, `match.js`, match health, replay viewer/control wiring.
- `model`: `state.js`, `command_composer.js`.
- `transport`: `net.js`, `protocol.js`.
- `rules-mirror`: `config.js`.
- `ui`: HUD, command card, lobby, match history, minimap, status badge, branch staging, settings.
- `input`: `input/` plus `replay_camera_input.js`.
- `renderer`: `renderer/`.
- `platform`: bootstrap, audio, combat audio, alerts, fog, camera.

Import rules:
- `protocol.js` and `config.js` are shared mirrors and may be imported where needed.
- Files in the same area may import each other.
- `app-shell` files may compose other areas; prefer adding new cross-area wiring in `match.js` or
  `app.js` instead of importing collaborators from feature modules.
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
