## 2. Wire protocol (JSON over WebSocket)

All messages are JSON objects with a `t` field (the discriminator/tag). Field names are
short but readable. Coordinates are **world pixels** (floats) unless a field name ends in
`Tile`. The canonical Rust definitions live in `server/crates/protocol/src/lib.rs`; the
server-shell `server/src/protocol.rs` is an adapter for typed entity-kind conversion and legacy
imports. The browser mirror lives in `client/src/protocol.js` (builders + constants). Rust and JS
MUST agree on every tag, field name, and compact transport shape.

`rts-protocol` may depend on `rts-contract` but must not depend on `rts-sim`, `rts-rules`,
`rts-ai`, or `rts-server`. Domain kind conversion that needs `EntityKind` belongs in an adapter
layer such as `server/src/protocol.rs` or `server/crates/sim/src/protocol.rs`, not in the wire DTO
crate.

### 2.1 Client → Server (`ClientMessage`)

| `t`        | Fields | Meaning |
|------------|--------|---------|
| `join`     | `name: string`, `room?: string`, `spectator?: bool`, `replayOk?: bool` | Join (or create) a room. `room` defaults to `"main"`. If `spectator` is true, join as a lobby-time observer instead of a match participant. If the target room is replay playback, the first join is rejected with `joinReplayPrompt`; retry with `replayOk: true` only after user confirmation. If the same WebSocket is already in a different room and the new room accepts the join, the connection transfers to the new room and leaves the previous room. |
| `ready`    | `ready: bool` | Toggle ready state in the lobby. |
| `start`    | — | Host asks to start the match (only honored from the room host). |
| `addAi`    | — | Host adds a computer opponent to the room (lobby phase only, host-only). |
| `removeAi` | `id: u32` | Host removes a previously-added AI opponent by id (lobby phase only, host-only). |
| `setQuickstart` | `enabled: bool` | Host toggles "Debug mode" for the next match in this room. |
| `setSpectator` | `spectator: bool` | Switch between active player and spectator role while still in the lobby. Ignored after the match starts; switching to active player is ignored if the active seats are full. |
| `command`  | `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. |
| `giveUp`   | — | Give up the active match. The server eliminates that player and sends their score screen. |
| `returnToLobby` | — | Leave post-match replay playback and return a normal match room to a clean lobby for rematch setup. Ignored outside replay playback and ignored by dedicated replay rooms created for match-history/dev replay viewing. |
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |
| `netReport` | `report: ClientNetReport` | Periodic client-observed network/render health aggregate. Server logs notable reports for diagnostics only; it never affects simulation state. |
| `setReplaySpeed` | `speed: f32` | Set replay/dev-watch playback speed multiplier; ignored outside replay rooms and dev watch playback. `0` pauses replay playback and dev scenario watch rooms. Other accepted speeds are clamped. |
| `stepDevTick` | — | Advance a paused dev scenario watch room by one authoritative simulation tick. Ignored outside paused dev scenario rooms. |
| `seekReplay` | `ticksBack: u32` | Rewind a replay by N simulation ticks; pass a large value (e.g. `2^31-1`) to reset to tick 0. Ignored outside replay rooms. Compatibility wrapper around absolute replay seek. |
| `seekReplayTo` | `tick: u32` | Seek a replay to an absolute simulation tick, clamped to the replay duration. Ignored outside replay rooms. The room rate-limits accepted seeks. Accepted seeks restore the nearest recorded replay keyframe at or before the target tick, fast-forward the remaining ticks, re-send `start`, and emit `replayState`. Replay rooms record authoritative keyframes every 2,000 ticks while playback/seek fast-forwarding advances. |
| `setReplayVision` | `vision: ReplayVisionRequest` | Select replay fog/vision for this viewer only. Ignored outside replay rooms. The server validates the request and applies it to that viewer's subsequent snapshot projection. |
| `requestReplayBranch` | — | Request creation of a new practice branch room from this replay room's current authoritative server tick. Ignored before join; rejected outside replay playback. The server rejects replays with AI seats in the first implementation and returns `error`. On success, the source replay room broadcasts `replayBranchCreated` to all current viewers. |
| `claimBranchSeat` | `playerId: u32` | Claim one original replay player seat in a replay branch staging room. Ignored outside branch staging. Rejected with `error` if the seat is unknown, already claimed, or this occupant already claimed another seat. |
| `releaseBranchSeat` | `playerId: u32` | Release one original replay player seat currently claimed by this occupant in branch staging. Ignored outside branch staging or when the occupant does not own that claim. |
| `startBranch` | — | Host asks to launch the staged replay branch. Ignored outside branch staging and from non-hosts. The server rejects launch until every original active seat is claimed; live promotion is handled by the branch promotion phase. |

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move selected units to a world point. Infantry ignore enemies until they arrive or receive another order; tanks and scout cars keep driving and fire at in-range enemies without chasing. When `queued` is true, store future movement intent instead of replacing the active order. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move while attacking enemies encountered; this is the aggressive movement order. When `queued` is true, store future attack-move intent instead of replacing the active order. |
| `attack`     | `units: u32[]`, `target: u32`, `queued?: bool` | Attack a specific entity. When `queued` is true, store future attack intent instead of replacing the active order. |
| `setupAtGuns` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Manually emplace owned AT guns and artillery toward a world point. When `queued` is true, append a future setup-facing intent for owned completed AT teams and artillery only; the stored point is evaluated from the unit's position when the stage promotes. Immediate setup clears movement/target state, records the setup facing, and enters `setting_up`. Other selected units are ignored. |
| `tearDownAtGuns` | `units: u32[]` | Pack up owned AT guns that are `setting_up` or `deployed`. Other selected units are ignored. |
| `charge`     | `units: u32[]` | Legacy Rifleman Charge activation. Preserved for old clients/replays, but no longer has eligible carriers. |
| `useAbility` | `ability: "charge"|"smoke"|"mortarFire"|"pointFire"`, `units: u32[]`, `x?: f32`, `y?: f32`, `queued?: bool` | Generic ability command. `charge` is legacy/no-op; `smoke`, `mortarFire`, and deployed Artillery `pointFire` target a world point. Smoke command execution is phased separately from the authoritative smoke world-state/LOS model; mortar fire schedules a delayed area impact. Artillery point fire requires a deployed gun and is terminal in the unit order queue: once accepted, later queued unit orders are not appended after it. |
| `setAutocast` | `ability: "mortarFire"`, `units: u32[]`, `enabled: bool` | Toggle server-authoritative autocast for owned Mortar Teams. Other unit/ability combinations are ignored. |
| `gather`     | `units: u32[]`, `node: u32`, `queued?: bool` | Send workers to harvest a resource node. When `queued` is true, store future gather intent instead of replacing the active order. |
| `build`      | `units: u32[]`, `building: string`, `tileX: u32`, `tileY: u32`, `queued?: bool` | Selected workers construct a building at a tile. The server allocates one compatible worker per build click, first walks that worker to a nearby point outside the requested footprint, then starts construction once it is in range. `building` ∈ building kinds. When `queued` is true, store future build intent instead of replacing the active order. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `research`   | `building: u32`, `upgrade: string` | Queue a permanent player upgrade at a tech building. Upgrade ids: `methamphetamines` at the Training Centre; `at_gun_unlock`, `artillery_unlock`, `tank_unlock`, and `mortar_autocast` at the R&D Complex (`research_complex`). `artillery_unlock` requires completed `at_gun_unlock`. |
| `cancel`     | `building: u32` | Cancel the latest item in a building's production queue. |
| `stop`       | `units: u32[]` | Clear orders, hold position. |
| `setRally`   | `building: u32`, `x: f32`, `y: f32`, `kind?: "move"|"attackMove"`, `queued?: bool` | Set or append a unit-producing building rally stage. `kind` defaults to `"move"`. Freshly produced units receive the building's rally plan as active + queued move/attack-move orders, and the building prefers the spawn exit nearest the first stage. Ignored for buildings the player doesn't own, non-producers (depot, training centre, research_complex), or buildings still under construction. Points are clamped into map bounds. When `queued` is true, append until the four-stage building rally cap is reached; otherwise replace the whole rally plan. |

Servers MUST ignore commands referencing entities the player does not own, unknown ids,
illegal placements, or unaffordable actions (fail silently or emit a `notice` event).
For appendable unit commands, omitted `queued` is equivalent to `false`. Unit order queues are
capped at 8 intents per unit. Queued intents are lightweight future intent only; active `Order`
remains the per-tick execution state. Non-queued unit orders replace the active order and clear
future unit intents; `stop` clears both active and queued unit orders.
Production building rally plans are capped at four total stages. A non-queued rally replaces the
whole plan; a queued rally appends if space remains and establishes the first stage when the plan is
empty.

`ClientNetReport` is an untrusted, rate-limited diagnostic aggregate emitted by the browser while
in a match:
```
{
  schemaVersion: u8,        // currently 1
  elapsedMs: u32,           // client-side aggregation window duration
  matchTick: u32,           // latest snapshot tick observed by this client
  rttMs: u16,               // latest app-level ping round-trip sample
  rttMaxMs: u16,            // max round-trip sample in this report window
  badRttSamples: u32,       // samples at/above the client's latency warning threshold
  snapshotJitterMs: u16,    // current max receive jitter over the client's short jitter window
  snapshotGapMaxMs: u16,    // largest observed interval between received snapshots
  jitterSamples: u32,       // jitter incidents in this report window
  snapshots: u32,           // snapshots received in this report window
  frameGapMaxMs: u16,       // largest requestAnimationFrame gap in this report window
  fpsEstimate: u16,         // coarse average client frame rate for this report window
  hidden: bool,             // document.hidden when the report was sent
  focused: bool,            // document.hasFocus() when available
  wsBufferedBytes: u32,     // browser WebSocket bufferedAmount
  serverTickMs: u16,        // latest server tick work duration seen in snapshot netStatus
  serverLagMs: u16,         // latest scheduler lag seen in snapshot netStatus
  slowTickCount: u32,       // latest server slow-tick count seen by this client
  headOfLineCount: u32      // latest per-client pending-snapshot replacement count seen
}
```
The server logs this message only when the aggregate contains notable lag, jitter, browser frame
stalls, WebSocket backlog, or server tick/scheduler pressure, alongside the connection's
`player_id` and room name. Values are advisory because clients are untrusted; use them to diagnose
transport/browser behavior, not as gameplay authority.

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool`, `quickstart: bool` |
| `matchCountdown` | `durationMs: u32`, `words: string[]` — reliable pre-match countdown sent to every lobby participant after the host starts and before `start`. During this interval the server keeps the room in lobby setup, disables `canStart`, freezes lobby edits, rejects new joins, and sends `start` only after the countdown duration elapses. |
| `start`    | `Game start payload` (see 2.3). |
| `snapshot` | `Per-player snapshot` (see 2.4). |
| `replayState` | `Replay playback state` (see 2.6). |
| `joinReplayPrompt` | `room: string` — the requested room is currently replay playback; clients should confirm before retrying `join` with `replayOk: true`. |
| `replayBranchCreated` | `branchRoom: string`, `sourceTick: u32`, `seats: ReplayBranchSeat[]` — a separate practice branch room has been created from the source replay's current authoritative tick. |
| `branchStaging` | `room: string`, `sourceTick: u32`, `hostId: u32`, `seats: BranchStagingSeat[]`, `occupants: BranchStagingOccupant[]`, `canStart: bool` — reliable current state for a replay branch staging room. Sent after joins, leaves, claims, and releases. |
| `shutdownWarning` | `deadlineUnixMs: u64`, `secondsRemaining: u64` — deploy/termination drain has started; active matches may continue until the deadline, but new match starts are disabled. |
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

`ReplayBranchSeat`: `{ playerId: u32, name: string, color: string, claimable: bool }`. Seats are
listed in original replay player order. `claimable` is false only for unsupported original seats;
the first implementation rejects AI-seat replays before creating a branch, so successful branch
creation currently reports all seats as claimable.

`BranchStagingSeat`: `{ playerId: u32, name: string, color: string, claimantId?: u32,
claimantName?: string }`. Seats are listed in original replay player order. A missing claimant
means that original seat is still available to claim.

`BranchStagingOccupant`: `{ id: u32, name: string }`. Occupants are all human viewers currently in
the branch staging room, whether they have claimed an original seat or are remaining spectators.

### 2.3 `start` payload
Sent once when the match begins. Carries everything static for the whole match.
```
{
  t: "start",
  playerId: u32,                 // your id (repeat of welcome for convenience)
  spectator: bool,               // true when this connection is observing only
  debugMode?: bool,              // true when movement path diagnostics are available
  replay?: {                     // present for production replay playback
    artifactSchemaVersion: u32,
    serverBuildSha: string,
    mapName: string,
    mapSchemaVersion: u32,
    mapContentHash: string,
    seed: u32,
    durationTicks: u32
  },
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
resources, and each human player also starts with five supply depots, one Gun Works
(`steelworks` kind), one R&D Complex (`research_complex` kind), one Training Centre, two Barracks,
two Vehicle Works (`factory` kind), and five of each unit kind. Debug mode also adds one inert enemy player in the clockwise-adjacent
corner from the first human start, with five deployed Mortar Teams clumped around one Scout Car
and four enemy Supply Depots five tiles north/east/south/west of the clump. It also sets
`debugMode: true`,
which lets the client expose local movement-waypoint overlay controls for the owner-only
`debugPath` fields in snapshots.
Spectator start payloads keep the spectator connection's `playerId`, set `spectator: true`, and
list only active match players in `players`.

Replay start payloads include `replay` metadata so the client can display or cache a
self-describing playback session. The server validates replay artifacts before playback: artifact
schema version, server build SHA, map name, map schema version, and map content hash must match the
running server/map asset or the replay is rejected with a clear error. Saved self-play artifacts use
the same `ReplayArtifactV1` contract as post-match and match-history replays; pre-unified dev-only
artifact payloads are rejected instead of falling back to a separate loader.

When a real multi-player match ends, the server sends the normal `gameOver` score payload, clears
pending latest-only live snapshots for connected humans, and then sends a replay `start` payload
at tick 0 plus `replayState`. Post-match replay defaults every viewer to all active players'
combined authoritative vision and starts at `2.0x` speed. In a normal match room,
`returnToLobby` exits this replay phase, drops the replay simulation, clears ready flags, and
broadcasts a normal lobby snapshot for the next match. Dedicated replay rooms created for
match-history or dev replay viewing ignore `returnToLobby`; they keep the shared replay session
alive until viewers disconnect and the room empties.

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
  smokes?: SmokeCloud[],         // active smoke clouds visible to this recipient; omitted when empty
  visibleTiles?: u8[],           // row-major current server visibility; 1 = visible, 0 = fogged
  rememberedBuildings?: RememberedBuilding[], // recipient-only stale enemy building intel
  events: Event[],               // transient things to surface (see 2.5)
  upgrades?: string[],           // completed permanent upgrades for this recipient
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

Live WebSocket snapshot frames are sent as compact JSON text, version 17. `client/src/net.js`
decodes this transport shape back into the semantic object above before dispatching `S.SNAPSHOT`.
Older object-shaped JSON snapshots remain decodable by the client for fallback/dev use.

```
{
  "t": "snapshot",
  "v": 17,
  "s": [tick, steel, oil, supplyUsed, supplyCap],
  "e": [
    [
      id, owner, kind, x, y, hp, maxHp, state,
      facing?, weaponFacing?, prodKind?, prodProgress?, prodQueue?,
      buildProgress?, latchedNode?, targetId?, setupState?, remaining?, rally?, oilUsed?,
      setupFacing?, orderPlan?, chargeCooldownLeft?, abilities?, visionOnly?, debugPath?,
      rallyPlan?, prodUpgrade?
    ]
  ],
  "r": [[id, remaining]],         // omitted when empty
  "sm": [[id, x, y, radiusTiles, expiresIn]], // omitted when empty
  "fg": [firstValue, runLen, ...], // RLE visibleTiles; omitted when empty/no-fog
  "mb": [[id, owner, kind, x, y, [[tileX, tileY], ...], observedTick]], // rememberedBuildings; omitted when empty
  "ev": [EventRecord],            // omitted when empty
  "pr": [[id, steel, oil, supplyUsed, supplyCap]], // omitted in normal play; present in spectator/replay
  "n": [serverLagMs, tickMs, flags, slowTickCount, headOfLineCount]
}
```

Compact numeric codes:

| Vocabulary | Codes |
|------------|-------|
| `kind` | 1 `worker`, 2 `rifleman`, 3 `machine_gunner`, 4 `at_team`, 5 `tank`, 6 `city_centre`, 7 `depot`, 8 `barracks`, 9 `training_centre`, 10 `factory`, 11 `steel`, 12 `oil`, 13 `steelworks`, 14 `scout_car`, 15 `mortar_team`, 16 `artillery`, 17 `research_complex` |
| `state` | 1 `idle`, 2 `move`, 3 `attack`, 4 `gather`, 5 `build`, 6 `train`, 7 `construct`, 8 `dead` |
| `setupState` | 1 `packed`, 2 `setting_up`, 3 `deployed`, 4 `tearing_down` |
| `upgrade` | 1 `methamphetamines`, 2 `at_gun_unlock`, 3 `tank_unlock`, 4 `artillery_unlock`, 5 `mortar_autocast` |
| `notice.severity` | 1 `info`, 2 `warn`, 3 `alert` |
| `EventRecord` | `[1, from, to]` attack, `[1, from, to, reveal?, toPos?]` attack with optional shooter reveal and target position, `[2, id, x, y, kind]` death, `[3, id, kind]` build, `[4, msg]` notice, `[4, msg, severity]` position-free notice with severity, `[4, msg, severity, x, y]` positioned notice, `[5, [fromX, fromY], [toX, toY], delayTicks]` smoke launch, `[6, x, y, radiusTiles]` mortar impact/marker, `[6, x, y, radiusTiles, from?, reveal?]` mortar impact with optional shooter reveal, `[7, from, [x, y], radiusTiles, delayTicks]` artillery target marker, `[8, x, y, radiusTiles]` artillery impact, `[9, from, [fromX, fromY], [toX, toY], radiusTiles, delayTicks]` mortar launch |

Compact entity records are positional arrays. Optional fields keep the semantic order above and
trailing missing optional fields are omitted; interior missing optional fields are encoded as
`null`. The `rally` slot is itself a two-element `[x, y]` array (or `null`).
The `orderPlan` slot is an owner-only array capped at 9 entries. It contains the current active
stage first, followed by queued unit stages in execution order. Each compact stage is
`[kind, x, y]`, where `kind` is 1 `move`, 2 `attackMove`, 3 `attack`, 4 `gather`, 5 `build`,
6 `smoke`, 7 `setupAtGuns`, 8 `charge`, 9 `mortarFire`, or 10 `pointFire`.
Stages carry safe world points only, never target ids; hidden attack target stages may be omitted
rather than leaking enemy positions through fog. Production building rally points are exposed
separately through `rally` and `rallyPlan` and are not part of `orderPlan`. `rallyPlan` is appended
after `debugPath` in compact snapshots to preserve older optional slot positions; it is owner-only,
capped at four stages, and uses the same `[kind, x, y]` compact stage encoding with `move` and
`attackMove` stages.
The `abilities` slot is owner-only and capped at 8 entries. Each compact ability cooldown is
`[ability, cooldownLeft, remainingUses?, autocastEnabled?]`, where `ability` is 2 `smoke`,
3 `mortarFire`, or 4 `pointFire`; 1 `charge` is legacy.
`remainingUses` is present for finite-use abilities such as Scout Car Smoke; a value of `0`
means the ability is depleted and cannot be used by that caster.
`autocastEnabled` is present for Mortar Team `mortarFire` so the command card can display and
toggle autocast without exposing enemy data.
`visionOnly` is true only for non-owned units/buildings visible through lingering death vision;
clients render them below the fog overlay and must not select or issue targeted commands against
them. In `n.flags`, bit 0 = `slowTick` and bit 1 = `headOfLine`.
`debugPath` is present only in lobby Debug mode matches, only for the owner, and only while the unit
has remaining movement waypoints. It carries `{ waypoints, goal, lastRepathTick, stuckTicks,
staticBlockedTicks, totalWaypoints }`, where `waypoints` are remaining `{x, y}` world-pixel path
points in traversal order and `waypoints[0]` is the current movement target. The compact slot
encodes this as `[waypoints, goal, lastRepathTick, stuckTicks, staticBlockedTicks, totalWaypoints]`,
with points encoded as `[x, y]`; `waypoints` is capped at 128 entries for transport.

`RememberedBuilding`: `{ id, owner, kind, x, y, footprint, observedTick }`. These records are
recipient-only last-seen enemy building memory, sent only when the building is not currently
projected as a live visible entity. They are stale intel for normal building rendering below the
fog overlay and coordinate targeting context; clients must not make them selectable live entities
or issue entity-targeted commands against them. `footprint` is an array of `[tileX, tileY]` cells
from the last visible state. The record intentionally omits hidden live HP, current build progress,
and destruction state. Artillery `pointFire` remains a world-coordinate ability; remembered
buildings help the player know where to aim but do not become target ids.

`ResourceDelta`: `{ id: u32, remaining: u32 }`. Resource node positions/kinds are static and come
from `start.map.resources`; clients keep last-known `remaining` locally. The server sends
`remaining` updates only for resource nodes currently visible to that recipient (dev full-world
watch rooms receive all resource updates).

`SmokeCloud`: `{ id: u32, x: f32, y: f32, radiusTiles: f32, expiresIn: u16 }`. Smoke clouds are
neutral world effects, not entities. Normal player snapshots include only clouds that have at least
one currently visible tile after smoke-suppressed fog is recomputed, plus any cloud currently
containing one of that player's own non-resource entities; spectator/dev full-world
snapshots may include all active clouds. Smoke-covered enemy units/buildings, target ids, death
events, and positioned notices remain fog-gated and are withheld when smoke hides the position.

`Entity` (lean; omit fields that don't apply):
```
{
  id: u32,
  owner: u32,                    // 0 = neutral (resources), else player id
  kind: string,                  // EntityKind: "worker","rifleman","machine_gunner","at_team","mortar_team","artillery","scout_car","tank","city_centre","depot","barracks","training_centre","research_complex","factory","steelworks"
  x: f32, y: f32,                // world px (center)
  hp: u32, maxHp: u32,
  state: string,                 // "idle","move","attack","gather","build","train","construct","dead"
  facing?: f32,                  // radians, for unit body/hull orientation (optional)
  weaponFacing?: f32,            // radians, for independent weapon/barrel orientation (optional)
  // production buildings:
  prodKind?: string,             // unit currently being produced
  prodUpgrade?: string,          // upgrade currently being researched
  prodProgress?: f32,            // 0..1
  prodQueue?: u32,               // queued count (including the in-progress one)
  // buildings under construction:
  buildProgress?: f32,           // 0..1; when present and <1, render as scaffolding
  // workers:
  latchedNode?: u32,             // node id the worker is currently harvesting (attached mining)
  // combat feedback:
  targetId?: u32,                // current attack target, for drawing tracers
  setupState?: string,           // machine_gunner/at_team/mortar_team/artillery only:
                                  // "packed","setting_up","deployed","tearing_down"
  // unit-producing buildings:
  rally?: [f32, f32],            // first rally point (world px); ONLY ever sent to the owner
  rallyPlan?: [                  // building rally stages; ONLY ever sent to the owner
    { kind: "move"|"attackMove", x: f32, y: f32 }
  ],
  // tanks:
  oilUsed?: f32,                 // lifetime oil burned by movement, in resource units
  setupFacing?: f32,             // at_team/artillery only: owner-visible deployed arc center; appended after oilUsed in compact snapshots
  orderPlan?: [                  // current + queued order stages; ONLY ever sent to the owner
    { kind: "move"|"attackMove"|"attack"|"gather"|"build"|"smoke"|"mortarFire"|"pointFire"|"setupAtGuns", x: f32, y: f32 }
  ],
  chargeCooldownLeft?: u16,      // legacy; no longer projected by current server
  abilities?: [                  // owner-only ability affordance/cooldown data
    { ability: "smoke"|"mortarFire"|"pointFire", cooldownLeft: u16,
      remainingUses?: u16, autocastEnabled?: bool }
  ],
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
{ e: "smokeLaunch", fromX: f32, fromY: f32, toX: f32, toY: f32, delayTicks: u32 }
{ e: "mortarLaunch", from: u32, fromX: f32, fromY: f32, toX: f32, toY: f32, radiusTiles: f32, delayTicks: u32 }
{ e: "mortarImpact", from?: u32, x: f32, y: f32, radiusTiles: f32,
  reveal?: { owner: u32, kind: string, x: f32, y: f32, facing?: f32, weaponFacing?: f32, setupState?: string } }
{ e: "artilleryTarget", from: u32, x: f32, y: f32, radiusTiles: f32, delayTicks: u32 }
{ e: "artilleryImpact", x: f32, y: f32, radiusTiles: f32 }
{ e: "notice", msg: string, severity?: "info"|"warn"|"alert", x?: f32, y?: f32 }
```
Notices default to `severity: "info"` with no position. `alert:`-prefixed notice ids are
gameplay alerts: the client plays alert audio and pings the minimap at `(x, y)` when present,
or pulses the minimap border when absent. `alert:under_attack` is emitted at the damaged unit's
position after normal fog/visibility filtering. Unit attack events include `reveal` so a shooter
that fires from fog can be rendered briefly as a semi-transparent, non-interactive silhouette above
the fog overlay; `toPos` lets tracers draw even when the hit target is no longer in the snapshot.
Smoke launch events are owner-visible local feedback for the scout-car canister animation; the
authoritative smoke cloud appears later in `smokes` after the reported launch delay. Mortar launch
events are sent to the firing player and to other recipients that currently see the mortar, with
shooter id, shell origin, impact point, radius, and delay so the client can draw launch dust,
recoil, the projectile, and the warning marker until detonation. Mortar impact events are sent to
the firing player, to recipients with current visibility at the impact point, and to players whose
entities were damaged by the shell. A damaged victim owner receives `from` + `reveal` so the
attacking mortar can be shown briefly above fog after indirect fire lands. Enemy players do not
receive hidden mortar launch data or hidden mortar impact markers unless their entities were hit.
Artillery target events are sent only to the firing player so enemies never receive pre-impact
markers, even if they have vision of the gun. The `from` id lets the firing client recoil the
specific gun and draw launch dust. Other players receive a visual-only `attack` event with a
shooter `reveal` when artillery fires, so the firing gun is briefly shown above fog without
revealing terrain, exploration, or the target point. Artillery impact events are sent to every
active recipient after impact as visual-only explosions; they do not reveal terrain, update
exploration, or carry entity visibility.
Events are best-effort visual flavor; the client must not depend on receiving them.

### 2.6 Replay playback state and vision

`replayState` is a reliable server message that carries the shared playback cursor/state:
```
{
  t: "replayState",
  currentTick: u32,
  durationTicks: u32,
  keyframeTicks: u32[],
  speed: f32,
  paused: bool,
  ended: bool,
  controllerId?: u32
}
```
`keyframeTicks` lists the replay keyframes the server has recorded so far. Clients may display
them as seek marks, but a seek target is not limited to these ticks; the server restores the nearest
recorded keyframe at or before the requested tick and fast-forwards from there.

`ReplayVisionRequest` selects fog/vision per viewer:
```
{ mode: "all" }
{ mode: "player", playerId: u32 }
{ mode: "players", playerIds: u32[] }
```
The server rejects unknown player ids, empty subsets, and duplicate subset ids. Vision selection is
not shared between viewers unless a later protocol explicitly adds shared-view control. Replay
snapshots are spectator-style authoritative fog snapshots from the selected real player ids; the
default is the union of all replay players.

---
