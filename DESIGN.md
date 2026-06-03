# Bewegungskrieg — Design & Architecture

A simple but functional real-time-strategy game inspired by StarCraft: Brood War.
Server-authoritative simulation in **Rust**; client in **HTML/CSS/JS** rendered with
**PixiJS** (WebGL) loaded from a CDN. No sound. Built to be iterated on for years, so
the boundaries below are contracts: keep them stable and well-documented.

This document is the single source of truth for cross-file contracts. If you implement
a module, code against the interfaces defined here. If you must change an interface,
update this file in the same change.

---

## 1. High-level architecture

```
┌────────────────────────┐         WebSocket (JSON)         ┌──────────────────────────┐
│  Browser client (JS)   │  ── ClientMessage ───────────▶   │     Rust server          │
│  PixiJS renderer        │                                  │  axum + tokio            │
│  - lobby UI             │  ◀─ ServerMessage ──────────     │  - static file serving   │
│  - input / selection    │                                  │  - /ws upgrade           │
│  - camera / minimap      │                                  │  - Lobby (rooms)         │
│  - fog overlay (local)   │                                  │  - Game (authoritative)  │
└────────────────────────┘                                   └──────────────────────────┘
```

- The **server** owns the authoritative game state and runs a fixed-rate simulation
  loop (`TICK_HZ`). Clients only send **commands** (intent); they never mutate game
  state directly.
- Every tick the server produces a **per-player snapshot**, applying **fog of war**:
  a player only receives neutral/enemy entities standing on tiles that player can
  currently see. This makes the fog cheat-proof (hidden enemies are never sent).
- The **client** renders snapshots, interpolating entity positions between them for
  smoothness, and computes the **fog overlay** locally from its own units'/buildings'
  sight radii (the server already withholds anything it shouldn't see, so the local
  overlay only needs to look right — it is not a security boundary).
- Local development also exposes a dev-only watch entry at `/dev/selfplay` that auto-runs
  scripted self-play and streams **full-world** snapshots (no fog) to the ordinary match
  renderer. This path is isolated from normal lobby play and is only for debugging.
- The same Rust process serves the static client files, so development is a single
  `cargo run` and then open the printed local URL.

### Tick & networking model
- `TICK_HZ = 30` (~33 ms per simulated tick).
- The server broadcasts a snapshot every `SNAPSHOT_EVERY_N_TICKS` ticks (default 1 →
  30 snapshots/s).
- Commands are queued on arrival and drained at the start of each tick (deterministic
  ordering per connection; ordering across connections is arrival order).
- The client renders at `requestAnimationFrame` (~60fps), interpolating between the two
  most recent snapshots using wall-clock time.

---

## 2. Wire protocol (JSON over WebSocket)

All messages are JSON objects with a `t` field (the discriminator/tag). Field names are
short but readable. Coordinates are **world pixels** (floats) unless a field name ends in
`Tile`. The canonical definitions live in `server/src/protocol.rs` (serde) and
`client/src/protocol.js` (builders + constants). These two files MUST agree.

### 2.1 Client → Server (`ClientMessage`)

| `t`        | Fields | Meaning |
|------------|--------|---------|
| `join`     | `name: string`, `room?: string` | Join (or create) a room. `room` defaults to `"main"`. |
| `ready`    | `ready: bool` | Toggle ready state in the lobby. |
| `start`    | — | Host asks to start the match (only honored from the room host). |
| `addAi`    | — | Host adds a computer opponent to the room (lobby phase only, host-only). |
| `removeAi` | `id: u32` | Host removes a previously-added AI opponent by id (lobby phase only, host-only). |
| `setQuickstart` | `enabled: bool` | Host toggles "Start with more money mode" for the next match in this room. |
| `command`  | `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. |
| `giveUp`   | — | Give up the active match. The server eliminates that player and sends their score screen. |
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |
| `setReplaySpeed` | `speed: f32` | Set replay playback speed multiplier in dev replay rooms; ignored elsewhere. |

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32` | Move selected units to a world point, ignoring enemies until they arrive or receive another order. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32` | Move while attacking enemies encountered; this is the aggressive movement order. |
| `attack`     | `units: u32[]`, `target: u32` | Attack a specific entity. |
| `gather`     | `units: u32[]`, `node: u32` | Send workers to harvest a resource node. |
| `build`      | `worker: u32`, `building: string`, `tileX: u32`, `tileY: u32` | Worker constructs a building at a tile. The server first walks the worker to a nearby point outside the requested footprint, then starts construction once it is in range. `building` ∈ building kinds. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `cancel`     | `building: u32` | Cancel the front of a building's production queue. |
| `stop`       | `units: u32[]` | Clear orders, hold position. |
| `setRally`   | `building: u32`, `x: f32`, `y: f32` | Set a unit-producing building's rally point. Freshly produced units receive a plain `move` order to the point and the building prefers the spawn exit nearest it. Ignored for buildings the player doesn't own, non-producers (depot, training centre), or buildings still under construction. The point is clamped into map bounds. |

Servers MUST ignore commands referencing entities the player does not own, unknown ids,
illegal placements, or unaffordable actions (fail silently or emit a `notice` event).

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool`, `quickstart: bool` |
| `start`    | `Game start payload` (see 2.3). |
| `snapshot` | `Per-player snapshot` (see 2.4). |
| `gameOver` | `winnerId: u32 | null`, `you: "won" | "lost" | "draw"`, `scores: PlayerScore[]` |
| `pong`     | `ts: number` (echo of the ping ts) |
| `error`    | `msg: string` |

`LobbyPlayer`: `{ id: u32, name: string, ready: bool, color: string, isAi: bool }`. `isAi` is
true for computer opponents (always shown ready; the client renders an "AI" tag and a host-only
remove control instead of a ready toggle).

`PlayerScore`: `{ id: u32, name: string, color: string, unitScore: u32, structureScore: u32,
unitsKilled: u32, unitsLost: u32, buildingsKilled: u32, buildingsLost: u32 }`. `scores` is a
frozen server snapshot taken when that recipient gets `gameOver`; it is not live-updated while a
3-4 player match continues. Unit/structure score is the configured steel+oil value of every
unit/building entity created for that player, including starting entities.

### 2.3 `start` payload
Sent once when the match begins. Carries everything static for the whole match.
```
{
  t: "start",
  playerId: u32,                 // your id (repeat of welcome for convenience)
  tick: u32,                     // starting tick (usually 0)
  map: {
    width: u32, height: u32,     // in tiles
    tileSize: u32,               // world px per tile
    // terrain: row-major array length width*height, each a TerrainKind code (u8).
    terrain: number[],
    // All neutral resource nodes (static, never move). Sent so the client can
    // render them on the minimap before fog-of-war reveals them.
    resources: [ { id: u32, kind: "steel"|"oil", x: f32, y: f32 } ]
  },
  players: [ { id, name, color, startTileX, startTileY } ],
}
```
Units/buildings arrive via snapshots (so they obey fog), including
the player's own starting Industrial Center + workers. When the lobby's `setQuickstart` toggle is
enabled, every player starts with 99,999 steel and 99,999 oil instead of the default opening resources.

### 2.4 `snapshot` payload (per-player, fog-filtered)
`Snapshot` remains the semantic shape used by server game code and by client modules after
transport decode:
```
{
  t: "snapshot",
  tick: u32,
  steel: u32, oil: u32,       // your resources
  supplyUsed: u32, supplyCap: u32,
  entities: Entity[],            // your non-resource entities (always) + enemy on visible tiles
  resourceDeltas?: ResourceDelta[], // visible resource remaining updates; omitted when empty
  events: Event[]                // transient things to surface (see 2.5)
}
```

Live WebSocket snapshot frames are sent as compact JSON text, version 1. `client/src/net.js`
decodes this transport shape back into the semantic object above before dispatching `S.SNAPSHOT`.
Older object-shaped JSON snapshots remain decodable by the client for fallback/dev use.

```
{
  "t": "snapshot",
  "v": 1,
  "s": [tick, steel, oil, supplyUsed, supplyCap],
  "e": [
    [
      id, owner, kind, x, y, hp, maxHp, state,
      facing?, weaponFacing?, prodKind?, prodProgress?, prodQueue?,
      buildProgress?, latchedNode?, targetId?, setupState?, remaining?, rally?
    ]
  ],
  "r": [[id, remaining]],         // omitted when empty
  "ev": [EventRecord]             // omitted when empty
}
```

Compact numeric codes:

| Vocabulary | Codes |
|------------|-------|
| `kind` | 1 `worker`, 2 `rifleman`, 3 `machine_gunner`, 4 `at_team`, 5 `tank`, 6 `industrial_center`, 7 `depot`, 8 `barracks`, 9 `training_centre`, 10 `tank_factory`, 11 `steel`, 12 `oil` |
| `state` | 1 `idle`, 2 `move`, 3 `attack`, 4 `gather`, 5 `build`, 6 `train`, 7 `construct`, 8 `dead` |
| `setupState` | 1 `packed`, 2 `setting_up`, 3 `deployed`, 4 `tearing_down` |
| `EventRecord` | `[1, from, to]` attack, `[2, id, x, y, kind]` death, `[3, id, kind]` build, `[4, msg]` notice |

Compact entity records are positional arrays. Optional fields keep the semantic order above and
trailing missing optional fields are omitted; interior missing optional fields are encoded as
`null`. The `rally` slot is itself a two-element `[x, y]` array (or `null`).

`ResourceDelta`: `{ id: u32, remaining: u32 }`. Resource node positions/kinds are static and come
from `start.map.resources`; clients keep last-known `remaining` locally. The server sends
`remaining` updates only for resource nodes currently visible to that recipient (dev full-world
watch rooms receive all resource updates).

`Entity` (lean; omit fields that don't apply):
```
{
  id: u32,
  owner: u32,                    // 0 = neutral (resources), else player id
  kind: string,                  // EntityKind: "worker","rifleman","machine_gunner","at_team","tank","industrial_center","depot","barracks","training_centre","tank_factory"
  x: f32, y: f32,                // world px (center)
  hp: u32, maxHp: u32,
  state: string,                 // "idle","move","attack","gather","build","train","construct","dead"
  facing?: f32,                  // radians, for unit body/hull orientation (optional)
  weaponFacing?: f32,            // radians, for independent weapon/barrel orientation (optional)
  // production buildings:
  prodKind?: string,             // unit currently being produced
  prodProgress?: f32,            // 0..1
  prodQueue?: u32,               // queued count (including the in-progress one)
  // buildings under construction:
  buildProgress?: f32,           // 0..1; when present and <1, render as scaffolding
  // workers:
  latchedNode?: u32,             // node id the worker is currently harvesting (attached mining)
  // combat feedback:
  targetId?: u32,                // current attack target, for drawing tracers
  setupState?: string,           // machine_gunner only: "packed","setting_up","deployed","tearing_down"
  // unit-producing buildings:
  rally?: [f32, f32]             // rally point (world px); ONLY ever sent to the owner
}
```

### 2.5 `Event` (transient, one snapshot only)
```
{ e: "attack", from: u32, to: u32 }            // for muzzle flashes / tracers
{ e: "death",  id: u32, x: f32, y: f32, kind } // for death poofs
{ e: "build",  id: u32, kind: string }         // building completed
{ e: "notice", msg: string }                   // "Not enough steel", etc.
```
Events are best-effort visual flavor; the client must not depend on receiving them.

---

## 3. Rust server — modules & the Game core API

Crate layout (`server/`):
```
Cargo.toml
src/
  main.rs        # tokio runtime, axum router: static files + /ws, room manager task
  protocol.rs    # serde types for §2  (PINNED — provided)
  config.rs      # all balance/sim constants (PINNED — provided)
  lobby.rs       # Room, Lobby: join/ready/start, per-connection actor plumbing
  rules/
    mod.rs       # rules module boundary
    defs.rs      # immutable unit/building/node definition tables
    combat.rs    # weapon/armor predicates and damage formula
    economy.rs   # tech/production predicates and cost/supply wrappers
    terrain.rs   # terrain movement/cover/concealment seam (Open only today)
    projection.rs # fog-gated entity/event projection seam
  game/
    mod.rs       # Game struct + public API (the seam below)
    command.rs   # SimCommand domain commands + protocol translation helpers
    map.rs       # Map: handcrafted terrain asset loading, passability, base-site validation
    entity.rs    # Entity, EntityKind, EntityStore (slotmap-style Vec + free list)
    pathfinding.rs # A* over the tile grid (impassable = terrain + building footprints)
    fog.rs       # per-player visibility grid (visible / explored)
    systems.rs   # orchestrator: runs services in order each tick
    services/    # per-tick internal services: commands, move_coordinator, movement (incl. unit collision), combat, economy, production, construction, death, occupancy, supply, pathing, geometry, standability
    ai.rs        # optional computer opponents: one AiController per AI player (see §8)
    ai_core/     # shared AI observation/facts/action/profile core, introduced incrementally
    ai_shared.rs # compatibility helpers while live AI and self-play migrate to ai_core
    replay.rs    # tick-stamped command log replay harness for determinism checks
    selfplay.rs  # test-only API-driven scripted self-play harness (see §9)
```

### 3.1 `game::Game` public API (seam between `game` and `lobby`/`main`)
The `lobby`/networking layer interacts with the simulation ONLY through this surface.
`game-core` implementer: provide exactly these. `server-shell` implementer: call only these.

```rust
pub struct Game { /* private */ }

impl Game {
    /// Create a match for the given players (ids + colors + names already assigned by lobby).
    /// Loads the hardcoded handcrafted map, shuffles the authored (start, expansion) pairs by
    /// `seed`, assigns the first N shuffled starts to the N players in lobby order, and spawns
    /// each player's starting Industrial Center + STARTING_WORKERS workers + nearby steel/oil
    /// resource clusters. For one-, three-, and four-player games, each start keeps its authored
    /// paired expansion. For two-player games, the selected starts are kept but the two active
    /// neutral expansions are reselected from the authored expansion pool as the most symmetric
    /// assignment for that start matchup, so adjacent starts both expand in comparable directions.
    pub fn new(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a match with explicit starting steel/oil for every player.
    pub fn new_with_starting_resources(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply domain command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand);

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Build a full-world snapshot for a dev watch client. Normal gameplay must not use this.
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot;

    /// Player ids still alive. Humans need at least one building; AI players also need a unit.
    pub fn alive_players(&self) -> Vec<u32>;

    /// Frozen score-screen rows for every match participant, in start/lobby order.
    pub fn scores(&self) -> Vec<PlayerScore>;

    /// Remove all of a player's entities (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32);

    pub fn tick_count(&self) -> u32;

    /// Authoritative commands applied so far, stamped with the tick that applied them.
    pub fn command_log(&self) -> &[CommandLogEntry];

    /// Reconstruct the player specs used to create this match for replay/crash artifacts.
    pub fn player_inits(&self) -> Vec<PlayerInit>;
}

pub struct PlayerInit { pub id: u32, pub name: String, pub color: String, pub is_ai: bool }
pub struct CommandLogEntry { pub tick: u32, pub player_id: u32, pub command: Command }
```
`SimCommand` is the internal command enum from `game::command`; `ClientMessage::Command` and
replay artifacts are translated into it at the boundary. `CommandLogEntry.command` remains the
serde `Command` from `protocol.rs` so replay JSON stays wire-compatible. `StartPayload`,
`Snapshot`, `Event`, and `PlayerScore` are also serde types from `protocol.rs`.

`PlayerInit.is_ai` marks a computer-controlled player. AI players are full players in every
respect (they get a start position, Industrial Center, workers, economy, and count toward win/elimination); the
only difference is they have no socket. `Game` owns one `AiController` per AI player and drives
them at the top of `tick()` — see §8.

### 3.2 Concurrency model
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`).
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` to push to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `Command`, `GiveUp`). The room task is the
  single writer of game state — no locks around `Game`.
- The room task, each tick: drain commands → `game.tick()` → for each connected player
  `game.snapshot_for(pid)` → send. Lobby phase: broadcast `lobby` on changes.
- Dev self-play watch rooms are a special-case room mode inside the same task model: they own a
  normal `Game`, feed it scripted commands from `game::selfplay`, and send watchers
  `game.snapshot_full_for(view_pid)` instead of fog-filtered snapshots. Replay rooms advance at
  1.5x the normal room tick rate so artifact playback finishes faster than live self-play.

### 3.3 Rules layer (`rules/`)

`server/src/rules/` contains classification, formula, terrain, and projection functions. Rules
never mutate state. Most take `EntityKind` and context primitives; `rules::projection` is the
explicit exception that reads `Entity` plus `Fog` so snapshot and event visibility policy is
centralized instead of scattered through services.

- `rules::defs` — immutable unit/building/node definition tables keyed by `EntityKind`. These
  records are the source of truth for kind-specific stats, armor class, weapon class, target
  priority, production chains, tech requirements, and resource-node amounts.
- `rules::combat` — AP/armor predicates (`is_ap`, `is_armored`, `prefers_armored_targets`),
  `attack_profile(kind) -> AttackProfile`, and
  `effective_damage(attacker_kind, victim_kind, base_dmg, victim_terrain) -> u32`.
- `rules::economy` — tech/production predicates (`trainable_units`, `build_requirement_met`,
  `train_requirement_met`), resource-node amounts, and cost/supply wrappers (`cost`,
  `supply_cost`, `supply_provided`).
- `rules::terrain` — `TerrainKind` plus movement, cover, and concealment modifiers. It is
  intentionally near-empty today (`Open` returns current defaults) so the forest/road/hill feature
  has one rules file to grow in.
- `rules::projection` — fog-gated `EntityView` construction and event visibility predicates.
  It is intentionally a seam for future last-known-position memory or partial unit-type reveal;
  it does not add wire fields today.

Services in `game/services/` orchestrate tick logic and call into `rules::*` for classification.
Rules functions have no imports from `services/`; classification and formula rules read
kind-specific data from `rules::defs`. `config.rs` holds scalar constants and compatibility
wrappers such as `unit_stats(kind)` / `building_stats(kind)`, which return the stats embedded in
defs.

`game::systems::run_tick` owns the tick pipeline and the lifecycle of tick-scoped derived state.
It rebuilds named phase state at explicit boundaries: pre-command state for command validation,
pathing, and movement; post-movement state for combat and economy queries; pre-collision state
after production/construction/death mutations; and final state for snapshot interest filtering.
Systems should consume the derived-state object for their phase instead of carrying occupancy or
spatial indexes across later mutations.

`services::geometry` owns shared body primitives: unit bodies are circles centered on `(x, y)`
with the configured unit radius, building bodies are axis-aligned rectangles derived from
footprint tiles, and resource node bodies are circles for build-site blocking. `services::standability`
owns reusable legality predicates for unit bodies and building sites. Production spawn exits,
construction/build intent, movement landing, steering candidates, collision push targets, and
formation goal selection all use this shared standability layer for static/body legality. These
helpers are pure and do not change the wire protocol or client contract.

---

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
  state.js        # GameState: holds prev+current snapshot, selection, camera, placement
  camera.js       # Camera: pan/zoom, world<->screen transforms, edge/keyboard scroll
  renderer.js     # Renderer: PixiJS app + layers; render(state, camera, alpha)
  fog.js          # Fog overlay: accumulate explored, compute visible from own entities
  input.js        # Input: mouse/keyboard -> selection box, issue commands, build placement
  hud.js          # HUD: resources/supply bar, selected panel, command card (build/train)
  minimap.js      # Minimap: draw terrain+entities+viewport; click to move camera/command
  lobby.js        # Lobby screen: name entry, player list, ready/start buttons
  main.js         # Bootstrap & wiring: screens (lobby<->game), net hookup, render loop
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
  command(cmd)                           // cmd built via protocol.js builders
  ping()
  setReplaySpeed(speed)                  // dev replay rooms only
  get playerId()
}
```

`state.js`
```js
export class GameState {
  playerId
  startInfo                              // §2.3 payload
  map                                    // {width,height,tileSize,terrain}
  players                                // [{id,name,color,startTileX,startTileY}]
  // snapshot buffering for interpolation:
  applySnapshot(msg)                     // pushes msg, keeps prev+current, stamps recvTime
  entitiesInterpolated(alpha)            // -> array of entities with lerped x,y
  get prevRecvTime() / get currRecvTime()// recv timestamps of the two buffered snapshots
                                         //   (null until two exist); main.js derives interp alpha
  resources                             // {steel,oil,supplyUsed,supplyCap} (latest)
  events                                 // latest snapshot's events
  // selection (client-only):
  selection                              // Set<entityId>
  setSelection(ids), addToSelection(ids), clearSelection()
  selectedEntities()                     // resolved entity objects from current snapshot
  entityById(id)
  // build placement (client-only):
  placement                              // null | { building, valid, tileX, tileY }
  beginPlacement(buildingKind), updatePlacement(tileX,tileY,valid), endPlacement()
  // resource hover preview (client-only):
  resourceMiningPreview                  // null | {resourceId, resourceX, resourceY, icId, icX, icY, inRange}
  updateResourceMiningPreview(preview)
}
```

`camera.js`
```js
export class Camera {
  x, y, zoom                             // world coords of viewport top-left, zoom factor
  update(dt, input)                      // apply pan (keys/edge) & clamp to map bounds
  worldToScreen(wx, wy) -> {x,y}
  screenToWorld(sx, sy) -> {x,y}
  centerOn(wx, wy)
  setBounds(worldW, worldH, viewW, viewH)
}
```

`renderer.js`
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
  update(ownEntities, tileSize)          // mark visible tiles this frame; accumulate explored
  isVisible(tileX,tileY), isExplored(tileX,tileY)
  // renderer reads the grids to draw the black/dim overlay
  visibleGrid, exploredGrid              // Uint8Array length w*h
}
```

`input.js`
```js
export class Input {
  constructor(domElement, camera, state, net, renderer, fog)
  // installs listeners; translates gestures into selection + protocol commands.
  update(dt)                             // continuous handling (edge scroll handled by camera)
  // emits nothing to return; mutates state.selection / state.placement and calls net.command
}
```

`hud.js`
```js
export class HUD {
  constructor(rootEl, state, net)
  update()                               // refresh resources/supply, selected panel, command card
  // command card buttons call net.command(...) or state.beginPlacement(...)
}
```

`minimap.js`
```js
export class Minimap {
  constructor(canvasEl, state, camera, fog, net)
  render()                               // draw terrain + fog + entity blips + viewport rect
  // click/drag -> camera.centerOn or issue move command (right-click)
}
```

`lobby.js`
```js
export class Lobby {
  constructor(rootEl, net)
  show(), hide()
  // renders player list + ready/start; calls net.join/ready/start.
  onGameStart(cb)                        // main.js subscribes to transition to game screen
}
```

`main.js` wires it all: create `Net`, derive ws url from `window.location`, show `Lobby`;
on `start` message build `GameState`, `Camera`, `Renderer`, `Fog`, `HUD`, `Minimap`, `Input`,
start the rAF loop (compute `alpha` from snapshot timing, `camera.update`, `input.update`,
`fog.update`, `renderer.render`, `hud.update`, `minimap.render`); on `gameOver` show the
victory/defeat overlay with the frozen score table.

### 4.2 Rendering & look (PixiJS, procedural art — neutral PS1 field-command style)
- Layers (back→front): terrain → resource nodes → building shadows → buildings → unit
  shadows → units → selection rings → health bars → fog overlay → command/hover feedback → placement ghost →
  selection drag-box → (HUD is DOM, not Pixi).
- Units: low-detail hard-edged silhouettes tinted by player color, with a dark drop shadow,
  dark outline, small facing indicator, HP bar above when damaged/selected, and glowing
  selection ring when selected. Distinct silhouette per kind (engineer: compact block;
  rifleman: infantry wedge; machine gunner / AT team: support silhouettes; tank: chunky
  flat-shaded armor).
- Buildings: footprint-sized blocky field structures with neutral geometry and plain
  two-letter stencils; under construction → translucent with a progress bar; production →
  small progress arc.
- Resource nodes: steel = tan supply crates; oil = olive fuel drums; show last-known remaining
  from `resourceDeltas` via size/opacity. When a worker is selected and the cursor hovers a
  resource, draw a blue circle on the resource when the nearest completed own Industrial Center
  is inside mining range; draw a red/dashed line to the Industrial Center when too far.
- Terrain: muted grass/field/mud, rock, and water tiles with deterministic coarse dithering
  so movement is readable and the map has a PlayStation 1-era low-resolution texture feel.
- Fog: unexplored = 86% dark overlay so terrain remains faintly readable; explored-but-not-visible =
  55% dark overlay; visible = clear. Use a single overlay sprite/graphics updated from `fog`
  grids; soften edges if cheap.
- Selection: green for own, red tint for enemy, yellow for neutral. Drag-box translucent green.
- Keep a cohesive muted palette; define colors in `config.js`.
- Art must stay faction-agnostic: no Soviet, German, Nazi, imperial, national, or unit-branch
  iconography. Avoid flags, stars, crosses, eagles, skulls, sickles, hammers, and historically
  specific insignia.

---

## 5. Balance definitions & constants
Kind-specific server balance lives in `server/src/rules/defs.rs`; terrain movement/cover/
concealment hooks live in `server/src/rules/terrain.rs` and currently return the all-open-ground
defaults. `config.rs` is the thin constants module for timings, tile size, starting resources,
supply caps, mining amounts, and other scalar simulation constants; its `unit_stats(kind)` and
`building_stats(kind)` helpers read the defs table.
`client/src/config.js` mirrors the subset the UI/render/fog needs (costs, supply, sight, sizes,
colors). Keep both in sync; the comment in each file points at the other.

### 5.1 Target theme and MVP combat loop

The target gameplay direction is a simplified World War II-inspired battlefield with
fictional, faction-agnostic sides. This is not a historical simulation. The theme should
support readable gameplay, clear unit roles, and strong terrain identity without national
or regime-specific iconography.

MVP scope:
- No air forces.
- No artillery or mortars yet.
- No mines, morale, logistics, suppression-depth model, or detailed tank armor model yet. Tanks
  do have a simple hull-facing armor rule for anti-tank damage.

Core unit roles:
- **Rifleman** is the baseline combat unit: cheap, flexible, useful for capturing and
  holding ground, and the primary answer to enemy infantry in forests.
- **Machine gun** is the defensive escalation unit: it takes one second
  (`MACHINE_GUNNER_SETUP_TICKS`) to set up after stopping, then fires at a very high rate.
  Once deployed it must spend the same one-second interval tearing down before it can move.
  Machine-gun nests
  are the main base-defense tool and should dominate open-ground infantry combat in the
  second stage of the game.
- **Tank** is the machine-gun breaker and open-ground power unit: immune to rifle and
  machine-gun small-arms fire, strong against static defenses and exposed infantry, but
  vulnerable to other tanks and anti-tank infantry.
- **Anti-tank infantry team** is the ambush counter to tanks: dangerous from the side,
  rear, or at close range, especially when operating from forests, but weak or inefficient
  against regular infantry.

Terrain rules:
- **Open ground** favors machine guns and tanks.
- **Forests** are passable by infantry and impassable to tanks.
- Infantry in forests gets defensive and concealment bonuses.
- Forests are intentionally "infantry country": the main way to clear infantry from a
  forest is to send in your own infantry.
- Tanks and machine guns can contain forests by covering exits, clearings, and forest
  edges, but they should not reliably clear forest infantry from outside.

Intended progression:
- Early game: riflemen fight for map control.
- Midgame: machine guns lock down open lanes and bases.
- Armor phase: tanks break machine-gun-heavy defensive lines in open terrain.
- Counter-armor phase: anti-tank infantry, forest ambushes, and other tanks punish
  unsupported tanks.
- Forest fights remain infantry-led so tanks and machine guns never become universal
  answers.

### 5.2 Current implementation constants

The current implementation uses the themed unit/building names below. Combat is handled by the
shared attack model plus the machine-gunner setup/teardown state, tank turret aim gates, and
tank hull-facing damage modifiers for anti-tank hits against tank victims. Forest-specific rules
are future work. The unit, building, and resource-node tables below are the human-readable form of
the authoritative `rules::defs` records.

- `TICK_HZ = 30`, `SNAPSHOT_EVERY_N_TICKS = 1`.
- `MACHINE_GUNNER_SETUP_TICKS = 30` (~1s setup or teardown).
- Map: `TILE_SIZE = 32` px. The live map is the hardcoded handcrafted asset at
  `server/assets/maps/default-handcrafted.json` (96×96 today), served for tooling at
  `/maps/default-handcrafted.json`.
  Its JSON uses row strings (`.` grass, `#` rock, `~` water) plus ordered `baseSites`.
- Start: `STARTING_STEEL = 50`, `STARTING_OIL = 0`, `STARTING_WORKERS = 4`,
  one Industrial Center at the player's start tile, 18 steel patches + 3 oil patches nearby.
- Supply: Industrial Center gives `+10`, Depot gives `+8`, hard cap `200`.
- Attached mining: workers walk to a patch, latch onto it, and mine in place.
  Every `HARVEST_TICKS = 40` the load (`STEEL_LOAD = 2` / `OIL_LOAD = 2`) is deposited
  directly into the player's economy only if the resource node is within
  `MINING_IC_RANGE_TILES = 7.0` tiles of a completed Industrial Center owned by that player.
  The range matches `IC_RESOURCE_MAX_DIST_TILES`, so each starting Industrial Center can mine
  every patch in its main-base cluster. If no completed IC is close enough, workers ignore new
  gather orders for that patch and active miners go idle. When a patch empties the worker goes
  idle (no automatic retarget).
- One worker per patch: each node has a single harvest slot (`Entity::miner`). A patch is
  occupied only after the worker reaches `GatherPhase::Harvesting`; right-clicking a patch
  does not reserve it. Extra workers that arrive while the slot is taken go idle. The slot
  is advisory and self-heals — it's only honored while the recorded worker is alive and
  actively harvesting that node, so death / re-order / retarget free it automatically.
- Starting layout: each base site gets 18 steel patches and 3 oil patches. `baseSites` are stored
  as interleaved pairs: `[start0, expansion0, start1, expansion1, ...]`. The pairs are shuffled
  by the match seed, and the first N shuffled starts become the active player starts. For one-,
  three-, and four-player games, each selected start keeps its authored paired neutral expansion.
  For two-player games, the two neutral expansion sites are selected from the authored expansion
  pool by scoring each assignment in the players' local start-to-enemy frames; this favors matching
  forward/lateral offsets and natural distance, avoiding one player receiving a shared middle
  natural while the other receives a side natural. Sites not selected as an active start or active
  expansion are unused, giving exactly 2N active bases on the map. Shuffling stops the lobby seat
  order from pinning the human/AI to the same corner every match.

Unit stats (hp, dmg, range[tiles], cooldown[ticks], speed[px/tick], sight[tiles], cost, supply, buildTicks):

| kind            | hp  | dmg | range | cd | speed | sight | steel | oil | sup | buildTicks |
|-----------------|-----|-----|-------|----|-------|-------|-----|-----|-----|-----------|
| worker          | 40  | 4   | 1     | 12 | 1.6   | 7     | 50  | 0   | 1   | 240 (~8s) |
| rifleman        | 45  | 5   | 4     | 8  | 1.6   | 8     | 50  | 0   | 1   | 300 (~10s) |
| machine_gunner  | 55  | 4   | 5     | 3  | 1.28  | 8     | 75  | 25  | 2   | 400 (~13s) |
| at_team         | 45  | 48  | 5     | 48 | 1.28  | 8     | 75  | 25  | 2   | 440 (~15s) |
| tank            | 390 | 60  | 3     | 36 | 2.0   | 7     | 200 | 100 | 6   | 500 (~17s) |

Building stats (hp, sight, cost, footprint tiles wxh, buildTicks, extra):

| kind                       | hp  | sight | cost | foot | buildTicks | notes |
|----------------------------|-----|-------|-----|------|-----------|-------|
| industrial_center          | 600 | 9     | 200 | 3x3  | 400       | trains worker; +10 supply; players start with one free |
| depot                      | 220 | 4     | 100 | 2x2  | 120       | +8 supply |
| barracks                   | 320 | 6     | 150 | 3x2  | 200       | trains rifleman, machine_gunner, at_team; requires an Industrial Center |
| training_centre   | 300 | 6     | 100 steel + 50 oil | 3x2  | 220       | unlocks machine_gunner and at_team training at barracks; requires an Industrial Center |
| tank_factory               | 360 | 6     | 200 steel + 100 oil | 3x3  | 240       | trains tank; requires an Industrial Center and Training Centre |

Win: a player is **eliminated** when they own zero buildings (units alone do not keep them
alive). Last player standing wins; a 1-player match never ends (sandbox/exploration mode). In a
3-4 player match, a connected human who is eliminated receives a one-time `gameOver` score
snapshot immediately while the remaining players keep playing; final match resolution sends
`gameOver` only to players who have not already received one.

---

## 6. Conventions
- Rust: edition 2021, `cargo fmt`, `#![deny(warnings)]` off (warnings ok), no `unwrap()` on
  network/parse paths — handle errors and keep the room alive. Prefer small pure functions in
  `services/`. Avoid panics in the tick loop.
- JS: ES2020 modules, no framework, small classes per §4, JSDoc on public methods, no global
  state except `PIXI`. Pure helpers where possible.
- Both: names match this doc. Document any deviation here in the same change.
- Coordinates: world pixels everywhere on the wire; tiles only where a field ends in `Tile`.

---

## 7. Hardening (input is untrusted)
The server treats every client as potentially hostile. Limits live next to the code:
- **WebSocket frame cap** (`main.rs`): `max_message_size`/`max_frame_size` = 256 KiB. Oversized
  frames are rejected and the connection closed before they reach serde.
- **Command unit cap** (`services/commands.rs` `MAX_UNITS_PER_COMMAND = 256`): unit-list commands are
  deduped and capped before per-unit work, so a repeated/huge id list can't trigger an A* storm.
- **Bounds-checked placement** (`services/occupancy.rs` `footprint_tiles`): tile math uses `checked_add` and
  out-of-range build coords are rejected — the tick loop never panics on adversarial input.
- **Body-aware construction placement**: `services::standability::building_site_clear` is the
  final scaffold policy. A building footprint rectangle must be in-bounds, passable, clear of
  existing building rectangles/resource bodies, and clear of every living unit circle. Build
  command intent uses the paired build-intent predicate, which ignores only the chosen builder's
  own body so a worker can be ordered to build over its current position and walk out first.
  `construction_system` repeats the build-intent unit-body policy at arrival before creating the
  scaffold, so every other living unit still blocks the site but the chosen builder can start the
  scaffold and become a ghost active builder.
  The client placement ghost mirrors the intent policy for the first selected worker, but remains
  advisory; the server is authoritative.
- **Idle timeout + heartbeat**: the server drops connections idle for `IDLE_TIMEOUT = 40s`
  (`main.rs`); the client pings every 15s (`main.js`). This evicts half-open/stuck clients so a
  silent player can't wedge a shared room, and frees the room slot.
- **Join ack**: `RoomEvent::Join` carries a `oneshot<bool>`; a connection only marks itself joined
  on an accept, so a rejected mid-match join doesn't wedge the socket.
- **Fog is authoritative**: `snapshot_for` and per-recipient event delivery go through
  `rules::projection`, which gates entity views, `target_id` tracers, and death/attack events on
  visibility — hidden enemies are never sent.
- **Shot overpenetration**: ranged attacks continue 25% of their weapon range past the primary
  target and deal 50% reduced damage to additional enemies behind it, which discourages
  clumping and rewards tighter army control. Two exceptions: a shot whose primary target is a
  **tank** never overpenetrates (the armour stops the round dead, no exceptions — even AT teams),
  and **AT teams** punch deeper, carrying 50% of their weapon range past the primary target.
- **Tank body and weapon facing**: the snapshot `facing` field is the tank hull/body angle. Tanks
  rotate that body angle at a bounded rate on movement paths; badly misaligned tanks pivot in
  place instead of sliding sideways at full speed. The snapshot `weaponFacing` field is the
  independent turret/barrel angle. Tank combat rotates the turret toward the target at a bounded
  rate and fires only once the turret is within tolerance; the hull does not need to face the
  target. Projection omits enemy `weaponFacing` when it would reveal a hidden target direction.
- **Tank armor facing**: tank and AT-team attacks against tank victims use the victim tank's hull
  `facing` and the attacker's position. Front hits (`<=45°` from the hull direction) deal normal
  damage, side hits (`>45°` and `<=135°`) deal `1.25x`, and rear hits (`>135°`) deal `1.75x`.
  Infantry damage, building damage, non-tank victims, and non-anti-tank attackers ignore armor
  facing. Overpenetration victims use the same facing rule.
- **Worker direct-hit retreat**: a worker that takes primary-target damage from an attacker gets a
  short move-away order through normal pathing. Overpenetration splash does not trigger this
  reaction, and workers actively constructing a scaffold stay latched so unfinished buildings are
  not stranded.
- **Tolerant arrival**: a unit on a `Move` or `AttackMove` order in `MovePhase::Moving` that has not
  moved more than `STUCK_EPS_PX` per tick for `STUCK_ARRIVAL_TICKS` consecutive ticks (~0.5 s at
  30 Hz) and is within `TOLERANT_ARRIVAL_RADIUS_PX` (2 tiles) of its `path_goal` is immediately
  marked `Arrived` and halted. This dissolves the stuck-blob pattern where multiple units ordered
  to the same tile fight each other for the last position. The two per-unit state fields
  (`stuck_ticks: u16`, `last_progress_pos: (f32, f32)`) live in `MovementState` and are reset
  whenever a fresh order is issued.
- **Static-obstacle repath**: if a unit on a `Move` or `AttackMove` order repeatedly fails to take
  its next path step because terrain/building occupancy blocks the landing tile, movement debounces
  the failure for `STATIC_BLOCKED_REPATH_TICKS` (~1 s at 30 Hz), clears the stale path, and marks the
  unit `AwaitingPath`. The existing path coordinator then recomputes under current occupancy within
  the normal per-tick A* budget. This covers buildings constructed after a long path was assigned
  without periodically repathing every moving unit.
- **Formation goal legality**: group move goals keep the existing distance-sensitive formation
  behavior, but candidate tiles are accepted only when the specific unit kind can stand there under
  `standability::unit_static_standable`. This prevents large units from being assigned a center tile
  whose body would clip terrain or a building footprint; dynamic unit traffic is still handled by
  steering and collision after movement.
- **Local steering**: before taking a partial path step for a plain `Move` order, movement computes a
  short-range separation proposal away from nearby firm/braced/heavy mobile units. Neighbor ids are
  sorted and capped so replay behavior stays deterministic, and separation uses the same footing
  profiles as hard collision so braced/heavy units exert stronger local pressure than firm units.
  The steered landing is only accepted if `standability::unit_static_standable` says the unit body
  fits there; otherwise movement falls back to the ordinary path step / wall-slide logic. Steering
  does not reserve space or replace collision.
- **Production spawn legality**: production completes in two steps. The front queue item advances
  to complete, then the producer searches deterministic rings around its actual footprint for a
  `standability::unit_spawn_standable` point. Spawn candidates must fit the unit body inside world
  bounds without clipping terrain, any building footprint, or any living unit body, including ghost
  workers. If every candidate is blocked, the complete queue item stays in place and retries on
  later ticks; cost and supply remain reserved from enqueue time. When the producer has a rally
  point set, the search picks the closest standable candidate to the rally within the first ring
  that has any (so units exit the rally-facing side), and the new unit is immediately given a plain
  `move` order to the rally point; with no rally point the legacy first-found candidate is used and
  the unit spawns idle.
- **Unit collision**: `services::movement::resolve_collisions` runs after production each tick
  and pair-wise pushes overlapping mobile units apart along the connecting line. Workers in
  `GatherPhase::Harvesting` or `BuildPhase::Constructing` are ghost pass-through units: they
  neither push nor are pushed, which keeps walking units from being deadlocked by miners or active
  builders. All other mobile-unit pairs split overlap by footing resistance, so braced/deployed
  machine gunners and tanks hold ground better than soft moving infantry while equal-profile units
  still split pushes evenly. `Game::assert_invariants` then asserts that no two non-ghost mobile
  units overlap by more than `OVERLAP_TOLERANCE_PX` (residue from pushes that landed against
  impassable terrain or building body clearance). Collision is deterministic overlap cleanup for
  dynamic unit traffic; static correctness comes from standability checks before positions or
  scaffolds are accepted.

---

## 8. AI opponents (optional, `game/ai.rs`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). The lobby also has a host-only
`setQuickstart` toggle labeled "Start with more money mode", which causes the next match to begin
with 99,999 steel and 99,999 oil for every player. They are capped with humans at
`MAX_PLAYERS = 4` (the hardcoded map has enough ordered `baseSites` for four starts plus neutral
expansions). AI players are seated after the humans in the lobby player list; their colors come
from the tail of `PLAYER_PALETTE` so they never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.

**Where it runs.** `Game` holds one `AiController` per AI player and drives them at the top of
`tick()`, *before* commands are applied. Each controller pushes ordinary `SimCommand`s onto the same
pending queue as translated human client input, so every AI action goes through the identical
validation / cost / supply / placement path in `services/commands.rs` — the AI has **no special authority**
over the simulation and can't cheat economy or placement rules. Because the controller is
server-side (not a network client) it reads authoritative own/resource state directly, but enemy
entities are filtered through that player's authoritative fog grid. To stay fair, outbound attacks
target enemy **start tiles**, which are public via the `start` payload; direct attacks only target
currently visible enemy units/buildings during local defense.

**Strategy (deliberately "very basic").** Each controller, on a staggered cadence
(`DECISION_INTERVAL` ticks), builds a constrained live `AiObservation` and delegates RTS decisions
to `game::ai_core::decision::decide_profile`. The default live profile is
`rifle_flood_full_saturation`, selected server-side without a lobby protocol or UI change. It keeps
idle workers mining steel, trains workers toward starting steel saturation, builds depots before
supply deadlock, builds barracks, pumps riflemen, stages combat units forward, and attack-moves
escalating rifleman waves at the nearest living enemy's public start tile. It does not micro,
scout, or choose hidden enemy unit positions. A local per-think budget in the shared action layer
prevents it from over-committing resources/supply it does not have.

**Shared AI core.** `game::ai_core` has deterministic profile data (`profiles.rs`) and a generic
ranked decision loop (`decision.rs`) that emits ordinary `SimCommand`s through shared action helpers.
The first code-defined profiles are `rifle_flood_fast`, `rifle_flood_full_saturation`,
`tech_to_tanks`, and `steel_expansion_tanks`; they parameterize worker targets, supply buffers,
building/tech goals, production priorities, resource timing, expansion timing, and attack
thresholds without providing their own `think()` functions.
`rifle_flood_fast` sends exactly one reserved worker toward a hidden edge-biased proxy point near
the nearest public enemy start tile immediately, before it can afford the barracks. The transit
target stays at least 18 tiles from the enemy start, prefers map-edge footprints, and avoids the
direct own-base-to-enemy-base scouting line. If the worker was already committed when the barracks
becomes affordable, the AI places the barracks near that worker's current position rather than
waiting for the ideal edge point; if it can afford the barracks immediately, it uses the hidden
edge target as the build site. It trains only one extra home worker and attack-moves riflemen as
individual pressure units instead of waiting for escalating waves.
`tech_to_tanks` is a steel-first fast-tech profile: it keeps worker production active while saving
for the tank-factory step, delays oil workers until at least eight workers are already mining steel,
uses ready combat units to clear visible threats in its home resource line before attacking out,
and treats a single completed tank as a valid minimum attack wave.
All profiles share a defensive panic mode. A visible enemy near the AI's base, home resource line,
or workers temporarily suspends expansion, worker training, and non-defensive tech spending. While
panicking, the AI classifies the visible local threat by weapon DPS: tank-dominated pressure (75%+
of visible local DPS) prioritizes AT teams, infantry-dominated pressure prioritizes Machine
Gunners, mixed pressure asks for a support mix, and no-DPS pressure falls back to Riflemen. Support
panic only uses an already-completed Training Centre and may pull workers onto oil for those support
counters; if support tech is absent, Barracks production falls back to Riflemen and panic mode does
not create the Training Centre. If the pressure persists through the panic window, the AI asks for
an additional Barracks before resuming its normal profile once the threat has cleared.
`steel_expansion_tanks` is a defensive economic support profile: it saves for a second Industrial
Center near a neutral steel expansion before building any non-Depot tech structure. Valid
expansion sites must cover the full local resource line, then are ranked by own distance divided
by nearest living enemy-start distance so similarly close naturals prefer the base farther from
enemies. Once that expansion IC is planned, it builds Barracks and Training Centre tech, staffs
oil, produces Machine Gunners and AT teams toward a one-for-one support mix, and keeps those
support units staged in a short line on the enemy-facing side of its main-base steel cluster
instead of launching outbound attack waves.
After 100 supply used, it switches to a Tank Factory tech path, stops Machine Gunner / AT team
production, trains tanks, and launches outbound tank groups only once at least three tanks are
ready. After the expansion IC is complete, its worker resource assignment is locally bounded so
main-base workers do not walk to expansion patches, and expansion workers do not walk back to
main-base patches.
The live lobby AI uses this shared core through `AiController`, which only owns live identity,
profile id, cadence, and persistent decision memory. Profiles are still not client-selectable.

**Win/elimination.** AI players count as match players: a 1-human + N-AI match is a real match
(it resolves to a winner), while a lone human with no AI remains a never-ending sandbox. They have
one special elimination rule: an AI with no units left is defeated even if it still owns buildings,
because it has no player input path back into the game. The lobby's `match_player_count` is humans
**+** AIs.

---

## 9. API-driven self-play test harness

The automated self-play harness is a **test-only** layer in `game/selfplay.rs`. It is intentionally
separate from the gameplay AI in `game/ai.rs`: gameplay AI is a player feature, while self-play is
a regression harness for exercising the public simulation API.

**Contract.** Self-play scripts may only drive the game through the `Game` seam in §3.1:
`start_payload()`, `snapshot_for(player)`, `enqueue(player, SimCommand)`, `tick()`,
`alive_players()`, and `tick_count()`. Scripts observe the same fog-filtered snapshots a client
would receive and issue ordinary domain commands. They must not mutate entities, players, map state, or
private system internals. This keeps the simulation architected for future API clients, replay
tools, and external test drivers without adding a second privileged control path.

**Command log replay.** `Game` records every command at the authoritative apply tick, after AI
controllers have emitted their normal commands and before systems apply the pending queue.
`game/replay.rs` translates that wire-compatible log into `SimCommand`s, feeds them into a fresh
`Game` with AI thinking disabled, and compares the resulting event stream and final per-player
snapshots. Replay and live play use the same typed command application path, so a replay proves both
the recorded command artifact and the deterministic simulation ordering. Entity iteration and A*
tie-breaking must remain stable; avoid hash-order-dependent simulation behavior.

**Profile-backed coverage.** The main scripted self-play test spawns two non-AI players, gives each
the shared `tech_to_tanks` AI profile through the self-play adapter, and runs the match headlessly
under `cargo test`. The profile gathers steel and oil, constructs supply and tech structures,
trains Riflemen and Tanks, and launches mixed attack-move waves at public enemy start tiles. The
self-play adapter owns harness-only state such as pending build intents, failed build spots, and
staging/attack guards needed to interpret fog-filtered snapshots without duplicating profile
strategy logic. The harness checks per-tick invariants
for invalid resources, supply overflow, malformed entity snapshots, out-of-bounds positions, and
non-finite progress values. It also enforces progress deadlines so a stuck economy/tech/combat loop
fails as a deadlock instead of timing out silently.

Special harness scripts remain where they cover behavior that is not a normal AI strategy profile:
`WorkerRushScript` is an all-in worker-pull scenario, and `MineOnlyScript` is passive mining/fairness
coverage. These scripts are kept isolated from the canonical profile list.

**Artifacts.** On failure, the test writes `target/selfplay-failures/<test>-<pid>-<time>/`
with:
- `replay.json`: start payload, player specs, script decision log, authoritative tick-stamped
  command log, event log, milestone state, and sampled snapshot summaries.
- `summary.log`: short human-readable failure summary and missing milestones.

The artifact is meant to be enough to reproduce or inspect a failing run without manually
playtesting first. By default successful runs do not write artifacts. For manual inspection,
setting `RTS_SELFPLAY_SAVE_REPLAY=1` writes a successful run to
`target/selfplay-artifacts/<test>-<pid>-<time>/`; setting `RTS_SELFPLAY_SAVE_REPLAY=<name>` uses
that explicit safe artifact name instead.

**Profile matchup CLI.** The `ai-matchup` binary is the manual fixed-horizon matchup facility for
profile-vs-profile runs. It composes the same self-play adapter and `Game` seam as the tests, runs
one directed match to elimination or a tick cap, optionally verifies deterministic replay, and can
write a replay artifact:

```bash
cd server
cargo run --bin ai-matchup -- rush tech
cargo run --bin ai-matchup -- saturation tech --seed 7 --ticks 20000 --json
cargo run --bin ai-matchup -- --list-profiles
```

Keep invariant-style milestone coverage in `cargo test`; use the CLI for balance exploration,
seed sweeps, and strategy result sampling.
