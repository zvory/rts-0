# RTS — Design & Architecture

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
- The same Rust process serves the static client files, so development is a single
  `cargo run` and then open `http://localhost:8080`.

### Tick & networking model
- `TICK_HZ = 10` (100 ms per simulated tick).
- The server broadcasts a snapshot every `SNAPSHOT_EVERY_N_TICKS` ticks (default 1 →
  10 snapshots/s).
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
| `command`  | `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. |
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32` | Move selected units to a world point. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32` | Move while attacking enemies encountered. |
| `attack`     | `units: u32[]`, `target: u32` | Attack a specific entity. |
| `gather`     | `units: u32[]`, `node: u32` | Send workers to harvest a resource node. |
| `build`      | `worker: u32`, `building: string`, `tileX: u32`, `tileY: u32` | Worker constructs a building at a tile. `building` ∈ building kinds. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `cancel`     | `building: u32` | Cancel the front of a building's production queue. |
| `stop`       | `units: u32[]` | Clear orders, hold position. |

Servers MUST ignore commands referencing entities the player does not own, unknown ids,
illegal placements, or unaffordable actions (fail silently or emit a `notice` event).

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool` |
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
    terrain: number[]
  },
  players: [ { id, name, color, startTileX, startTileY } ],
}
```
Resource nodes and all units/buildings arrive via snapshots (so they obey fog), including
the player's own starting HQ + workers.

### 2.4 `snapshot` payload (per-player, fog-filtered)
```
{
  t: "snapshot",
  tick: u32,
  minerals: u32, gas: u32,       // your resources
  supplyUsed: u32, supplyCap: u32,
  entities: Entity[],            // your entities (always) + neutral/enemy on visible tiles
  events: Event[]                // transient things to surface (see 2.5)
}
```
`Entity` (lean; omit fields that don't apply):
```
{
  id: u32,
  owner: u32,                    // 0 = neutral (resources), else player id
  kind: string,                  // EntityKind: "worker","soldier","heavy","hq","depot","barracks","turret","minerals","gas"
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
  carrying?: u32,                // amount of resource being carried (0 if none)
  carryingKind?: string,         // "minerals" | "gas"
  // resource nodes:
  remaining?: u32,               // resource left in the node
  // combat feedback:
  targetId?: u32                 // current attack target, for drawing tracers
}
```

### 2.5 `Event` (transient, one snapshot only)
```
{ e: "attack", from: u32, to: u32 }            // for muzzle flashes / tracers
{ e: "death",  id: u32, x: f32, y: f32, kind } // for death poofs
{ e: "build",  id: u32, kind: string }         // building completed
{ e: "notice", msg: string }                   // "Not enough minerals", etc.
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
  game/
    mod.rs       # Game struct + public API (the seam below)
    map.rs       # Map: terrain grid, generation, passability, resource node placement
    entity.rs    # Entity, EntityKind, EntityStore (slotmap-style Vec + free list)
    pathfinding.rs # A* over the tile grid (impassable = terrain + building footprints)
    fog.rs       # per-player visibility grid (visible / explored)
    systems.rs   # per-tick systems: orders, movement, combat, gather, production, death
    ai.rs        # optional computer opponents: one AiController per AI player (see §8)
    selfplay.rs  # test-only API-driven scripted self-play harness (see §9)
```

### 3.1 `game::Game` public API (seam between `game` and `lobby`/`main`)
The `lobby`/networking layer interacts with the simulation ONLY through this surface.
`game-core` implementer: provide exactly these. `server-shell` implementer: call only these.

```rust
pub struct Game { /* private */ }

impl Game {
    /// Create a match for the given players (ids + colors + names already assigned by lobby).
    /// Generates a symmetric map sized for `players.len()` and spawns each player's
    /// starting HQ + STARTING_WORKERS workers + a nearby mineral cluster & geyser.
    pub fn new(players: &[PlayerInit]) -> Game;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: Command);

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Player ids still alive (have at least one entity). Lobby uses this for game-over.
    pub fn alive_players(&self) -> Vec<u32>;

    /// Remove all of a player's entities (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32);

    pub fn tick_count(&self) -> u32;
}

pub struct PlayerInit { pub id: u32, pub name: String, pub color: String, pub is_ai: bool }
```
`StartPayload`, `Snapshot`, `Command`, `Event` are the serde types from `protocol.rs`.
(`game` may use internal types and convert at the boundary, or use protocol types directly —
implementer's choice, but `snapshot_for`/`start_payload` must return protocol types.)

`PlayerInit.is_ai` marks a computer-controlled player. AI players are full players in every
respect (they get a start position, HQ, workers, economy, and count toward win/elimination); the
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

---

## 4. JS client — modules & exported APIs

`client/` (ES modules, no bundler; `index.html` imports `src/main.js` as a module).
PixiJS is loaded globally from CDN as `PIXI`.

```
index.html        # PINNED — CDN + #app + module entry + screens markup
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
  command(cmd)                           // cmd built via protocol.js builders
  ping()
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
  resources                             // {minerals,gas,supplyUsed,supplyCap} (latest)
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

### 4.2 Rendering & look (PixiJS, vector art — "simple but nice")
- Layers (back→front): terrain → resource nodes → building shadows → buildings → unit
  shadows → units → selection rings → health bars → fog overlay → placement ghost →
  selection drag-box → (HUD is DOM, not Pixi).
- Units: clean vector shapes tinted by player color, with a soft drop shadow, a thin dark
  outline, a small facing indicator, an HP bar above when damaged/selected, and a glowing
  selection ring when selected. Distinct silhouette per kind (worker: small rounded;
  soldier: chevron/triangle; heavy: chunky rounded square).
- Buildings: rounded rectangles footprint-sized, player-tinted, with an icon glyph; under
  construction → dashed/translucent with a progress bar; production → small progress arc.
- Resource nodes: minerals = cyan crystals cluster; gas = green geyser; show remaining via
  size/opacity.
- Terrain: high-contrast two-tone grass checker or noise so movement is readable; impassable
  rock and water tiles must remain visually distinct. Thin grid optional at high zoom.
- Fog: unexplored = 82% dark overlay so terrain remains faintly readable; explored-but-not-visible =
  55% dark overlay; visible = clear. Use a single overlay sprite/graphics updated from `fog`
  grids; soften edges if cheap.
- Selection: green for own, red tint for enemy, yellow for neutral. Drag-box translucent green.
- Keep a cohesive palette; define colors in `config.js`.

---

## 5. Balance & constants (authoritative in `server/src/config.rs`)
`client/src/config.js` mirrors the subset the UI/render/fog needs (costs, supply, sight,
sizes, colors). Keep both in sync; the comment in each file points at the other.

- `TICK_HZ = 10`, `SNAPSHOT_EVERY_N_TICKS = 1`.
- Map: `TILE_SIZE = 32` px. Size scales with player count: 2p → 64×64, 3-4p → 96×96.
- Start: `STARTING_MINERALS = 50`, `STARTING_GAS = 0`, `STARTING_WORKERS = 4`,
  one HQ at the player's start tile, a mineral cluster (8 patches) + 1 gas geyser nearby.
- Supply: HQ gives `+10`, Depot gives `+8`, hard cap `200`.
- Resource trip: worker carries `MINERAL_LOAD = 5` / `GAS_LOAD = 4`; harvest takes
  `HARVEST_TICKS`; mineral patch starts with `MINERAL_PATCH_AMOUNT = 1500`, geyser
  `GAS_GEYSER_AMOUNT = 5000`.
- One worker per patch: each node has a single harvest slot (`Entity::miner`). At most one
  worker may be in `Harvesting` on a node at a time; extra workers queue in place (`ToNode`)
  and take the slot when the current miner leaves to deposit. The slot is advisory and
  self-heals — it's only honored while the recorded worker is alive and actively harvesting
  that node, so death / re-order / retarget free it automatically.

Unit stats (hp, dmg, range[tiles], cooldown[ticks], speed[px/tick], sight[tiles], cost, supply, buildTicks):

| kind    | hp  | dmg | range | cd | speed | sight | min | gas | sup | buildTicks |
|---------|-----|-----|-------|----|-------|-------|-----|-----|-----|-----------|
| worker  | 40  | 4   | 1     | 12 | 3.0   | 7     | 50  | 0   | 1   | 120 (~12s)|
| soldier | 45  | 5   | 4     | 8  | 3.2   | 8     | 50  | 0   | 1   | 150 (~15s)|
| heavy   | 130 | 20  | 3     | 18 | 2.0   | 7     | 100 | 50  | 2   | 250 (~25s)|

Building stats (hp, sight, cost min, footprint tiles wxh, buildTicks, extra):

| kind     | hp  | sight | min | foot | buildTicks | notes |
|----------|-----|-------|-----|------|-----------|-------|
| hq       | 600 | 9     | 400 | 3x3  | 400       | trains worker; drop-off; +10 supply; players start with one free |
| depot    | 220 | 4     | 50  | 2x2  | 120       | +8 supply |
| barracks | 320 | 6     | 100 | 3x2  | 200       | trains soldier, heavy; requires an existing hq |
| turret   | 200 | 6     | 75  | 1x1  | 120       | auto-attacks: dmg 10, range 7, cd 10 |

Win: a player is **eliminated** when they own zero entities (units AND buildings). Last
player standing wins; a 1-player match never ends (sandbox/exploration mode).

---

## 6. Conventions
- Rust: edition 2021, `cargo fmt`, `#![deny(warnings)]` off (warnings ok), no `unwrap()` on
  network/parse paths — handle errors and keep the room alive. Prefer small pure functions in
  `systems.rs`. Avoid panics in the tick loop.
- JS: ES2020 modules, no framework, small classes per §4, JSDoc on public methods, no global
  state except `PIXI`. Pure helpers where possible.
- Both: names match this doc. Document any deviation here in the same change.
- Coordinates: world pixels everywhere on the wire; tiles only where a field ends in `Tile`.

---

## 7. Hardening (input is untrusted)
The server treats every client as potentially hostile. Limits live next to the code:
- **WebSocket frame cap** (`main.rs`): `max_message_size`/`max_frame_size` = 256 KiB. Oversized
  frames are rejected and the connection closed before they reach serde.
- **Command unit cap** (`systems.rs` `MAX_UNITS_PER_COMMAND = 256`): unit-list commands are
  deduped and capped before per-unit work, so a repeated/huge id list can't trigger an A* storm.
- **Bounds-checked placement** (`systems.rs` `footprint_tiles`): tile math uses `checked_add` and
  out-of-range build coords are rejected — the tick loop never panics on adversarial input.
- **Idle timeout + heartbeat**: the server drops connections idle for `IDLE_TIMEOUT = 40s`
  (`main.rs`); the client pings every 15s (`main.js`). This evicts half-open/stuck clients so a
  silent player can't wedge a shared room, and frees the room slot.
- **Join ack**: `RoomEvent::Join` carries a `oneshot<bool>`; a connection only marks itself joined
  on an accept, so a rejected mid-match join doesn't wedge the socket.
- **Fog is authoritative**: `snapshot_for` and per-recipient event delivery gate entity views,
  `target_id` tracers, and death events on visibility — hidden enemies are never sent.

---

## 8. AI opponents (optional, `game/ai.rs`)

Computer opponents are **opt-in**: a room has none unless the host adds them from the lobby
(`addAi` / `removeAi`, host-only, lobby phase only). They are capped with humans at
`MAX_PLAYERS = 4` (the map lays out at most four symmetric starts). AI players are seated after
the humans in the lobby player list; their colors come from the tail of `PLAYER_PALETTE` so they
never collide with human colors. They persist across rematches and are cleared only when the room
empties of humans.

**Where it runs.** `Game` holds one `AiController` per AI player and drives them at the top of
`tick()`, *before* commands are applied. Each controller pushes ordinary `Command`s onto the same
pending queue a human client feeds, so every AI action goes through the identical
validation / cost / supply / placement path in `systems.rs` — the AI has **no special authority**
over the simulation and can't cheat economy or placement rules. Because the controller is
server-side (not a network client) it reads authoritative state directly rather than a fog-filtered
snapshot; that is not a fog violation (fog only guards what's sent to *human* clients over the
wire). To stay fair it only ever targets enemy **start tiles**, which are public via the `start`
payload.

**Strategy (deliberately "very basic").** Each controller, on a staggered cadence
(`DECISION_INTERVAL` ticks): keeps idle workers mining the nearest mineral patch; trains workers
up to `TARGET_WORKERS`; builds a depot when supply is about to choke; builds up to
`TARGET_BARRACKS` barracks; pumps soldiers from each barracks; and once `WAVE_SIZE` soldiers are
free, attack-moves them at the nearest living enemy's base. It does not micro, tech to heavies, or
scout. A local per-think budget prevents it from over-committing minerals/supply it doesn't have.

**Win/elimination.** AI players count exactly like humans: a 1-human + N-AI match is a real match
(it resolves to a winner), while a lone human with no AI remains a never-ending sandbox. The
lobby's `match_player_count` is humans **+** AIs.

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

**MVP coverage.** The first scripted self-play test spawns two non-AI players, gives each the same
deterministic build/tech/attack script, and runs the match headlessly under `cargo test`. The
scripts gather minerals and gas, construct a Depot and Barracks, train Soldiers and a Heavy, and
attack-move toward a public map-center combat rendezvous. The harness checks per-tick invariants
for invalid resources, supply overflow, malformed entity snapshots, out-of-bounds positions, and
non-finite progress values. It also enforces progress deadlines so a stuck economy/tech/combat loop
fails as a deadlock instead of timing out silently.

**Failure artifacts.** On failure, the test writes `target/selfplay-failures/<test>-<pid>-<time>/`
with:
- `replay.json`: start payload, player specs, command log, event log, milestone state, and sampled
  snapshot summaries.
- `summary.log`: short human-readable failure summary and missing milestones.

The artifact is meant to be enough to reproduce or inspect a failing run without manually
playtesting first. It is only written on failure so successful test runs do not churn the worktree.
