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
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |
| `setReplaySpeed` | `speed: f32` | Set replay playback speed multiplier in dev replay rooms; ignored elsewhere. |

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32` | Move selected units to a world point, ignoring enemies until they arrive or receive another order. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32` | Move while attacking enemies encountered; this is the aggressive movement order. |
| `attack`     | `units: u32[]`, `target: u32` | Attack a specific entity. |
| `gather`     | `units: u32[]`, `node: u32` | Send workers to harvest a resource node. |
| `build`      | `worker: u32`, `building: string`, `tileX: u32`, `tileY: u32` | Worker constructs a building at a tile. If the worker is standing inside the requested footprint, the server first tries to walk it to a nearby point outside that footprint and then starts construction. `building` ∈ building kinds. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `cancel`     | `building: u32` | Cancel the front of a building's production queue. |
| `stop`       | `units: u32[]` | Clear orders, hold position. |

Servers MUST ignore commands referencing entities the player does not own, unknown ids,
illegal placements, or unaffordable actions (fail silently or emit a `notice` event).

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool`, `quickstart: bool` |
| `start`    | `Game start payload` (see 2.3). |
| `snapshot` | `Per-player snapshot` (see 2.4). |
| `gameOver` | `winnerId: u32 | null`, `you: "won" | "lost" | "draw"` |
| `pong`     | `ts: number` (echo of the ping ts) |
| `error`    | `msg: string` |

`LobbyPlayer`: `{ id: u32, name: string, ready: bool, color: string, isAi: bool }`. `isAi` is
true for computer opponents (always shown ready; the client renders an "AI" tag and a host-only
remove control instead of a ready toggle).

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
enabled, every player starts with 1400 steel and 600 oil instead of the default opening resources.

### 2.4 `snapshot` payload (per-player, fog-filtered)
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
  facing?: f32,                  // radians, for unit orientation (optional)
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
  setupState?: string            // machine_gunner only: "packed","setting_up","deployed","tearing_down"
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
    map.rs       # Map: handcrafted terrain asset loading, passability, base-site validation
    entity.rs    # Entity, EntityKind, EntityStore (slotmap-style Vec + free list)
    pathfinding.rs # A* over the tile grid (impassable = terrain + building footprints)
    fog.rs       # per-player visibility grid (visible / explored)
    systems.rs   # orchestrator: runs services in order each tick
    services/    # per-tick internal services: commands, move_coordinator, movement (incl. unit collision), combat, economy, production, construction, death, occupancy, supply, pathing
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
    /// `seed`, assigns the first N shuffled pairs to the N players in lobby order, and spawns
    /// each player's starting Industrial Center + STARTING_WORKERS workers + nearby steel/oil
    /// resource clusters. Shuffling keeps each start glued to its paired expansion but stops the
    /// lobby seat order from pinning the human/AI to the same corner every match.
    pub fn new(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a match with explicit starting steel/oil for every player.
    pub fn new_with_starting_resources(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: Command);

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Build a full-world snapshot for a dev watch client. Normal gameplay must not use this.
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot;

    /// Player ids still alive. Humans need at least one building; AI players also need a unit.
    pub fn alive_players(&self) -> Vec<u32>;

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
`StartPayload`, `Snapshot`, `Command`, `Event` are the serde types from `protocol.rs`.
(`game` may use internal types and convert at the boundary, or use protocol types directly —
implementer's choice, but `snapshot_for`/`start_payload` must return protocol types.)

`PlayerInit.is_ai` marks a computer-controlled player. AI players are full players in every
respect (they get a start position, Industrial Center, workers, economy, and count toward win/elimination); the
only difference is they have no socket. `Game` owns one `AiController` per AI player and drives
them at the top of `tick()` — see §8.

### 3.2 Concurrency model
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`).
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` to push to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `Command`). The room task is the
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
`fog.update`, `renderer.render`, `hud.update`, `minimap.render`); on `gameOver` show overlay.

### 4.2 Rendering & look (PixiJS, procedural art — neutral PS1 field-command style)
- Layers (back→front): terrain → resource nodes → building shadows → buildings → unit
  shadows → units → selection rings → health bars → fog overlay → placement ghost →
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
  from `resourceDeltas` via size/opacity.
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
- No mines, morale, logistics, suppression-depth model, or detailed tank armor model yet.

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

The current implementation uses the themed unit/building names below. Combat is still handled
by the simple shared attack model plus the machine-gunner setup/teardown state; armor facings
and forest-specific rules are future work. The unit, building, and resource-node tables below are
the human-readable form of the authoritative `rules::defs` records.

- `TICK_HZ = 30`, `SNAPSHOT_EVERY_N_TICKS = 1`.
- `MACHINE_GUNNER_SETUP_TICKS = 30` (~1s setup or teardown).
- Map: `TILE_SIZE = 32` px. The live map is the hardcoded handcrafted asset at
  `server/assets/maps/default.json` (96×96 today), served for tooling at `/maps/default.json`.
  Its JSON uses row strings (`.` grass, `#` rock, `~` water) plus ordered `baseSites`.
- Start: `STARTING_STEEL = 50`, `STARTING_OIL = 0`, `STARTING_WORKERS = 4`,
  one Industrial Center at the player's start tile, 18 steel patches + 3 oil patches nearby.
- Supply: Industrial Center gives `+10`, Depot gives `+8`, hard cap `200`.
- Attached mining: workers walk to a patch, latch onto it, and mine in place.
  Every `HARVEST_TICKS = 40` the load (`STEEL_LOAD = 2` / `OIL_LOAD = 2`) is deposited
  directly into the player's economy. When a patch empties the worker goes idle
  (no automatic retarget).
- One worker per patch: each node has a single harvest slot (`Entity::miner`). A patch is
  occupied only after the worker reaches `GatherPhase::Harvesting`; right-clicking a patch
  does not reserve it. Extra workers that arrive while the slot is taken go idle. The slot
  is advisory and self-heals — it's only honored while the recorded worker is alive and
  actively harvesting that node, so death / re-order / retarget free it automatically.
- Starting layout: each base site gets 18 steel patches and 3 oil patches. `baseSites` are stored
  as interleaved pairs: `[start0, expansion0, start1, expansion1, ...]`. The pairs are shuffled
  by the match seed (each start stays glued to its paired expansion), and the first N shuffled
  pairs become the active player starts + paired neutral expansion bases. Sites beyond the first
  N pairs are unused, giving exactly 2N active bases on the map. Shuffling stops the lobby seat
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
| industrial_center          | 600 | 9     | 400 | 3x3  | 400       | trains worker; +10 supply; players start with one free |
| depot                      | 220 | 4     | 100 | 2x2  | 120       | +8 supply |
| barracks                   | 320 | 6     | 150 | 3x2  | 200       | trains rifleman, machine_gunner, at_team; requires an Industrial Center |
| training_centre   | 300 | 6     | 100 steel + 50 oil | 3x2  | 220       | unlocks machine_gunner and at_team training at barracks; requires an Industrial Center |
| tank_factory               | 360 | 6     | 200 steel + 100 oil | 3x3  | 240       | trains tank; requires an Industrial Center and Training Centre |

Win: a player is **eliminated** when they own zero buildings (units alone do not keep them
alive). Last player standing wins; a 1-player match never ends (sandbox/exploration mode).

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
  clumping and rewards tighter army control.
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
- **Unit collision**: `services::movement::resolve_collisions` runs after production each tick
  and pair-wise pushes overlapping mobile units apart along the connecting line (50/50 split
  when neither is anchored). A worker is *anchored* while it is in
  `GatherPhase::Harvesting` or `BuildPhase::Constructing` — anchored units neither push nor
  are pushed, which keeps walking units from being deadlocked by miners or active builders
  (PLAN §4.3). `Game::assert_invariants` then asserts that no two non-anchored mobile units
  overlap by more than `OVERLAP_TOLERANCE_PX` (residue from pushes that landed against
  impassable terrain).

---

## 8. AI opponents (optional, `game/ai.rs`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). The lobby also has a host-only
`setQuickstart` toggle labeled "Start with more money mode", which causes the next match to begin
with 1400 steel and 600 oil for every player. They are capped with humans at
`MAX_PLAYERS = 4` (the hardcoded map has enough ordered `baseSites` for four starts plus neutral
expansions). AI players are seated after the humans in the lobby player list; their colors come
from the tail of `PLAYER_PALETTE` so they never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.

**Where it runs.** `Game` holds one `AiController` per AI player and drives them at the top of
`tick()`, *before* commands are applied. Each controller pushes ordinary `Command`s onto the same
pending queue a human client feeds, so every AI action goes through the identical
validation / cost / supply / placement path in `services/commands.rs` — the AI has **no special authority**
over the simulation and can't cheat economy or placement rules. Because the controller is
server-side (not a network client) it reads authoritative state directly rather than a fog-filtered
snapshot; that is not a fog violation (fog only guards what's sent to *human* clients over the
wire). To stay fair it only ever targets enemy **start tiles**, which are public via the `start`
payload.

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
ranked decision loop (`decision.rs`) that emits ordinary `Command`s through shared action helpers.
The first code-defined profiles are `rifle_flood_fast`, `rifle_flood_full_saturation`, and
`tech_to_tanks`; they parameterize worker targets, supply buffers, building/tech goals, production
priorities, resource timing, and attack thresholds without providing their own `think()` functions.
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
`start_payload()`, `snapshot_for(player)`, `enqueue(player, Command)`, `tick()`, `alive_players()`,
and `tick_count()`. Scripts observe the same fog-filtered snapshots a client would receive and
issue ordinary wire-protocol `Command`s. They must not mutate entities, players, map state, or
private system internals. This keeps the simulation architected for future API clients, replay
tools, and external test drivers without adding a second privileged control path.

**Command log replay.** `Game` records every command at the authoritative apply tick, after AI
controllers have emitted their normal commands and before systems apply the pending queue.
`game/replay.rs` can feed that log into a fresh `Game` with AI thinking disabled and compare the
resulting event stream and final per-player snapshots. Replay uses the same public command path as
clients, so a replay proves both the recorded command artifact and the deterministic simulation
ordering. Entity iteration and A* tie-breaking must remain stable; avoid hash-order-dependent
simulation behavior.

**MVP coverage.** The first scripted self-play test spawns two non-AI players, gives each the same
deterministic build/tech/attack script, and runs the match headlessly under `cargo test`. The
scripts gather steel and oil, construct a Depot, Barracks, and Tank Factory, train Riflemen and a Tank,
and attack-move toward a public combat rendezvous that sits four tiles toward the center from each
player's start line. The harness checks per-tick invariants
for invalid resources, supply overflow, malformed entity snapshots, out-of-bounds positions, and
non-finite progress values. It also enforces progress deadlines so a stuck economy/tech/combat loop
fails as a deadlock instead of timing out silently.

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
