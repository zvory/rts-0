## 2. Wire protocol (JSON over WebSocket)

All messages are JSON objects with a `t` field (the discriminator/tag). Field names are
short but readable. Coordinates are **world pixels** (floats) unless a field name ends in
`Tile`. The canonical definitions live in `server/src/protocol.rs` (serde) and
`client/src/protocol.js` (builders + constants). These two files MUST agree.

### 2.1 Client → Server (`ClientMessage`)

| `t`        | Fields | Meaning |
|------------|--------|---------|
| `join`     | `name: string`, `room?: string`, `spectator?: bool` | Join (or create) a room. `room` defaults to `"main"`. If `spectator` is true, join as a lobby-time observer instead of a match participant. |
| `ready`    | `ready: bool` | Toggle ready state in the lobby. |
| `start`    | — | Host asks to start the match (only honored from the room host). |
| `addAi`    | — | Host adds a computer opponent to the room (lobby phase only, host-only). |
| `removeAi` | `id: u32` | Host removes a previously-added AI opponent by id (lobby phase only, host-only). |
| `setQuickstart` | `enabled: bool` | Host toggles "Debug mode" for the next match in this room. |
| `setSpectator` | `spectator: bool` | Switch between active player and spectator role while still in the lobby. Ignored after the match starts; switching to active player is ignored if the active seats are full. |
| `command`  | `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. |
| `giveUp`   | — | Give up the active match. The server eliminates that player and sends their score screen. |
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |
| `setReplaySpeed` | `speed: f32` | Set replay playback speed multiplier in dev replay rooms; ignored elsewhere. |
| `seekReplay` | `ticksBack: u32` | Rewind a dev replay by N simulation ticks; pass a large value (e.g. `2^31-1`) to reset to tick 0. Ignored outside replay rooms. The room rebuilds the game from the artifact, fast-forwards to `current - N`, and re-sends `start`. |

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move selected units to a world point. Infantry ignore enemies until they arrive or receive another order; tanks and scout cars keep driving and fire at in-range enemies without chasing. When `queued` is true, store future movement intent instead of replacing the active order. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move while attacking enemies encountered; this is the aggressive movement order. When `queued` is true, store future attack-move intent instead of replacing the active order. |
| `attack`     | `units: u32[]`, `target: u32`, `queued?: bool` | Attack a specific entity. When `queued` is true, store future attack intent instead of replacing the active order. |
| `setupAtGuns` | `units: u32[]`, `x: f32`, `y: f32` | Manually emplace owned AT guns toward a world point. The server filters the unit list to owned, completed AT guns, clears movement/target state, records the setup facing, and enters `setting_up`. Other selected units are ignored. |
| `tearDownAtGuns` | `units: u32[]` | Pack up owned AT guns that are `setting_up` or `deployed`. Other selected units are ignored. |
| `charge`     | `units: u32[]` | Activate Rifleman Charge on owned riflemen. Requires at least one completed Training Centre. The server filters the unit list to owned, completed riflemen and sets a short sprint timer; other selected units are ignored. |
| `gather`     | `units: u32[]`, `node: u32`, `queued?: bool` | Send workers to harvest a resource node. When `queued` is true, store future gather intent instead of replacing the active order. |
| `build`      | `worker: u32`, `building: string`, `tileX: u32`, `tileY: u32`, `queued?: bool` | Worker constructs a building at a tile. The server first walks the worker to a nearby point outside the requested footprint, then starts construction once it is in range. `building` ∈ building kinds. When `queued` is true, store future build intent instead of replacing the active order. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `cancel`     | `building: u32` | Cancel the front of a building's production queue. |
| `stop`       | `units: u32[]` | Clear orders, hold position. |
| `setRally`   | `building: u32`, `x: f32`, `y: f32`, `queued?: bool` | Set a unit-producing building's rally point. Freshly produced units receive a plain `move` order to the point and the building prefers the spawn exit nearest it. Ignored for buildings the player doesn't own, non-producers (depot, training centre), or buildings still under construction. The point is clamped into map bounds. When `queued` is true, store a future rally stage instead of replacing the active rally point. |

Servers MUST ignore commands referencing entities the player does not own, unknown ids,
illegal placements, or unaffordable actions (fail silently or emit a `notice` event).
For appendable commands, omitted `queued` is equivalent to `false`. Unit order queues are capped at
8 intents per unit, and building rally stage queues are capped at 2 stages per building. Queued
intents are lightweight future intent only; active `Order` remains the per-tick execution state.

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

`LobbyPlayer`: `{ id: u32, name: string, ready: bool, color: string, isAi: bool, isSpectator: bool }`. `isAi` is
true for computer opponents (always shown ready; the client renders an "AI" tag and a host-only
remove control instead of a ready toggle). `isSpectator` is true for human observers; they do not
consume active map starts, block readiness, or count toward win/loss.

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
  spectator: bool,               // true when this connection is observing only
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
  players: [ { id, name, color, startTileX, startTileY } ], // active match players only
}
```
Units/buildings arrive via snapshots (so they obey fog), including
the player's own starting City Centre + workers. When the lobby's `setQuickstart` toggle is
enabled, every player starts with 99,999 steel and 99,999 oil instead of the default opening
resources, and each human player also starts with five supply depots, one Steelworks, one Training
Centre, two Barracks, two Factories, and five of each unit kind.
Spectator start payloads keep the spectator connection's `playerId`, set `spectator: true`, and
list only active match players in `players`.

### 2.4 `snapshot` payload (per-player, fog-filtered)
`Snapshot` remains the semantic shape used by server game code and by client modules after
transport decode:
```
{
  t: "snapshot",
  tick: u32,
  steel: u32, oil: u32,       // your resources
  supplyUsed: u32, supplyCap: u32,
  entities: Entity[],            // your non-resource entities (always) + enemies on live/death-vision-visible tiles
  resourceDeltas?: ResourceDelta[], // visible resource remaining updates; omitted when empty
  events: Event[],               // transient things to surface (see 2.5)
  playerResources?: {id, steel, oil, supplyUsed, supplyCap}[], // all players; spectator/replay mode only
  netStatus: {                // per-recipient server-side health for the current match
    serverLagMs: u16,         // how late this room started the tick vs its scheduled time
    tickMs: u16,              // elapsed room-tick work so far when this snapshot was built
    slowTick: bool,           // true when the room was at/over its tick budget this tick
    slowTickCount: u32,       // number of slow-tick incidents so far this match
    headOfLine: bool,         // true when an older unsent snapshot was still pending for this client
    headOfLineCount: u32      // number of pending-snapshot replacements so far this match
  }
}
```

Live WebSocket snapshot frames are sent as compact JSON text, version 6. `client/src/net.js`
decodes this transport shape back into the semantic object above before dispatching `S.SNAPSHOT`.
Older object-shaped JSON snapshots remain decodable by the client for fallback/dev use.

```
{
  "t": "snapshot",
  "v": 6,
  "s": [tick, steel, oil, supplyUsed, supplyCap],
  "e": [
    [
      id, owner, kind, x, y, hp, maxHp, state,
      facing?, weaponFacing?, prodKind?, prodProgress?, prodQueue?,
      buildProgress?, latchedNode?, targetId?, setupState?, remaining?, rally?, oilUsed?,
      setupFacing?, orderPlan?, chargeCooldownLeft?, visionOnly?, debugPath?
    ]
  ],
  "r": [[id, remaining]],         // omitted when empty
  "ev": [EventRecord],            // omitted when empty
  "pr": [[id, steel, oil, supplyUsed, supplyCap]], // omitted in normal play; present in spectator/replay
  "n": [serverLagMs, tickMs, flags, slowTickCount, headOfLineCount]
}
```

Compact numeric codes:

| Vocabulary | Codes |
|------------|-------|
| `kind` | 1 `worker`, 2 `rifleman`, 3 `machine_gunner`, 4 `at_team`, 5 `tank`, 6 `city_centre`, 7 `depot`, 8 `barracks`, 9 `training_centre`, 10 `factory`, 11 `steel`, 12 `oil`, 13 `steelworks`, 14 `scout_car` |
| `state` | 1 `idle`, 2 `move`, 3 `attack`, 4 `gather`, 5 `build`, 6 `train`, 7 `construct`, 8 `dead` |
| `setupState` | 1 `packed`, 2 `setting_up`, 3 `deployed`, 4 `tearing_down` |
| `notice.severity` | 1 `info`, 2 `warn`, 3 `alert` |
| `EventRecord` | `[1, from, to]` attack, `[1, from, to, reveal?, toPos?]` attack with optional shooter reveal and target position, `[2, id, x, y, kind]` death, `[3, id, kind]` build, `[4, msg]` notice, `[4, msg, severity]` position-free notice with severity, `[4, msg, severity, x, y]` positioned notice |

Compact entity records are positional arrays. Optional fields keep the semantic order above and
trailing missing optional fields are omitted; interior missing optional fields are encoded as
`null`. The `rally` slot is itself a two-element `[x, y]` array (or `null`).
The `orderPlan` slot is an owner-only array capped at 9 entries. It contains the current active
stage first, followed by queued stages in execution order. Each compact stage is `[kind, x, y]`,
where `kind` is 1 `move`, 2 `attackMove`, 3 `attack`, 4 `gather`, or 5 `build`. Stages carry safe
world points only, never target ids; hidden attack target stages may be omitted rather than leaking
enemy positions through fog.
`visionOnly` is true only for non-owned units/buildings visible through lingering death vision;
clients render them below the fog overlay and must not select or issue targeted commands against
them. In `n.flags`, bit 0 = `slowTick` and bit 1 = `headOfLine`.
`debugPath` is present only in lobby Debug mode matches, only for the owner, and only while the unit
has remaining movement waypoints. It carries `{ waypoints, goal, lastRepathTick, stuckTicks,
staticBlockedTicks, totalWaypoints }`, where `waypoints` are remaining `{x, y}` world-pixel path
points in traversal order and `waypoints[0]` is the current movement target. The compact slot
encodes this as `[waypoints, goal, lastRepathTick, stuckTicks, staticBlockedTicks, totalWaypoints]`,
with points encoded as `[x, y]`; `waypoints` is capped at 128 entries for transport.

`ResourceDelta`: `{ id: u32, remaining: u32 }`. Resource node positions/kinds are static and come
from `start.map.resources`; clients keep last-known `remaining` locally. The server sends
`remaining` updates only for resource nodes currently visible to that recipient (dev full-world
watch rooms receive all resource updates).

`Entity` (lean; omit fields that don't apply):
```
{
  id: u32,
  owner: u32,                    // 0 = neutral (resources), else player id
  kind: string,                  // EntityKind: "worker","rifleman","machine_gunner","at_team","scout_car","tank","city_centre","depot","barracks","training_centre","factory","steelworks"
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
  setupState?: string,           // machine_gunner/at_team only: "packed","setting_up","deployed","tearing_down"
  // unit-producing buildings:
  rally?: [f32, f32],            // rally point (world px); ONLY ever sent to the owner
  // tanks:
  oilUsed?: f32,                 // lifetime oil burned by movement, in resource units
  setupFacing?: f32,             // at_team only: owner-visible deployed arc center; appended after oilUsed in compact snapshots
  orderPlan?: [                  // current + queued order stages; ONLY ever sent to the owner
    { kind: "move"|"attackMove"|"attack"|"gather"|"build", x: f32, y: f32 }
  ],
  chargeCooldownLeft?: u16,      // rifleman only: owner-visible remaining Charge cooldown in ticks
  visionOnly?: bool,             // true = visible only through one-second death vision; visual intel only
  debugPath?: {                  // lobby Debug mode only; remaining movement path; ONLY ever sent to the owner
    waypoints: { x: f32, y: f32 }[],
    goal?: { x: f32, y: f32 },
    lastRepathTick: u32,
    stuckTicks: u16,
    staticBlockedTicks: u16,
    totalWaypoints: u16
  },
}
```

### 2.5 `Event` (transient, one snapshot only)
```
{ e: "attack", from: u32, to: u32,
  reveal?: { owner: u32, kind: string, x: f32, y: f32, facing?: f32, weaponFacing?: f32, setupState?: string },
  toPos?: [f32, f32] }                         // for muzzle flashes / tracers
{ e: "death",  id: u32, x: f32, y: f32, kind } // for death poofs
{ e: "build",  id: u32, kind: string }         // building completed
{ e: "notice", msg: string, severity?: "info"|"warn"|"alert", x?: f32, y?: f32 }
```
Notices default to `severity: "info"` with no position. `alert:`-prefixed notice ids are
gameplay alerts: the client plays alert audio and pings the minimap at `(x, y)` when present,
or pulses the minimap border when absent. `alert:under_attack` is emitted at the damaged unit's
position after normal fog/visibility filtering. Unit attack events include `reveal` so a shooter
that fires from fog can be rendered briefly as a semi-transparent, non-interactive silhouette above
the fog overlay; `toPos` lets tracers draw even when the hit target is no longer in the snapshot.
Events are best-effort visual flavor; the client must not depend on receiving them.

---
