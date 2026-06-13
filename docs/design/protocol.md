## 2. Wire protocol (JSON over WebSocket)

All messages are JSON objects with a `t` field (the discriminator/tag). Field names are
short but readable. Coordinates are **world pixels** (floats) unless a field name ends in
`Tile`. The canonical Rust definitions live in `server/crates/protocol/src/lib.rs`; the
server-shell `server/src/protocol.rs` is an adapter for typed entity-kind conversion and legacy
imports. The browser mirror lives in `client/src/protocol.js` (builders + constants). Rust and JS
MUST agree on every tag, field name, and compact transport shape.

This is a pre-alpha, latest-version-only protocol. It may change incompatibly with older clients,
servers, and replay artifacts; keep the current Rust and JS mirrors synchronized instead of
carrying compatibility shims for old builds by default.

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
| `setTeamPreset` | `preset: "solo"|"ffa"|"1v2"|"1v3"|"2v2"` | Host selects a scripted lobby team preset (lobby phase only, host-only). `ffa` is the default exposed to ordinary rooms; non-FFA presets are currently for tests/dev automation, not normal lobby UI. |
| `setTeam` | `id: u32`, `teamId: u32` | Host assigns an active human or AI lobby seat to a nonzero team id (lobby phase only, host-only). Unknown ids, spectators, team id `0`, and overfull preset moves are ignored. |
| `addAi`    | `teamId?: u32` | Host adds a computer opponent to the room (lobby phase only, host-only). When `teamId` is provided it must be nonzero and fit the current preset; otherwise the server assigns the next deterministic preset seat. |
| `removeAi` | `id: u32` | Host removes a previously-added AI opponent by id (lobby phase only, host-only). |
| `setQuickstart` | `enabled: bool` | Host toggles "Debug mode" for the next match in this room. |
| `setSpectator` | `spectator: bool` | Switch between active player and spectator role while still in the lobby. Ignored after the match starts; switching to active player is ignored if the active seats are full. |
| `command`  | `clientSeq: u32`, `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. `clientSeq` is a browser-local, per-match, per-connection sequence id for prediction/reconciliation. |
| `giveUp`   | — | Give up the active match. The server eliminates that player and sends their score screen. |
| `returnToLobby` | — | Leave replay playback for this connection only. Other viewers stay in the replay; the room resets to a clean lobby only after the last viewer leaves. Ignored outside replay playback. |
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

Live player `command` messages MUST include `clientSeq`; unsequenced live commands are
protocol-invalid and are not executed. The browser resets allocation to `1` on every `start`
payload and increments monotonically for every gameplay command sent through the live transport.
`0` is reserved/invalid. The sequence range does not wrap within a match; exhausting `u32` ends
client command allocation for that match rather than reusing earlier ids. `clientSeq` belongs to
the transport envelope only and is intentionally absent from replay/simulation command DTOs.

`Command` (the `cmd` object) — `c` is the command discriminator:

| `c`          | Fields | Meaning |
|--------------|--------|---------|
| `move`       | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move selected units to a world point. Infantry ignore enemies until they arrive or receive another order; tanks and scout cars keep driving and fire at in-range enemies without chasing. When `queued` is true, store future movement intent instead of replacing the active order. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move while attacking enemies encountered; this is the aggressive movement order. When `queued` is true, store future attack-move intent instead of replacing the active order. |
| `attack`     | `units: u32[]`, `target: u32`, `queued?: bool` | Attack a specific entity. When `queued` is true, store future attack intent instead of replacing the active order. |
| `setupAtGuns` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Manually emplace owned AT guns and artillery toward a world point. When `queued` is true, append a future setup-facing intent for owned completed AT teams and artillery only; the stored point is evaluated from the unit's position when the stage promotes. Immediate setup clears movement/target state, records the setup facing, and enters `setting_up`. Other selected units are ignored. |
| `tearDownAtGuns` | `units: u32[]` | Pack up owned AT guns that are `setting_up` or `deployed`. Other selected units are ignored. |
| `charge`     | `units: u32[]` | Legacy Rifleman Charge activation. Preserved for old clients/replays, but no longer has eligible carriers. |
| `useAbility` | `ability: "charge"|"smoke"|"mortarFire"|"pointFire"|"breakthrough"`, `units: u32[]`, `x?: f32`, `y?: f32`, `queued?: bool` | Generic ability command. `charge` is legacy/no-op; `smoke`, `mortarFire`, and deployed Artillery `pointFire` target a world point. Command Car `breakthrough` is self-targeted and ignores `x`/`y`. Smoke command execution is phased separately from the authoritative smoke world-state/LOS model; mortar fire schedules a delayed area impact. Artillery point fire requires a deployed gun and is terminal in the unit order queue: once accepted, later queued unit orders are not appended after it. |
| `setAutocast` | `ability: "mortarFire"`, `units: u32[]`, `enabled: bool` | Toggle server-authoritative autocast for owned Mortar Teams. Other unit/ability combinations are ignored. |
| `gather`     | `units: u32[]`, `node: u32`, `queued?: bool` | Send workers to harvest a resource node. When `queued` is true, store future gather intent instead of replacing the active order. |
| `build`      | `units: u32[]`, `building: string`, `tileX: u32`, `tileY: u32`, `queued?: bool` | Selected workers construct a building at a tile. The server allocates one compatible worker per build click, first walks that worker to a nearby point outside the requested footprint, then starts construction once it is in range. `building` ∈ building kinds. When `queued` is true, store future build intent instead of replacing the active order. |
| `train`      | `building: u32`, `unit: string` | Queue a unit at a production building. |
| `research`   | `building: u32`, `upgrade: string` | Queue a permanent player upgrade at a tech building. Upgrade ids: `methamphetamines` at the Training Centre; `at_gun_unlock`, `artillery_unlock`, `tank_unlock`, `command_car_unlock`, and `mortar_autocast` at the R&D Complex (`research_complex`). `artillery_unlock` requires completed `at_gun_unlock`; `command_car_unlock` requires completed `tank_unlock`. |
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

Prediction acknowledgement has two milestones. Socket/room receipt means the server parsed a
sequenced command and queued it for the room; this is diagnostics-only and is not exposed as the
reconciliation acknowledgement. Sim consumption means the authoritative tick stream drained the
queued command into the simulation; snapshots expose only this milestone. Sim consumption does not
mean the command succeeded: ownership, affordability, visibility, placement, and other
authoritative validation can still make the command a no-op.

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
  headOfLineCount: u32,     // latest per-client pending-snapshot replacement count seen
  predictionMode: string,   // disabled, tracking, predicting, or resyncing
  pendingCommandCount: u16,
  acknowledgedCommandLatencyMs: u16, // latest local issue -> sim-ack latency
  correctionDistancePx: u16,         // largest correction observed by the client
  correctionCount: u32,
  predictionDisableCount: u32,
  wasmTickMs: u16,          // latest measured WASM prediction/replay work duration
  wasmMemoryBytes: u32,     // current WASM memory buffer size, when available
  predictionReplayTicks: u16 // latest local replay/advance ticks processed in one measured step
}
```
The server logs this message only when the aggregate contains notable lag, jitter, browser frame
stalls, WebSocket backlog, server tick/scheduler pressure, or prediction correction/fallback
signals, alongside the connection's `player_id` and room name. Values are advisory because clients
are untrusted; use them to diagnose transport/browser/prediction behavior, not as gameplay
authority.

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool`, `quickstart: bool`, `teamPreset: string` |
| `matchCountdown` | `durationMs: u32`, `words: string[]` — reliable pre-match countdown sent to every lobby participant after the host starts and before `start`. During this interval the server keeps the room in lobby setup, disables `canStart`, freezes lobby edits, rejects new joins, and sends `start` only after the countdown duration elapses. |
| `start`    | `Game start payload` (see 2.3). |
| `snapshot` | `Per-player snapshot` (see 2.4). |
| `replayState` | `Replay playback state` (see 2.6). |
| `replayAnalysis` | `Replay analysis state` (see 2.7). |
| `joinReplayPrompt` | `room: string` — the requested room is currently replay playback; clients should confirm before retrying `join` with `replayOk: true`. |
| `replayBranchCreated` | `branchRoom: string`, `sourceTick: u32`, `seats: ReplayBranchSeat[]` — a separate practice branch room has been created from the source replay's current authoritative tick. |
| `branchStaging` | `room: string`, `sourceTick: u32`, `hostId: u32`, `seats: BranchStagingSeat[]`, `occupants: BranchStagingOccupant[]`, `canStart: bool` — reliable current state for a replay branch staging room. Sent after joins, leaves, claims, and releases. |
| `shutdownWarning` | `deadlineUnixMs: u64`, `secondsRemaining: u64` — deploy/termination drain has started; active matches may continue until the deadline, but new match starts are disabled. |
| `gameOver` | `winnerId: u32 | null`, `winnerTeamId: u32 | null`, `you: "won" | "lost" | "draw"`, `scores: PlayerScore[]` |
| `pong`     | `ts: number` (echo of the ping ts) |
| `error`    | `msg: string` |

`LobbyPlayer`: `{ id: u32, teamId: u32, name: string, ready: bool, color: string, isAi: bool, isSpectator: bool }`. `isAi` is
true for computer opponents (always shown ready; the client renders an "AI" tag and a host-only
remove control instead of a ready toggle). `isSpectator` is true for human observers; they do not
consume active map starts, block readiness, or count toward win/loss.

`teamId` is nonzero for active match players and AI seats. `ffa` assigns each active player a
unique singleton team by default. Scripted presets assign lobby seats deterministically with the
host first when the host is active: `solo` is exactly one active player on Team 1 and does not add
or require AI opponents; `1v2` is `[1,2,2]`; `1v3` is `[1,2,2,2]`; `2v2` is `[1,1,2,2]`.
Spectator lobby rows carry `teamId: 0` because they are not match players. `canStart` is false
until the active seat count and per-team sizes match the selected preset.

`PlayerScore`: `{ id: u32, teamId: u32, name: string, color: string, unitScore: u32, structureScore: u32,
unitsKilled: u32, unitsLost: u32, buildingsKilled: u32, buildingsLost: u32 }`. `scores` is a
frozen server snapshot taken when that recipient gets `gameOver`; it is not live-updated while a
3-4 player match continues. Unit/structure score is the configured steel+oil value of every
unit/building entity created for that player, including starting entities.

`winnerTeamId` is the winning team's id when a winner exists, otherwise `null`. `winnerId` remains
for FFA compatibility. During singleton-team FFA, `winnerTeamId` matches `winnerId`; during team
wins, `winnerId` is the first living player on the winning team in stable start/lobby order.

`ReplayBranchSeat`: `{ playerId: u32, teamId: u32, name: string, color: string, claimable: bool }`. Seats are
listed in original replay player order. `claimable` is false only for unsupported original seats;
the first implementation rejects AI-seat replays before creating a branch, so successful branch
creation currently reports all seats as claimable.

`BranchStagingSeat`: `{ playerId: u32, teamId: u32, name: string, color: string, claimantId?: u32,
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
  predictionBuildId?: string,    // live active players only; server/client bundle id
  predictionVersion?: u32,       // live active players only; currently 1
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
  players: [ { id, teamId, name, color, startTileX, startTileY } ], // active match players only
}
```
Units/buildings arrive via snapshots (so they obey fog), including
the player's own starting City Centre + workers. When the lobby's `setQuickstart` toggle is
enabled, every player starts with 99,999 steel and 99,999 oil instead of the default opening
resources, and each human player also starts with five supply depots, one Gun Works
(`steelworks` kind), one R&D Complex (`research_complex` kind), one Training Centre, two Barracks,
two Vehicle Works (`factory` kind), and five of each unit kind including Command Cars. Debug mode also adds one inert enemy player in the clockwise-adjacent
corner from the first human start, with five deployed Mortar Teams clumped around one Scout Car
and four enemy Supply Depots five tiles north/east/south/west of the clump. It also sets
`debugMode: true`,
which lets the client expose local movement-waypoint overlay controls for the owner-only
`debugPath` fields in snapshots.
Spectator start payloads keep the spectator connection's `playerId`, set `spectator: true`, and
list only active match players in `players`.

For compatibility with hand-built fixtures and older replay artifacts, missing `teamId` values at
simulation/replay/test-helper boundaries default to singleton FFA: the player's own nonzero `id`.
Current live server payloads always emit explicit nonzero `teamId` values for active players.

Prediction start compatibility metadata is present only for live active players. Clients MUST keep
prediction disabled unless `predictionVersion` matches their supported prediction protocol version
and, when both sides know a build id, `predictionBuildId` matches the client bundle id. Mismatches
fall back to authoritative snapshots/tracking instead of running local visual reconciliation.

Replay start payloads include `replay` metadata so the client can display or cache a
self-describing playback session. The server validates replay artifacts before playback: artifact
schema version, server build SHA, map name, map schema version, and map content hash must match the
running server/map asset or the replay is rejected with a clear error. Saved self-play artifacts use
the same `ReplayArtifactV1` contract as post-match and match-history replays; pre-unified dev-only
artifact payloads are rejected instead of falling back to a separate loader.

When a real multi-player match ends, the server sends the normal `gameOver` score payload, clears
pending latest-only live snapshots for connected humans, and then sends a replay `start` payload
at tick 0 plus `replayState`. Post-match replay defaults every viewer to all active players'
combined authoritative vision and starts at `2.0x` speed. `returnToLobby` detaches only the
requesting replay viewer; the shared replay session remains alive for everyone else. The room drops
the replay simulation and resets to a clean lobby only after the last viewer leaves. Dedicated
replay rooms created for match-history or dev replay viewing follow the same per-viewer detach rule;
they keep the shared replay session alive until the room empties.

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
    headOfLineCount: u32,     // number of pending-snapshot replacements so far this match
    predictionVersion?: u32,  // live active players only; currently 1
    lastSimConsumedClientSeq?: u32,  // highest contiguous local clientSeq consumed by the sim
    lastSimConsumedClientTick?: u32 | null // authoritative tick that consumed that sequence
  }
}
```

Live WebSocket snapshot frames are sent as compact JSON text, version 19. `client/src/net.js`
decodes this transport shape back into the semantic object above before dispatching `S.SNAPSHOT`.
Older object-shaped JSON snapshots remain decodable by the client for fallback/dev use.

```
{
  "t": "snapshot",
  "v": 19,
  "s": [tick, steel, oil, supplyUsed, supplyCap],
  "e": [
    [
      id, owner, kind, x, y, hp, maxHp, state,
      facing?, weaponFacing?, prodKind?, prodProgress?, prodQueue?,
      buildProgress?, latchedNode?, targetId?, setupState?, remaining?, rally?, oilUsed?,
      setupFacing?, orderPlan?, chargeCooldownLeft?, abilities?, breakthroughTicks?,
      visionOnly?, debugPath?, rallyPlan?, prodUpgrade?
    ]
  ],
  "r": [[id, remaining]],         // omitted when empty
  "sm": [[id, x, y, radiusTiles, expiresIn]], // omitted when empty
  "fg": [firstValue, runLen, ...], // RLE visibleTiles; omitted when empty/no-fog
  "mb": [[id, owner, kind, x, y, [[tileX, tileY], ...], observedTick]], // rememberedBuildings; omitted when empty
  "ev": [EventRecord],            // omitted when empty
  "pr": [[id, steel, oil, supplyUsed, supplyCap]], // omitted in normal play; present in spectator/replay
  "n": [serverLagMs, tickMs, flags, slowTickCount, headOfLineCount,
        predictionVersion?, lastSimConsumedClientSeq?, lastSimConsumedClientTick?]
}
```

Compact numeric codes:

| Vocabulary | Codes |
|------------|-------|
| `kind` | 1 `worker`, 2 `rifleman`, 3 `machine_gunner`, 4 `at_team`, 5 `tank`, 6 `city_centre`, 7 `depot`, 8 `barracks`, 9 `training_centre`, 10 `factory`, 11 `steel`, 12 `oil`, 13 `steelworks`, 14 `scout_car`, 15 `mortar_team`, 16 `artillery`, 17 `research_complex`, 18 `command_car` |
| `state` | 1 `idle`, 2 `move`, 3 `attack`, 4 `gather`, 5 `build`, 6 `train`, 7 `construct`, 8 `dead` |
| `setupState` | 1 `packed`, 2 `setting_up`, 3 `deployed`, 4 `tearing_down` |
| `upgrade` | 1 `methamphetamines`, 2 `at_gun_unlock`, 3 `tank_unlock`, 4 `artillery_unlock`, 5 `mortar_autocast`, 6 `command_car_unlock` |
| `notice.severity` | 1 `info`, 2 `warn`, 3 `alert` |
| `EventRecord` | `[1, from, to]` attack, `[1, from, to, reveal?, toPos?]` attack with optional shooter reveal and target position, `[2, id, x, y, kind]` death, `[3, id, kind]` build, `[4, msg]` notice, `[4, msg, severity]` position-free notice with severity, `[4, msg, severity, x, y]` positioned notice, `[5, [fromX, fromY], [toX, toY], delayTicks]` smoke launch, `[6, x, y, radiusTiles]` mortar impact/marker, `[6, x, y, radiusTiles, from?, reveal?]` mortar impact with optional shooter reveal, `[7, from, [x, y], radiusTiles, delayTicks]` artillery target marker, `[8, x, y, radiusTiles]` artillery impact, `[9, from, [fromX, fromY], [toX, toY], radiusTiles, delayTicks]` mortar launch |

Compact entity records are positional arrays. Optional fields keep the semantic order above and
trailing missing optional fields are omitted; interior missing optional fields are encoded as
`null`. The `rally` slot is itself a two-element `[x, y]` array (or `null`).
The `orderPlan` slot is an owner-only array capped at 9 entries. It contains the current active
stage first, followed by queued unit stages in execution order. Each compact stage is
`[kind, x, y]`, where `kind` is 1 `move`, 2 `attackMove`, 3 `attack`, 4 `gather`, 5 `build`,
6 `smoke`, 7 `setupAtGuns`, 8 `charge`, 9 `mortarFire`, 10 `pointFire`, or
11 `breakthrough`.
Stages carry safe world points only, never target ids; hidden attack target stages may be omitted
rather than leaking enemy positions through fog. Production building rally points are exposed
separately through `rally` and `rallyPlan` and are not part of `orderPlan`. `rallyPlan` is appended
after `debugPath` in compact snapshots to preserve older optional slot positions; it is owner-only,
capped at four stages, and uses the same `[kind, x, y]` compact stage encoding with `move` and
`attackMove` stages.
The `abilities` slot is owner-only and capped at 8 entries. Each compact ability cooldown is
`[ability, cooldownLeft, remainingUses?, autocastEnabled?]`, where `ability` is 2 `smoke`,
3 `mortarFire`, 4 `pointFire`, or 5 `breakthrough`; 1 `charge` is legacy.
`remainingUses` is present for finite-use abilities such as Scout Car Smoke; a value of `0`
means the ability is depleted and cannot be used by that caster.
`autocastEnabled` is present for Mortar Team `mortarFire` so the command card can display and
toggle autocast without exposing enemy data.
`breakthroughTicks` is present only while the affected visible unit has active Breakthrough speed
status. Owner snapshots also expose the Command Car's `breakthrough` ability cooldown through
`abilities`.
`visionOnly` is true only for non-owned units/buildings visible through lingering death vision;
clients render them below the fog overlay and must not select or issue targeted commands against
them. In `n.flags`, bit 0 = `slowTick` and bit 1 = `headOfLine`.
The optional compact `n` prediction fields are present only for live active player snapshots.
Spectators, replay viewers, and dev full-world viewers omit prediction acknowledgement metadata.
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
  kind: string,                  // EntityKind: "worker","rifleman","machine_gunner","at_team","mortar_team","artillery","scout_car","tank","command_car","city_centre","depot","barracks","training_centre","research_complex","factory","steelworks"
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
    { ability: "smoke"|"mortarFire"|"pointFire"|"breakthrough", cooldownLeft: u16,
      remainingUses?: u16, autocastEnabled?: bool }
  ],
  breakthroughTicks?: u16,       // active Breakthrough speed status; visible only with the entity
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
or pulses the minimap border when absent. `alert:under_attack` is emitted at the damaged enemy
unit's position to the victim owner and same-team recipients that pass the normal event visibility
filter. Same-team friendly-fire damage does not emit under-attack alerts. Unit attack events include
`reveal` so a shooter
that fires from fog can be rendered briefly as a semi-transparent, non-interactive silhouette above
the fog overlay; `toPos` lets tracers draw even when the hit target is no longer in the snapshot.
Smoke launch events are owner-visible local feedback for the scout-car canister animation; the
authoritative smoke cloud appears later in `smokes` after the reported launch delay. Mortar launch
events are always sent to the firing player, with shooter id, shell origin, impact point, radius,
and delay so the client can draw launch dust, recoil, the projectile, and the warning marker until
detonation. Autocast mortar launch events are also sent to other recipients that currently see the
mortar; manual launch events are owner-only so enemy clients do not receive the pre-impact warning
marker. Mortar impact events are sent to the firing player, to recipients with current visibility
at the impact point, and to enemy players whose entities were damaged by the shell. An enemy damaged
victim owner receives `from` + `reveal` so the attacking mortar can be shown briefly above fog after
indirect fire lands. Allied or owned entities can still take mortar splash damage, but that damage
is unattributed and does not reveal the firing mortar as hostile. Enemy players do not receive
hidden mortar launch data or hidden mortar impact markers unless their entities were hit.
Artillery target events are sent only to the firing player so enemies never receive pre-impact
markers, even if they have vision of the gun. The `from` id lets the firing client recoil the
specific gun and draw launch dust. Other players receive a visual-only `attack` event with a
shooter `reveal` when artillery fires, so the firing gun is briefly shown above fog without
revealing terrain, exploration, or the target point. Artillery impact events are sent to every
active recipient after impact as visual-only explosions; they do not reveal terrain, update
exploration, or carry entity visibility. Artillery impact damage follows the same support-fire
friendly-fire attribution rule as mortar splash: owned and allied entities in the radius can take
damage, but same-team damage does not produce hostile reveal, under-attack, or score attribution.
Events are best-effort visual flavor; the client must not depend on receiving them.

### 2.6 Replay playback state and vision

`replayState` is a reliable server message that carries the shared playback cursor/state. Replay
rooms send it for playback cursor changes; dev scenario watch rooms also send it after pause/resume
and one-tick step controls so clients can confirm the authoritative dev-watch speed and tick:
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

### 2.7 Replay analysis state

`replayAnalysis` is a replay-only reliable server message for overlay/tab data that cannot be
derived safely from the browser's current projected snapshot. It is sent to replay viewers after
replay `start`/`replayState`, after accepted seeks, after replay vision changes, and during replay
playback ticks. It is not sent during live matches or dev scenario watch rooms.
```
{
  t: "replayAnalysis",
  tick: u32,
  players: [
    {
      id: u32,
      units: [{ kind: string, count: u32, steelValue: u32, oilValue: u32 }],
      production: [
        {
          buildingId: u32,
          buildingKind: string,
          itemKind: string,
          itemType: "unit" | "upgrade",
          progress: f32,
          queueDepth: u32
        }
      ],
      unitsLost: [{ kind: string, count: u32, steelValue: u32, oilValue: u32 }],
      resourcesLost: { steel: u32, oil: u32 }
    }
  ]
}
```
`players` lists every active replay player. `units` is the current living unit inventory by kind.
`production` has one row for each owned building with a non-empty unit or research queue; `progress`
is the front item's completion fraction and `queueDepth` is that queue's total item count.
`steelValue` and `oilValue` are aggregate row values (`count * configured cost`), not per-unit
costs. `unitsLost` is the authoritative unit-death count by kind. `resourcesLost` is intentionally
narrow: the spent steel/oil value of units that died, matching `unitsLost`; it does not include
buildings, current spending, cancelled production, refunds, harvesting, or stockpile deltas.

Replay analysis uses an all-player spectator policy independent of each viewer's replay vision
selection. It is replay-only data for analysis overlays, not a live-player information surface.
The server recomputes the payload from the current authoritative replay `Game` state after normal
playback ticks and after `ReplaySession::rebuild_to()` restores a keyframe and fast-forwards to the
target tick. Analysis state is not serialized separately in `ReplayKeyframe`.

---
