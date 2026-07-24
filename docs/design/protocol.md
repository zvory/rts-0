## 2. Wire protocol (JSON and binary snapshots over WebSocket)

Client-to-server messages and reliable server-to-client messages are JSON objects with a `t` field
(the discriminator/tag). Live snapshot messages are MessagePack compact binary snapshot frames over
the same WebSocket. Field names are short but readable. Coordinates are **world pixels** (floats)
unless a field name ends in `Tile`. The canonical Rust definitions live in
`server/crates/protocol/src/lib.rs`; the server-shell `server/src/protocol.rs` is an adapter for
typed entity-kind conversion and legacy imports. Protocol constants, compact code tables, and the
structured contract dump are factored into `server/crates/protocol/src/contract_metadata.rs`; the
compact snapshot serializer is factored into `server/crates/protocol/src/compact_snapshot.rs`; both
are re-exported through `lib.rs`. The browser mirror lives in `client/src/protocol.js` (builders +
decode entry points), with constants in `client/src/protocol_constants.js`, frame transport
internals in `client/src/protocol_frame.js`, and compact snapshot decoding in
`client/src/protocol_snapshot.js`. Rust and JS MUST agree on every tag, field name, and compact
transport shape.

This is a pre-alpha, latest-version-only protocol. It may change incompatibly with older clients,
servers, and replay artifacts; keep the current Rust and JS mirrors synchronized instead of
carrying compatibility shims for old builds by default.

`rts-protocol` may depend on `rts-contract` but must not depend on `rts-sim`, `rts-rules`,
`rts-ai`, or `rts-server`. Domain kind conversion that needs `EntityKind` belongs in an adapter
layer such as `server/src/protocol.rs` or `server/crates/sim/src/protocol.rs`, not in the wire DTO
crate.

### 2.0 Boundary authority and guardrails

`server/crates/protocol/src/lib.rs` owns the wire DTO vocabulary, with protocol constants, message
tags, compact code tables, compact slot schemas, `COMPACT_SNAPSHOT_VERSION`,
`PREDICTION_PROTOCOL_VERSION`, and the unknown compact-code sentinel (`255`) housed in
`server/crates/protocol/src/contract_metadata.rs` and re-exported from `lib.rs`. MessagePack
frame-writing internals live in
`server/crates/protocol/src/messagepack_frame.rs`; compact snapshot semantic serialization lives in
`server/crates/protocol/src/compact_snapshot.rs`; both stay behind the public frame and serializer
helpers re-exported by `lib.rs`. `server/crates/contract/src/lib.rs` owns shared semantic DTOs that
the protocol crate re-exports, including start/snapshot contract records and `DEFAULT_FACTION_ID`.
`rts-rules::EntityKind::stable_id()` owns entity domain identity strings, while
`rts-rules::faction::{AbilityKind, UpgradeKind}` own typed gameplay identities and their stable
strings. Because crate direction forbids `rts-protocol` from depending on `rts-rules`, protocol
ability/upgrade constants and compact code tables remain an intentional wire mirror. Focused
rules/sim adapter tests require stable-id agreement and a non-unknown compact code for every typed
kind; browser parity separately checks that wire mirror against JavaScript.

`client/src/protocol.js` is the browser mirror and stable public import surface for protocol
vocabulary, compact decode tables, message builders, and decode helpers. Internal browser constants
and compact-code maps live in `client/src/protocol_constants.js`, while callers should continue
importing through `client/src/protocol.js`. Binary frame parsing lives in
`client/src/protocol_frame.js`; compact snapshot decoding lives in
`client/src/protocol_snapshot.js`. Future internal browser splits may live under
`client/src/protocol_*.js` or `client/src/protocol/**`. Protocol changes must update Rust DTOs or
dumps, the JS mirror, this design file, and focused parity coverage in the same commit. Compact slot
order is append-only unless the compact snapshot version is intentionally bumped and the Rust
serializer, JS decoder, parity fixture, and docs change together.

Run `node tests/protocol_parity.mjs` after any protocol vocabulary, compact code, compact slot,
start/snapshot/replay DTO, prediction metadata, default faction id, or lobby color palette change.
That check compares the structured Rust protocol contract dump to the JS mirror, rejects duplicate
or sentinel compact codes, decodes representative compact fixtures, and also checks
`PLAYER_PALETTE` against `server/src/lobby/mod.rs`. The palette is not a wire-protocol constant,
but the server assigns lobby/start colors and the client keeps a fallback/render mirror, so the
cross-surface guard intentionally lives with the protocol parity smoke test until a structured
lobby/config dump replaces the source scrape.

### 2.1 Client → Server (`ClientMessage`)

| `t`        | Fields | Meaning |
|------------|--------|---------|
| `join`     | `name: string`, `room?: string`, `spectator?: bool`, `replayOk?: bool` | Join (or create) a room as an active lobby player or, when `spectator` is true, as an observer. `room` defaults to `"main"`. Normal live matches accept `spectator: true` after match start and attach the connection as a gameplay-read-only live spectator with shared live pause controls; active late joins and countdown joins are rejected. Lobby role switches are observer-only and must happen before match start. Persisted match-history replay rooms start as `kind: "replay"` staging lobbies; joins there are accepted as spectators only and do not require `replayOk`. Active replay playback remains listed in the lobby browser with `kind: "replay"`; its explicit **Join replay** action sends `replayOk: true` and attaches the viewer at the room's current shared tick. Other attempts to enter replay playback without `replayOk` receive `joinReplayPrompt` and may retry after confirmation. If the same WebSocket is already in a different room and the new room accepts the join, the connection transfers to the new room and leaves the previous room. |
| `setName`  | `name: string` | Update the sender's display name while still in the lobby. The server trims and bounds the value with the same rules as `join`, broadcasts the updated roster, and ignores the request during countdown or after match start. |
| `ready`    | `ready: bool` | Toggle ready state in the lobby. |
| `matchLoadReady` | `countdownId: u32` | Confirm that this client's renderer and required assets finished warming for the exact active multiplayer countdown. Stale ids, spectators, and requests outside countdown are ignored. |
| `start`    | — | Host asks to start the match (only honored from the room host). In a persisted replay staging lobby, host `start` begins replay playback immediately when at least one spectator is present and deploy drain is not blocking new sessions; ready/team/map/AI checks do not apply. |
| `setTeamPreset` | `preset: string` | Deprecated compatibility command. The server ignores it; lobby teams are host-managed slots. |
| `setTeam` | `id: u32`, `teamId: u32` | Host assigns an active human or AI lobby seat to team `1..=4` (lobby phase only, host-only). Unknown ids, spectators, team id `0`, and team ids outside the supported range are ignored. |
| `setFaction` | `factionId: string` | Active human players select their own playable lobby faction (lobby phase only). Unknown ids, fixture ids, spectators, countdown, and in-game requests are ignored. The normal client only exposes this during the beta UI rollout. |
| `addAi`    | `teamId?: u32`, `aiProfileId?: string` | Host adds a computer opponent to the room (lobby phase only, host-only). When `teamId` is provided it must be in `1..=4`; otherwise the server assigns the first empty team slot. Player lobbies support only `ai_2_1`; omitted, unknown, or internal-only profile IDs default to `ai_2_1`. Observer-only internal sessions may request `ai_turtle`. |
| `setAiProfile` | `id: u32`, `aiProfileId: string` | Host selects a supported profile for an existing AI lobby seat (lobby phase only, host-only). Player lobbies support only `ai_2_1`; unknown AI ids and unsupported or internal-only profile ids are ignored. Observer-only internal sessions may select `ai_turtle`. Any internal profile is replaced with `ai_2_1` if a human takes an active seat and again at match start. |
| `removeAi` | `id: u32` | Host removes a previously-added AI opponent by id (lobby phase only, host-only). |
| `setSpectator` | `spectator: bool`, `id?: u32` | Switch between active player and spectator role while still in the lobby. When `id` is omitted, the sender switches their own role. The host may include another connected human player's id to move that lobby player into or out of spectators; non-host targeted requests, AI ids, and unknown ids are ignored. Ignored after the match starts; switching to active player is ignored if the active seats are full. |
| `command`  | `clientSeq: u32`, `cmd: Command` | Issue a gameplay command (see below). Ignored unless in-game. `clientSeq` is a browser-local, per-match, per-connection sequence id for prediction/reconciliation and diagnostics-only command receipts. |
| `giveUp`   | — | Give up the active match. The server eliminates that player and sends their score screen. |
| `pauseGame` | — | Pause a live match. Honored only from live recipients with `matchControls.pause` while the room is unpaused and that active seat or spectator connection has successful pause starts remaining. |
| `unpauseGame` | — | Resume a paused live match. Honored from any live recipient with `matchControls.pause` while the room is paused. |
| `returnToLobby` | — | Leave replay playback for this connection only. Other viewers stay in the replay; the room resets to a clean lobby only after the last viewer leaves. Ignored outside replay playback. |
| `ping`     | `ts: number` | Latency probe; server replies with `pong`. |
| `netReport` | `report: ClientNetReport` | Periodic client-observed network/render health aggregate. Server logs notable reports for diagnostics only; it never affects simulation state. |
| `activity` | — | Throttled notice that the connected browser received human pointer, keyboard, wheel, or foregrounding input. It extends the server's player-inactivity deadline but does not mutate room state. Automatic heartbeat and diagnostics traffic never sends this message. |
| `setRoomTimeSpeed` | `speed: f32` | Set the room-controlled time speed where the current room-time capability profile allows speed control. `0` pauses replay playback, speed-only live-game rooms, dev scenario watch rooms, and lab rooms; other accepted speeds are clamped. Ignored in fixed-realtime rooms. |
| `stepRoomTime` | — | Advance room-controlled time by one authoritative simulation tick where the current room clock capability allows stepping. Currently accepted only in paused dev scenario watch rooms and paused lab rooms. |
| `seekRoomTime` | `ticksBack: u32` | Rewind room-controlled time by N simulation ticks where the current room clock capability allows relative seek; pass a large value (e.g. `2^31-1`) to reset to tick 0. Currently accepted in replay and lab rooms. |
| `seekRoomTimeTo` | `tick: u32` | Seek room-controlled time to an absolute simulation tick where the current room clock capability allows absolute seek. Replay rooms clamp to duration, rate-limit accepted seeks, restore the nearest recorded replay keyframe at or before the target tick, fast-forward the remaining ticks, re-send `start`, and emit `roomTimeState`. Lab rooms use room-local keyframes and recorded accepted lab operations/issue-as commands the same way, then re-send lab `start`, `roomTimeState`, `labState`, and a fresh snapshot. Replay and lab keyframes are recorded every 2,000 ticks while their authoritative time advances. |
| `setVisionSelection` | `selection: VisionSelectionRequest` | Select replay fog/vision for this viewer only. Ignored outside replay rooms. The server validates the request and applies it to that viewer's subsequent snapshot projection. |
| `lab` | `requestId: u32`, `op: LabClientOp` | Privileged lab request envelope. `requestId` must be nonzero. Ignored before join; rejected outside lab rooms and from non-operator roles with `labResult`. Accepted setup mutations and issue-as commands are room-local; `setVision` changes only the requesting operator's projection. Accepted lab requests append to the lab operation log with the requesting connection id. |
| `requestBranchFromTick` | — | Request creation of a new practice branch room from this replay room's current authoritative server tick. Ignored before join; rejected outside replay playback. The server rejects replays with AI seats in the first implementation and returns `error`. On success, the source replay room broadcasts `branchFromTickCreated` to all current viewers. |
| `claimBranchSeat` | `playerId: u32` | Claim one original replay player seat in a replay branch staging room. Ignored outside branch staging. Rejected with `error` if the seat is unknown, already claimed, or this occupant already claimed another seat. |
| `releaseBranchSeat` | `playerId: u32` | Release one original replay player seat currently claimed by this occupant in branch staging. Ignored outside branch staging or when the occupant does not own that claim. |
| `startBranch` | — | Host asks to launch the staged replay branch. Ignored outside branch staging and from non-hosts. The server rejects launch until every original active seat is claimed; live promotion is handled by the branch promotion phase. |
| `selectMap` | `map: string` | Host selects the lobby map by its stable map name. Ignored outside the lobby, from non-hosts, during match countdown, in dev-watch rooms, or in replay staging lobbies. The server broadcasts the selected value as lobby `map` and the available catalog as `maps[]`. |

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
| `formationMove` | `units: u32[]`, `points: {x:f32,y:f32}[]`, `attackMove?: bool`, `queued?: bool` | Move selected units into a freehand formation along a world-space polyline. `attackMove` gives every resolved unit goal attack-move semantics; absent/false preserves ordinary move behavior. The submitted stroke must contain 2–64 finite points; the server clamps points to map bounds and collapses near-duplicate neighbors. Long strokes distribute one rank across the full arc length, while short strokes create parallel ranks. Legal final goals remain authoritative and may shift from the stroke for body standability, uniqueness, terrain, reachability, or known-trench preference. Queued formation moves store one resolved move or attack-move point intent per accepted unit, not the polyline. |
| `attackMove` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Move toward a world point while engaging enemies that enter current weapon range. The unit may pause to fire, then resumes the player-issued destination; enemy sightings never replace that path with pursuit movement. When `queued` is true, store future attack-move intent instead of replacing the active order. |
| `attack`     | `units: u32[]`, `target: u32`, `queued?: bool` | Direct-target attack. The unit keeps this target locked, pursues it until it reaches its current weapon range band, then stops and fires; if the target moves back out of range, it resumes pursuit. It does not switch to a different target while this commanded target remains legal and visible. Explicit attack commands may target an enemy entity or an entity owned by the issuing player, allowing deliberate self-attacks on stuck friendly units/buildings. Allied teammates' entities are not valid self-attack targets, and automatic attack-move/idle acquisition remains enemy-only. When `queued` is true, store future attack intent instead of replacing the active order. |
| `deconstruct` | `units: u32[]`, `target: u32`, `queued?: bool` | Send one selected worker to deconstruct a completed Tank Trap. The target may be friendly, allied, or enemy; enemy traps must be visible when the command is accepted or when a queued stage promotes. Deconstruction uses half of the Tank Trap build time (150 ticks / 5 seconds with current balance), cannot be sped up by multiple workers on the same trap, and refunds the Tank Trap cost to the deconstructing player's economy. When `queued` is true, store one future deconstruct intent using the same selected-worker allocation policy as build orders. |
| `setupAntiTankGuns` | `units: u32[]`, `x: f32`, `y: f32`, `queued?: bool` | Manually emplace owned Anti-Tank Guns, Mortar Teams, and Artillery. Anti-Tank Guns and Artillery face the submitted world point. Mortar Teams retain their current facing, so the client issues setup directly from the command card without a target click. When `queued` is true, append a future setup intent for owned completed setup-capable support weapons only; queued Mortar Team setup is terminal, while the stored facing point for other weapons is evaluated from the unit's position when the stage promotes. Immediate setup clears movement/target state and begins after alignment. Other selected units are ignored. |
| `tearDownAntiTankGuns` | `units: u32[]` | Pack up owned Anti-Tank Guns, Mortar Teams, and Artillery that are `setting_up` or `deployed`, or cancel their staged setup facing while packed. Other selected units are ignored. |
| `charge`     | `units: u32[]` | Legacy Rifleman Charge activation. Preserved for old clients/replays as a parseable no-op; it has no eligible carriers, cooldown, or runtime status. |
| `artilleryFire` | `units: u32[]`, `x: f32`, `y: f32`, `radiusTiles: f32`, `queued?: bool` | Unified Artillery Fire command. `x`/`y` is the selected center and `radiusTiles` is the click-click or press-drag-release radius. The server clamps it to 6–15 tiles normally or 3–15 after Artillery Fire Control. Each gun locks the center to its 25-to-55-tile range band, can set up or redeploy in place, and stores the selected radius in its terminal fire order. Every shell lands at a uniformly area-sampled point inside that circle; there is no additional accuracy error or repeated-shot tightening. |
| `useAbility` | `ability: "charge"|"smoke"|"mortarFire"|"pointFire"|"blanketFire"|"breakthrough"|"scoutPlane"|"dismissScoutPlane"|"ekatTeleport"|"ekatLineShot"|"ekatMagicAnchor"|"ekatConsumeGolem"`, `units: u32[]`, `x?: f32`, `y?: f32`, `queued?: bool` | Generic ability command. Ability ids, carriers, target mode, cost, cooldown, finite uses, queueability, queue policy, autocast support, and command-card exposure are mirrored from the Rust faction ability registry. `charge` is legacy/no-op with no carriers or runtime effect; `dismissScoutPlane` is reserved legacy vocabulary with no carriers or command-card descriptor. `smoke`, `scoutPlane`, `mortarFire`, Ekat `ekatTeleport`, `ekatLineShot`, and `ekatMagicAnchor` target a world point. Command Car `breakthrough` and Ekat `ekatConsumeGolem` are self-targeted and ignore `x`/`y`. Command Car `scoutPlane` spends 50 steel and 75 oil, launches a Scout Plane from a selected ready Command Car without a City Centre requirement, starts that Command Car's 30-second cooldown, and gives the independent sortie a 20-second total lifetime from launch; transit consumes that lifetime, the plane orbits only for any time remaining after arrival, and it despawns on expiry even if it never reaches the clicked point. Any number of valid sorties may coexist, and every active plane contributes its own aerial vision. Smoke command execution is phased separately from the authoritative smoke world-state/LOS model; mortar fire schedules a delayed area impact. Registry queue policy differentiates non-queued, skip-if-not-ready, and wait-until-ready abilities; queued Mortar Fire waits for ability cooldown/weapon reload and then fires once per queued click. Legacy Artillery `pointFire` and `blanketFire` ids remain decodable for older commands and replays, but the live command card emits `artilleryFire`. Ekat dash and line shot clamp out-of-range world targets to their max range instead of walking Ekat into range; queued Ekat ability commands append future dash, line shot, or Magic Anchor intents. Ekat dash moves her within the target range if the landing point is statically standable and leaves an authoritative return marker at the original position; Ekat line shot spawns a projected out-and-back line projectile that damages enemy targetables on each swept leg, and an active Magic Anchor adds a second projectile from the anchor toward the same point; Magic Anchor places one replacement-style, non-blocking, non-attackable 10-second pull field. Ekat Consume is non-queued; it permanently removes the nearest owned living Golem within 2 tiles and heals Ekat to full HP. |
| `recastAbility` | `ability: "ekatTeleport"`, `units: u32[]`, `targetObjectId?: u32`, `queued?: bool` | Explicit second activation for an existing per-caster ability state. The server does not infer recast from missing `x`/`y`; it validates ownership, live caster eligibility, matching active return marker state, the no-instant-return availability tick, and destination standability, then returns Ekat to the marker and consumes it. |
| `setAutocast` | `ability: "mortarFire"`, `units: u32[]`, `enabled: bool` | Toggle server-authoritative autocast for owned Mortar Teams. Other unit/ability combinations are ignored. |
| `gather`     | `units: u32[]`, `node: u32`, `queued?: bool` | Send gather-capable units to harvest a direct-mineable resource node. Oil nodes are not direct-mineable; workers extract oil by building Pump Jacks with the `build` command. When `queued` is true, store future gather intent instead of replacing the active order. |
| `build`      | `units: u32[]`, `building: string`, `tileX: u32`, `tileY: u32`, `queued?: bool` | Selected workers construct a building at a tile. The server allocates one compatible worker per build click, first walks that worker to a nearby point outside the requested footprint, then starts construction once it is in range. `building` ∈ building kinds. `pump_jack` is a contextual worker build that is valid only when its footprint overlaps a live oil node; when its builder arrives, owned and allied units on that footprint are moved to nearby clear positions before placement is revalidated, while enemy units remain blockers. When `queued` is true, store future build intent instead of replacing the active order. |
| `train`      | `building: u32`, `unit: string` | Queue a manual unit at a production building. A legal manual request is accepted without current Steel, Oil, or supply; an unpaid front item waits at zero progress until it can pay and reserve supply. Later FIFO items do not prepay. Standing repeat production remains separate and creates no queue item until it can pay. |
| `adjustProductionRepeat` | `buildings: u32[]`, `unit: string`, `delta: -1\|1` | Atomically adjust `unit` across the selected owned completed compatible producers. `+1` enables it on one producer that does not already repeat it, preferring the producer with the fewest standing repeat units; `-1` removes it from one producer that repeats it, preferring the producer with the most standing repeat units so another automatic order survives when possible. Stable opposite entity-id tie-breaks make repeated additions followed by removals reversible. Other delta values are ignored. When a producer's ordinary unit queue is empty, it silently retries the ordered list's current entry. Each successful automatic enqueue advances to the next enabled unit, so two active units alternate. Once inserted, a repeated unit is an ordinary FIFO item, so later manual production queues behind it. Any production cancel clears the affected producer's whole repeat list. |
| `research`   | `building: u32`, `upgrade: string` | Queue a permanent player upgrade at a tech building. A legal manual request may wait unpaid at zero progress until its cost is available. A prerequisite must either be complete or already appear earlier in the same building's authoritative FIFO research queue. Current Kriegsia upgrade ids: `methamphetamines`, `panzerfausts`, and `entrenchment` at the Training Centre; `anti_tank_gun_unlock` (AT Guns), `artillery_unlock` (Artillery), `ballistic_tables` (Artillery Fire Control), `tank_unlock`, `mortar_autocast`, and `smoke_plus` at the R&D Complex (`research_complex`). `panzerfausts` unlocks Panzerfaust training at the Barracks without changing Riflemen. `anti_tank_gun_unlock` unlocks Anti-Tank Gun training; `artillery_unlock` requires `anti_tank_gun_unlock` and unlocks Artillery training; `ballistic_tables` requires `artillery_unlock` and reduces the Artillery Fire minimum radius from 6 tiles to 3; `tank_unlock` unlocks Tank and Command Car training. `smoke_plus` doubles Scout Car Smoke radius and duration. |
| `cancel`     | `building: u32`, `construction?: bool` | With `construction: true`, cancel an owned unfinished building for a full construction-cost refund; otherwise cancel the latest item in a completed building's production queue. The explicit construction scope prevents a delayed scaffold action from cancelling production after the building completes. Attached builders return to ordinary order handling when a construction site is canceled. |
| `stop`       | `units: u32[]` | Clear orders and return selected units to ordinary idle behavior. |
| `holdPosition` | `units: u32[]`, `queued?: bool` | Stand ground. Without `queued`, clear active and queued unit orders immediately. With `queued: true`, append a terminal hold-position intent so units complete earlier queued stages before standing ground. Held units still fire at enemies already in weapon range and can still be pushed by collision resolution. |
| `setRally`   | `building: u32`, `x: f32`, `y: f32`, `kind?: "move"|"attackMove"`, `queued?: bool` | Set or append a unit-producing building rally stage. `kind` defaults to `"move"` on the wire. Freshly produced units receive the building's rally plan as active + queued orders; plain move rally stages become attack-move orders for ordinary combat units, while faction gatherers keep plain move orders. The building prefers the spawn exit nearest the first stage. Ignored for buildings the player doesn't own, non-producers (depot, training centre, research_complex), or buildings still under construction. Points are clamped into map bounds. When `queued` is true, append until the four-stage building rally cap is reached; otherwise replace the whole rally plan. |

Servers MUST ignore commands referencing commanded units/buildings the player does not own, unknown
ids, illegal placements, or unaffordable immediate actions (fail silently or emit a `notice` event).
Manual build, train, and research commands are the explicit affordability exceptions documented
above: they retain bounded authoritative intent and wait before spending or progressing. Attack
target ids are separately validated by explicit attack-target rules.
For appendable unit commands, omitted `queued` is equivalent to `false`. Unit order queues are
capped at 8 intents per unit. Queued intents are lightweight future intent only; active `Order`
remains the per-tick execution state. Non-queued unit orders replace the active order and clear
future unit intents; `stop` and non-queued `holdPosition` clear both active and queued unit orders.
Queued `holdPosition` is terminal, so later queued unit orders are not appended after it.
Production building rally plans are capped at four total stages. A non-queued rally replaces the
whole plan; a queued rally appends if space remains and establishes the first stage when the plan is
empty.

Prediction acknowledgement has two milestones. Socket/room receipt means the server parsed a
sequenced command and the room task accepted or rejected the envelope; the server may send a tiny
`commandReceipt` containing only `clientSeq`, `serverTick`, `accepted`, and optional stable `reason`.
This is diagnostics-only and is not the reconciliation acknowledgement. Sim consumption means the
authoritative tick stream drained the queued command into the simulation; snapshots expose only this
milestone. Sim consumption does not mean the command succeeded: ownership, affordability,
visibility, placement, and other authoritative validation can still make the command a no-op.

`ClientNetReport` is an untrusted, rate-limited diagnostic aggregate emitted by the browser while
in a match:
```
{
  schemaVersion: u8,        // currently 1
  matchRunId: string,       // live match correlation id from start payload; empty when absent
  elapsedMs: u32,           // client-side aggregation window duration
  matchTick: u32,           // latest snapshot tick observed by this client
  rttMs: u16,               // latest app-level ping round-trip sample
  rttMaxMs: u16,            // max round-trip sample in this report window
  badRttSamples: u32,       // samples at/above the client's latency warning threshold
  snapshotJitterMs: u16,    // current max receive jitter over the client's short jitter window
  snapshotGapMaxMs: u16,    // largest observed interval between received snapshots
  jitterSamples: u32,       // jitter incidents in this report window
  snapshots: u32,           // snapshots received in this report window
  snapshotLateFrameCount: u32, // frames where the latest snapshot was late by jitter threshold
  predictedSnapshotLateFrameCount: u32, // late-snapshot frames with owned predicted overlay present
  predictedSnapshotLateFramePctX100: u16, // predicted/late coverage percentage multiplied by 100
  predictionActiveLateFrameCount: u32, // late-snapshot frames where local prediction was predicting/resyncing
  snapshotBytesTotal: u32,   // total received snapshot application payload bytes
  snapshotBytesMax: u32,     // largest received snapshot application payload bytes
  snapshotBytesAvg: u32,     // average received snapshot application payload bytes
  snapshotMessageCount: u32, // snapshot frames observed by the transport report window
  snapshotByteSource: string, // currently "messagepack-application-payload"; not compressed wire bytes
  snapshotCodec: string,      // currently "messagepack-compact"
  snapshotCodecVersion: u16,  // currently 1
  snapshotFrameKind: string,  // currently "binary"
  snapshotBytesP95: u32,     // bucketed p95 received snapshot application payload bytes
  snapshotSegmentBudgetBytes: u32, // payload-byte single-segment budget used by this client
  snapshotOverSegmentBudgetCount: u32, // snapshot frames above snapshotSegmentBudgetBytes
  snapshotOverSegmentBudgetPctX100: u16, // over-budget percentage multiplied by 100
  snapshotParseMaxMs: u16,   // max browser frame parse cost for snapshot frames
  snapshotParseP95Ms: u16,   // bucketed p95 frame parse cost for snapshot frames
  snapshotDecodeMaxMs: u16,  // max compact protocol decode cost
  snapshotDecodeP95Ms: u16,  // bucketed p95 compact protocol decode cost
  websocketExtensions: string, // bounded browser WebSocket.extensions after open
  websocketCompression: string, // normalized "permessage-deflate" or "none"
  snapshotApplyMaxMs: u16,   // max GameState.applySnapshot cost
  snapshotApplyP95Ms: u16,   // bucketed p95 GameState.applySnapshot cost
  predictionApplyMaxMs: u16, // max authoritative prediction reconciliation/overlay cost
  predictionApplyP95Ms: u16, // bucketed p95 authoritative prediction reconciliation/overlay cost
  snapshotTickGapMax: u32,   // largest authoritative tick delta between received snapshots
  staleSnapshotCount: u32,   // snapshots older than the latest accepted snapshot tick
  duplicateSnapshotCount: u32, // snapshots with the same tick as the latest accepted snapshot
  skippedSnapshotCount: u32, // snapshots whose tick jumped by more than one
  snapshotBurstCount: u32,   // frames where more than one snapshot arrived before the next rAF
  snapshotBurstMax: u32,     // max snapshots received before one rAF boundary
  frameGapMaxMs: u16,       // largest requestAnimationFrame gap in this report window
  fpsEstimate: u16,         // coarse average client frame rate for this report window
  frameWorkMaxMs: u16,      // largest measured JS frame work duration
  frameWorkP95Ms: u16,      // bucketed p95 measured JS frame work duration
  frameRafDispatchMaxMs: u16, // max RAF callback dispatch delay before JS frame work
  frameRafDispatchP95Ms: u16,
  frameUnattributedMaxMs: u16, // max frame work not covered by top-level match.* phases
  frameUnattributedP95Ms: u16,
  slowFrameCount: u32,      // frames whose gap or work crossed the slow-frame threshold
  frameWorkBudgetMissCount: u32, // frames whose complete JS work exceeded 1000/60 ms
  presentBudgetMissCount: u32, // actual presents whose synchronous work exceeded 1000/60 ms
  worstFramePhase: string,  // bounded profiler label most often worst in this report window
  worstFramePhaseMs: u16,   // max duration for worstFramePhase
  rendererMaxMs: u16,       // largest measured match.renderer duration
  rendererP95Ms: u16,       // bucketed p95 match.renderer duration
  rendererUpdateMaxMs: u16, // max backend scene translation/update duration
  rendererUpdateP95Ms: u16, // bucketed p95 backend scene translation/update duration
  rendererPresentMaxMs: u16, // max synchronous Pixi/Babylon present duration
  rendererPresentP95Ms: u16, // bucketed p95 synchronous Pixi/Babylon present duration
  topRendererPhase: string, // allowlisted top renderer.* phase label for this report window
  topRendererPhaseMs: u16,
  topRenderDiagnosticGroup: string, // allowlisted grouped render/minimap/HUD diagnostic counter
  topRenderDiagnosticGroupCount: u32,
  clientFramePhases: [{ label: string, count: u32, maxMs: u16, p95Ms: u16 }], // top 5 allowlisted frame phases
  rendererFramePhases: [{ label: string, count: u32, maxMs: u16, p95Ms: u16 }], // top 5 allowlisted renderer.* phases
  renderDiagnosticCounters: [{ label: string, samples: u32, frames: u32, total: u32, maxFrame: u32 }], // top 5 grouped counters
  renderWorkerMode: string, // active worker presentation route
  renderWorkerSubmitted: u32, // cumulative host frame submissions
  renderWorkerPresented: u32, // cumulative acknowledged presentations
  renderWorkerFailureCount: u32, // new terminal worker failures since the prior report
  renderWorkerContextLostCount: u32, // new WebGL context-loss failures since the prior report
  renderWorkerInFlight: bool,
  renderWorkerInFlightFrameId: u32,
  renderWorkerInFlightAgeMs: u32, // age of the unacknowledged in-flight frame
  renderWorkerPending: bool,
  renderWorkerPendingFrameId: u32,
  renderWorkerLastPresentedFrameId: u32,
  renderWorkerLastPresentedAgeMs: u32,
  renderWorkerLastMessageAgeMs: u32,
  renderWorkerErrorCode: string, // bounded stable worker/host failure category
  renderWorkerErrorMessage: string, // bounded terminal failure detail
  renderWorkerErrorStack: string, // bounded worker/host stack with control characters flattened
  renderWorkerErrorSource: string, // bounded worker error-event source when available
  renderWorkerErrorLine: u32,
  renderWorkerErrorColumn: u32,
  renderWorkerBackend: string,
  renderWorkerPixiVersion: string,
  renderWorkerGlVendor: string,
  renderWorkerGlRenderer: string,
  renderWorkerGlVersion: string,
  renderWorkerUserAgent: string,
  entityCount: u32,         // latest client-visible entity count context
  selectedCount: u16,       // latest local selection size context
  visibleTileCount: u32,    // latest visible-tile count context
  viewportWidth: u16,       // latest CSS viewport width context
  viewportHeight: u16,      // latest CSS viewport height context
  devicePixelRatioX100: u16, // latest devicePixelRatio multiplied by 100
  commandBurstBucketMs: u16, // short command-density bucket width, currently 250 ms
  commandBurstMax: u16,      // max commands issued in any commandBurstBucketMs window
  commandBurstFrameGapMaxMs: u16, // max frame gap while a command burst was active
  commandBurstWorstFramePhase: string, // bounded worst frame phase while a command burst was active
  commandBurstWorstFramePhaseMs: u16,
  hidden: bool,             // document.hidden when the report was sent
  focused: bool,            // document.hasFocus() when available
  desktopRuntimePresent: bool, // true when the desktop shell runtime flag exists
  nativeCursorBridgePresent: bool, // true when the native cursor JS bridge exists
  nativeCursorSupported: bool, // native cursor bridge support result
  nativeCursorActive: bool, // native cursor capture active state from bridge diagnostics
  nativeCursorLastReason: string, // bounded native cursor diagnostic reason
  nativeCursorLastError: string, // bounded native cursor diagnostic error
  tauriInternalsPresent: bool, // true when Tauri IPC internals are visible
  tauriGlobalPresent: bool,   // true when the global Tauri API object is visible
  tauriGlobals: string,       // bounded comma-separated Tauri global key summary
  wsBufferedBytes: u32,     // browser WebSocket bufferedAmount
  serverTickMs: u16,        // latest server tick work duration seen in snapshot netStatus
  serverLagMs: u16,         // latest scheduler lag seen in snapshot netStatus
  slowTickCount: u32,       // latest server slow-tick count seen by this client
  headOfLineCount: u32,     // latest per-client pending-snapshot replacement count seen
  predictionMode: string,   // disabled, tracking, predicting, or resyncing
  pendingCommandCount: u16,
  acknowledgedCommandLatencyMs: u16, // latest local issue -> sim-ack latency
  commandsIssued: u32,                 // commands allocated in this report window
  commandSocketSendAccepted: u32,      // WebSocket.send accepted by the browser
  commandServerReceived: u32,          // accepted commandReceipt count
  commandSimAcknowledged: u32,         // commands covered by snapshot sim-consumption ack
  commandRejected: u32,                // rejected commandReceipt count
  commandIssueToSocketSendAcceptedLatestMs: u16,
  commandIssueToSocketSendAcceptedMaxMs: u16,
  commandIssueToSocketSendAcceptedP95Ms: u16,
  commandIssueToServerReceiptLatestMs: u16,
  commandIssueToServerReceiptMaxMs: u16,
  commandIssueToServerReceiptP95Ms: u16,
  commandServerReceiptToSimAckLatestMs: u16,
  commandServerReceiptToSimAckMaxMs: u16,
  commandServerReceiptToSimAckP95Ms: u16,
  commandIssueToSimAckLatestMs: u16,
  commandIssueToSimAckMaxMs: u16,
  commandIssueToSimAckP95Ms: u16,
  commandAckSnapshotReceivedToAppliedLatestMs: u16,
  commandAckSnapshotReceivedToAppliedMaxMs: u16,
  commandAckSnapshotReceivedToAppliedP95Ms: u16,
  oldestPendingCommandAgeMs: u16,
  maxPendingCommandCount: u16,
  commandFamilyMove: u32,              // command family counts for stable low-cardinality grouping
  commandFamilyAttackMove: u32,
  commandFamilyBuild: u32,
  commandFamilyTrain: u32,
  commandFamilyOther: u32,
  commandLifecycleExemplars: [{
    clientSeq: u32,                    // bounded top-N diagnostic exemplar, no command payload
    family: "move"|"attackMove"|"build"|"train"|"other",
    issuedElapsedMs: u32,              // report-window-relative client issue time
    stage: string,                     // stable lifecycle stage label
    stageMs: u16
  }],
  correctionDistancePx: u16,         // largest correction observed by the client
  correctionCount: u32,
  predictionDisableCount: u32,
  predictionDisableUserCount: u32,
  predictionDisableReplayCount: u32, // replay-viewer or replay-budget reset reasons
  predictionDisableSpectatorCount: u32,
  predictionDisableCompatibilityCount: u32,
  predictionDisableWasmCount: u32,
  predictionDisableOtherCount: u32,
  wasmTickMs: u16,          // latest measured WASM prediction/replay work duration
  wasmMemoryBytes: u32,     // current WASM memory buffer size, when available
  predictionReplayTicks: u16, // latest local replay/advance ticks processed in one measured step
  predictionReplayMaxMs: u16, // max WASM pending-command replay duration in report window
  predictionReplayMaxTicks: u16, // max pending-command replay ticks in report window
  predictionReplayBudgetExceededCount: u32
}
```
The snapshot payload, codec, parse, decode, apply, prediction-apply, cadence, command milestone,
command lifecycle, and desktop cursor runtime fields are report-window aggregates or bounded
summaries only; raw snapshot
payloads, raw timestamp arrays, entity ids, unit ids, target ids, positions, replay data, command
payloads, and raw cursor input events are not uploaded. HUD `jit` and `snapshotJitterMs` mean
snapshot arrival jitter, not JavaScript compiler/JIT time. The canonical single-segment payload
budget is 1280 bytes. It is intentionally below a common 1460-byte Ethernet TCP MSS because the measured
snapshot bytes are only WebSocket application payload bytes and exclude WebSocket framing plus TLS,
TCP, and IP overhead. Command milestone timing splits local issue to WebSocket send acceptance,
issue/send to receipt, receipt to sim acknowledgement, issue to sim acknowledgement, and ack snapshot
receipt to browser apply. `commandLifecycleExemplars` preserves at most five report-window
exemplars by `clientSeq`, stable command family, stage, and duration; it never includes units,
targets, positions, raw command payloads, or raw timestamp arrays. The
frame-work and renderer fields come from the browser's bounded frame-profiler report window; the
local debug surface may keep richer cumulative phase tables, but those raw arrays and detailed
recent frames are not uploaded. Durations are integer milliseconds after bounded rounding for the
wire. `rendererUpdate*` and `rendererPresent*` are stable scalars even when their nested phase rows
fall outside a capped top-five list. They are nested inside `match.renderer` and must not be added
to `frameWork*`. The two budget-miss counts use strict `> 1000/60 ms`; they reset after each report
upload, while the local profiler remains cumulative until explicitly reset. The existing 33 ms
`slowFrameCount` meaning is unchanged. `commandsIssued` is the report-window total and catches sustained
rapid input that may not reach the fixed 250 ms `commandBurstMax` threshold. Command burst fields
only count commands accepted by the browser WebSocket send path after local command-budget checks. Prediction
disable reason fields are stable buckets; detailed WASM loader errors stay local. The server logs
this message only when the aggregate contains
notable lag, jitter, browser frame stalls, 60 FPS work-budget misses, local JS frame work,
large-payload pressure, packet-budget pressure, snapshot parse/decode/apply cost,
snapshot cadence/burst issues, renderer update/present cost, WebSocket
backlog, server tick/scheduler pressure, sustained or bursty command density, command milestone delay/rejection, or prediction
correction/fallback signals, alongside the connection's `player_id`, room name, and reported
`match_run_id`. The same structured row also includes server-observed counters for the report
window, prefixed `server*`, such as command receipt counts, command frame deserialize time,
deserialize-to-room-enqueue time, room-event queue delay, room handling/receipt queue time,
receipt-send age, accepted-to-sim-ack time, bounded server command lifecycle exemplars, reliable
messages drained while a snapshot was pending, snapshot send age, and latest-only snapshot slot
stored/replaced/closed counts.
One reliable message before a snapshot with no send age or slot replacement is normal ordering, not
outbound pressure.
Those server-only log fields are not client protocol fields. Values are advisory because clients are untrusted; use them to diagnose
transport/browser/prediction/render behavior, not as gameplay authority.

### 2.2 Server → Client (`ServerMessage`)

| `t`        | Fields |
|------------|--------|
| `welcome`  | `playerId: u32` — assigned on connect. |
| `lobby`    | `room: string`, `kind: LobbyKind`, `hostId: u32`, `players: LobbyPlayer[]`, `canStart: bool`, `teamPreset: string`, `map: string`, `maps: AvailableMap[]` |
| `matchCountdown` | `countdownId: u32`, `durationMs: u32`, `words: string[]` — reliable pre-match countdown sent to every lobby participant after the host starts and before `start`. Active human clients begin warming their renderer when they become ready, then acknowledge this exact nonzero countdown generation with `matchLoadReady` once warmup completes. During this interval the server keeps the room in lobby setup, disables `canStart`, freezes lobby edits, and rejects new joins. At expiry it sends `start` only if every active human acknowledged; otherwise it returns the room to editable lobby state and broadcasts `<name> failed to load the game.` Spectators and AI do not block launch. |
| `start`    | `Game start payload` (see 2.3). |
| `snapshot` | `Per-player snapshot` (see 2.4). |
| `roomTimeState` | `Room-controlled time state` (see 2.6). |
| `roomTimeSeekStarted` | `controllerId: u32`, `fromTick: u32`, `targetTick: u32` — reliable broadcast to every replay viewer immediately before an accepted shared replay seek begins rebuilding. Rejected and rate-limited seeks do not emit it. |
| `livePauseState` | `Live match pause state` (see 2.6). |
| `observerAnalysis` | `Observer analysis state` (see 2.7). |
| `joinReplayPrompt` | `room: string` — the requested room is currently replay playback; clients should confirm before retrying `join` with `replayOk: true`. |
| `branchFromTickCreated` | `branchRoom: string`, `sourceTick: u32`, `seats: ReplayBranchSeat[]` — a separate practice branch room has been created from the source replay's current authoritative tick. |
| `branchStaging` | `room: string`, `sourceTick: u32`, `hostId: u32`, `seats: BranchStagingSeat[]`, `occupants: BranchStagingOccupant[]`, `canStart: bool` — reliable current state for a replay branch staging room. Sent after joins, leaves, claims, and releases. |
| `shutdownWarning` | `deadlineUnixMs: u64`, `secondsRemaining: u64` — deploy/termination drain has started; active matches may continue until the deadline, but new match starts are disabled. |
| `observationReady` | `matchRunId: string` — a watched all-AI match has resolved; this id retrieves its saved replay and joins its structured server logs. |
| `gameOver` | `winnerId: u32 | null`, `winnerTeamId: u32 | null`, `you: "won" | "lost" | "draw"`, `scores: PlayerScore[]` |
| `pong`     | `ts: number` (echo of the ping ts) |
| `commandReceipt` | `clientSeq: u32`, `serverTick: u32`, `accepted: bool`, `reason?: string` — reliable diagnostics-only room receipt. Does not reconcile prediction. |
| `error`    | `msg: string` |

`LobbyPlayer`: `{ id: u32, teamId: u32, factionId: string, name: string, ready: bool, color: string, isAi: bool, aiProfileId?: string, isSpectator: bool }`. `isAi` is
true for computer opponents (always shown ready; the client renders an "AI" tag, a host-only
profile selector, and a host-only remove control instead of a ready toggle). `aiProfileId` is
present only for computer opponents and identifies the canonical live AI profile selected for that
seat. `isSpectator` is true for human observers; they do not consume active map starts,
block readiness, or count toward win/loss.

`AvailableMap`: `{ name: string, description: string, minPlayers: u32, maxPlayers: u32 }`.
`name` is the stable value sent back in `selectMap`; `description` is display text for the lobby
selector; `minPlayers` is one and `maxPlayers` is derived from the authored `startLocations` count
for that map. Lobby `map` is the current selected map name and is distinct from replay start
metadata `mapName`.

`LobbyKind`: `"normal"` for ordinary public lobbies/live rooms and `"replay"` for replay staging
or active replay playback, including the automatic replay entered after a normal match ends.
Replay staging lobbies carry only spectator `LobbyPlayer` rows, report `canStart` when the host may
begin playback, send the replay artifact map name as `map`, and send an empty `maps[]` because map
selection is disabled. The HTTP `GET /api/lobbies` row uses the same `kind` values and includes
only safe room metadata: room, kind, host name, map, creation time, active-slot counts, spectator
count, phase, and join state. Active replay rows use `phase: "inGame"` and
`joinState: "inGame"`, report zero active slots, and count every connected replay viewer as a
spectator.

`GET /api/lab-scenarios` returns a bounded catalog of bundled lab checkpoint setup metadata:
`[{ id, title, description, tags, map, playerCount, filename }]`. `id` is the stable safe token used
in direct lab room URLs as `scenario=<id>`, `map` and `playerCount` mirror the listed setup
payload, and `filename` is the safe bundled JSON filename under
`server/assets/lab-scenarios/`. The listing deliberately omits the full setup JSON; lab starts
load it server-side from the manifest source of truth.

`teamId` is nonzero for active match players and AI seats. New active players and default-added AI
opponents are assigned to the next empty team after the currently occupied teams when possible,
falling back to the first empty team in `1..=4`; the host may move active human or AI seats between
those team slots. The normal lobby UI shows occupied teams plus one "New team" drop target while
fewer than four teams are occupied, plus a bottom spectator drop target for host-managed observer
moves. Spectator lobby rows carry `teamId: 0` because they are not match players. In normal
lobbies, `canStart` is false until there is at least one active seat, every active human is ready,
every active seat has a team in `1..=4`, and the active seat count is at or below the selected
map's `maxPlayers` cap and the server's hard four-player cap. Selecting a lower-capacity map
removes overflow AI seats first, then moves overflow humans to spectators. In replay staging
lobbies, `canStart` is true when a host spectator is present and the server is not blocking new
sessions for deploy drain.

`PlayerScore`: `{ id: u32, teamId: u32, name: string, color: string, apm: u32, unitScore: u32, structureScore: u32,
unitsKilled: u32, unitsLost: u32, buildingsKilled: u32, buildingsLost: u32 }`. `scores` is a
frozen server snapshot taken when that recipient gets `gameOver`; it is not live-updated while a
3-4 player match continues. Unit/structure score is the configured steel+oil value of every
unit/building entity created for that player, including starting entities. `apm` counts one action
per authoritative gameplay command regardless of the number of units in that command, excludes the
first 60 seconds, and averages the remaining actions over the remaining simulated match time.

`winnerTeamId` is the winning team's id when a winner exists, otherwise `null`. `winnerId` remains
for FFA compatibility. During singleton-team FFA, `winnerTeamId` matches `winnerId`; during team
wins, `winnerId` is the first living player on the winning team in stable start/lobby order.

`ReplayBranchSeat`: `{ playerId: u32, teamId: u32, factionId: string, name: string, color: string, claimable: bool }`. Seats are
listed in original replay player order. `claimable` is false only for unsupported original seats;
the first implementation rejects AI-seat replays before creating a branch, so successful branch
creation currently reports all seats as claimable.

`BranchStagingSeat`: `{ playerId: u32, teamId: u32, factionId: string, name: string, color: string, claimantId?: u32,
claimantName?: string }`. Seats are listed in original replay player order. A missing claimant
means that original seat is still available to claim.

`BranchStagingOccupant`: `{ id: u32, name: string }`. Occupants are all human viewers currently in
the branch staging room, whether they have claimed an original seat or are remaining spectators.

### 2.3 `start` payload
Sent when a live match begins and when replay playback is rebuilt, including after replay seeks. Carries static match metadata and recipient-scoped capabilities for that start.
```
{
  t: "start",
  playerId: u32,                 // your id (repeat of welcome for convenience)
  spectator: bool,               // true when this connection is observing only
  predictionBuildId?: string,    // live active players only; server/client bundle id
  predictionVersion?: u32,       // live active players only; currently 1
  matchRunId?: string,           // live match correlation id for log joins
  capabilities?: {               // explicit recipient-scoped shared room affordances
    roomTime?: {
      available?: bool,
      setSpeed?: bool,
      pause?: bool,
      step?: bool,
      seekRelative?: bool,
      seekAbsolute?: bool,
      timeline?: bool
    },
    matchControls?: { pause?: bool },
    visibility?: { visionSelection?: bool },
    commands?: { gameplay?: bool },
    actions?: { branchFromTick?: bool }
  },
  diagnostics?: {                // explicit recipient-scoped diagnostic affordances
    movementPaths?: "ownerOnly"|"all",
    observerAnalysis?: bool
  },
  replay?: {                     // present for production replay playback
    artifactSchemaVersion: u32,
    serverBuildSha: string,
    mapName: string,
    mapSchemaVersion: u32,
    mapContentHash: string,
    seed: u32,
    durationTicks: u32
  },
  lab?: {                        // present for lab room starts
    room: string,                // safe public lab id, not the hidden internal room prefix
    operatorId: u32,
    role: "operator"|"readOnly",
    vision: { mode: "all" } | { mode: "team", teamId: u32 }, // legacy setup/default metadata; runtime selection uses setVisionSelection
    godModePlayers?: u32[],
    initialCamera?: { centerX: u32, centerY: u32 }, // optional world-pixel camera center from the selected setup
    dirty: bool,
    operationCount: u32
  },
  observerView?: { mode: "all" } | { mode: "player", playerId: u32 } | { mode: "players", playerIds: u32[] },
  // authoritative per-connection privileged observer selection; omitted for active players
  tick: u32,                     // starting tick (0 for normal starts; restored checkpoint tick for checkpoint replays)
  map: {
    width: u32, height: u32,     // in tiles
    tileSize: u32,               // world px per tile
    // terrain: row-major array length width*height. Codes: 0 grass, 1 rock,
    // 2 water, 3 bare road, 4 horizontal-marked road, 5 vertical-marked road,
    // 6 NW-SE diagonal-marked road, 7 NE-SW diagonal-marked road.
    // Grass/all roads are passable; rock/water are impassable.
    terrain: number[],
    // All neutral resource nodes (static, never move). Sent so the client can
    // render them on the minimap before fog-of-war reveals them.
    resources: [ { id: u32, kind: "steel"|"oil", x: f32, y: f32 } ]
  },
  players: [ { id, teamId, factionId, name, color, isAi, startTileX, startTileY } ], // active match players only
}
```
Units/buildings arrive via snapshots (so they obey fog), including the player's own starting
loadout from the validated faction catalog. Normal live starts may skip countdown when there are
one or zero active human seats, including one-human-vs-AI and AI-only rooms, but they still use
ordinary starting resources and ordinary faction loadouts. Dev scenario start payloads may
advertise `diagnostics.movementPaths: "all"` because those rooms
intentionally use full-world diagnostic projection. `players[].isAi` identifies computer-controlled
seats in live matches and remains preserved in replay start payloads so client diagnostics can
require an actual AI participant. Replay viewers and live spectators receive
`diagnostics.observerAnalysis: true` only
when room projection policy will send observer-analysis payloads to that recipient.
`capabilities` is the neutral control/affordance contract. Live active players receive
`commands.gameplay: true` and `matchControls.pause: true`; live spectators receive
`matchControls.pause: true` without `commands.gameplay`. Replay viewers, dev-watch viewers, and
lab viewers do not receive match controls. AI-only live matches are the live route that maps to the
speed-only room-time capability profile: connected spectators receive room-time speed/pause
controls, but no step, relative seek, absolute seek, or timeline capability.
Replay branch live rooms also advertise `matchControls.pause` to both claimed-seat and observer
recipients; gameplay commands remain claimed-seat only through the branch-live seat alias path.
Every privileged observer (live spectator, replay, dev-watch, and Lab) advertises
`visibility.visionSelection: true`. Replay playback additionally advertises room-time
speed/pause/relative seek/absolute seek/timeline controls. Replay branch creation is advertised separately with
`actions.branchFromTick: true` only when the current replay can accept a branch request. Dev scenario
watch rooms advertise speed/pause/step room-time controls without seek. Lab rooms advertise
speed/pause/step/relative seek/absolute seek/timeline controls and generic vision selection, but
not branch-from-tick controls. Clients must not infer these shared affordances
from `replay`, `lab`, URL-local dev-watch state, or legacy debug flags.
The browser's shared room-time controls render lab seek and keyframe metadata from these
capabilities and `roomTimeState`; observer perspective is per connection and remains separate from
Lab command authority.
Spectator start payloads keep the spectator connection's `playerId`, set `spectator: true`, and
list only active match players in `players`. Late live spectator joins receive the same live start
payload shape stamped from the current `Game::start_payload()` tick, with prediction metadata
omitted and live spectator capabilities/diagnostics applied for that recipient.
Lab room start payloads set `lab` metadata and currently also set `spectator: true` with prediction
metadata omitted. Labs use a hidden internal room id, a default two-team real `Game` template, and
server-owned projection. `initialCamera`, when present, is a setup-authored world-pixel center
point that the browser uses for the first Lab view instead of centering on the operator's home
base. `role` names the room-owned operator/read-only viewer classification.
Direct lab URL joiners currently receive `operator`; `readOnly` remains available for future
explicit viewer modes. `operatorId` identifies the original lab joiner for compatibility metadata,
not the sole authority for privileged lab operations. Lab room-time controls are shared room state:
any operator can pause, resume at a clamped positive speed, step exactly one authoritative tick
while paused, seek relatively, or seek to an absolute retained lab timeline tick. Accepted lab seeks
restore the nearest retained lab keyframe at or before the target, replay accepted lab operations
and issue-as commands through normal server validation, re-send lab start metadata, broadcast
`roomTimeState`/`labState`, and send fresh snapshots. If a new lab operation or issue-as command is
accepted after a past seek, future lab timeline entries and keyframes after the current tick are
truncated; there is no branch, redo, or undo protocol in this slice.

For compatibility with hand-built fixtures and older replay artifacts, missing `teamId` values at
simulation/replay/test-helper boundaries default to singleton FFA: the player's own nonzero `id`.
Current live server payloads always emit explicit nonzero `teamId` values for active players.
The canonical default faction id is `kriegsia`; `ekat` is also a playable catalog id. Start payloads emit `factionId` for every active
start player, lobby seat, and replay branch seat, and replay artifacts store `faction_id` for every
player. Missing faction requests default to `kriegsia` in normal lobby, AI, self-play, and
dev-start contexts, while explicit `kriegsia` and `ekat` requests are accepted by the current
playable faction policy. Other ids are rejected unless a lifecycle path explicitly accepts recorded
replay data or the `phase2_empty_fixture` test fixture.
Protocol vocabulary is not lifecycle admission: adding a string constant, compact code, or payload
field does not make a faction playable. Fixture-only, reserved/future, and historical-only ids must
not become valid `setFaction`, AI-seat, replay-branch, or post-match replay ids without updating
`docs/design/faction-architecture-inventory.md`, the lifecycle validator, and protocol parity in
the same change.

Prediction start compatibility metadata is present for live active players. Spectator,
replay-viewer, Lab, and dev-watch recipients omit it. Clients MUST keep
prediction disabled unless `predictionVersion` matches their supported prediction protocol version
and, when both sides know a build id, `predictionBuildId` matches the client bundle id. Mismatches
fall back to authoritative snapshots/tracking instead of running local visual reconciliation.

Replay start payloads include `replay` metadata so the client can display or cache a
self-describing playback session. The server validates replay artifacts before playback: artifact
schema version, map name, map schema version, and map content hash must match the running
server/map asset or the replay is rejected with a clear error. Schema 3 artifacts additionally
validate the embedded `startState.checkpointPayload` map binding, including the materialized map
hash, before constructing the replay `Game`. A server build-SHA mismatch is warning-compatible:
replay metadata keeps the original `serverBuildSha`, and the server logs or surfaces a warning
while attempting playback. Saved self-play artifacts use the same `ReplayArtifactV1` contract as
post-match and match-history replays; pre-unified dev-only artifact payloads are rejected instead
of falling back to a separate loader.

Replay artifact schema version 3 stores a launch-time `startState` containing map binding fields
and a tick-zero `GameCheckpointV1` text payload, plus ordered `players[]` with each original
`team_id` and required `faction_id`, plus `playerLoadouts[]` with one `{ playerId, factionId,
loadoutId, startingSteel, startingOil }` record per player. Replay reconstruction restores the
start `Game` from the checkpoint payload and then applies the authoritative `commandLog[]` on the
recorded ticks. The compatibility `winnerId`, optional `winnerTeamId`, `durationTicks`, and
`finalScores[]` with each row's `teamId` remain part of the artifact. Artifact schema 2 and older
payloads are rejected with the unsupported-schema error; new captures always include explicit
nonzero player and score team ids, required player faction ids, required player loadout records,
`startState`, and `winnerTeamId` when there is a winning team.

Persisted match-history replay launch creates a replay staging lobby rather than starting playback
on the first join. The first spectator becomes host, additional viewers may gather from the lobby
browser, and the host's `start` transitions the shared room into the same replay playback runtime
used after post-match replay. Replay staging ignores ready toggles, active-seat role changes, team
or faction edits, AI changes, and map selection. After playback starts, the room remains in the
lobby browser as an active replay. New viewers join the existing shared cursor and receive its
current start metadata, room-time state, snapshot, and observer analysis immediately, including
when playback is already at its final tick.

When a real multi-player match ends, the server sends the normal `gameOver` score payload, clears
pending latest-only live snapshots for connected humans, and then sends a replay `start` payload
at tick 0 plus `roomTimeState`. Post-match replay defaults every viewer to all active players'
combined authoritative vision and starts at `2.0x` speed. `returnToLobby` detaches only the
requesting replay viewer; the shared replay session remains alive for everyone else. The room drops
the replay simulation after the last viewer leaves; for normal public rooms, that empty room then
asks the lobby registry to dispose the public name rather than holding it for reconnect. Dedicated
replay rooms created for match-history or dev replay viewing follow the same per-viewer detach rule
after playback has started; they keep the shared replay session alive until the room empties.
Automatic post-match replay rooms remain listed and joinable as replay rooms for that entire
shared-viewing lifetime.

### 2.4 `snapshot` payload (per-player, fog-filtered)
`Snapshot` remains the semantic shape used by server game code and by client modules after
transport decode:
```
{
  t: "snapshot",
  tick: u32,
  worldCombatPosition?: [f32, f32], // coarse global combat area; omitted when inactive
  steel: u32, oil: u32,       // your resources
  supplyUsed: u32, supplyCap: u32,
  entities: Entity[],            // your non-resource entities (always) + entities visible to living-team current/firing/death vision
  resourceDeltas?: ResourceDelta[], // visible resource remaining updates; omitted when empty
  smokes?: SmokeCloud[],         // active smoke clouds visible to this recipient; omitted when empty
  abilityObjects?: AbilityObject[], // active ability world objects visible to this recipient; omitted when empty
  trenches?: Trench[],           // neutral trench terrain visible to this recipient; omitted when empty
  visibleTiles?: u8[],           // row-major current server visibility; 1 = visible, 0 = fogged
  exploredTiles?: u8[],          // row-major server-owned exploration history for this perspective
  rememberedBuildings?: RememberedBuilding[], // stale enemy building intel for projected players
  rememberedAntiTankGuns?: {id, owner, x, y, facing, observedTick}[], // stale deployed enemy AT-gun arcs for projected players
  events: Event[],               // transient things to surface (see 2.5)
  upgrades?: string[],           // completed permanent upgrades for this recipient
  playerResources?: {id, steel, oil, supplyUsed, supplyCap, apm, upgrades?}[], // projected players; observer modes only; upgrades are that real owner's completed research
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

Steel, Oil, and Supply are fixed protocol fields for this faction plan. Normal snapshots,
observer `playerResources`, compact `"s"`, compact `"pr"`, start-map resources, score values, and
observer analysis remain on the current Steel/Oil/Supply schema; faction-specific or arbitrary
resource vectors are deferred to a separate generic-resource migration. Selected observer snapshots
include `playerResources` rows only for the explicitly selected real player ids; omniscient views
expose all active player rows. Each row carries that owner's completed research so multi-player
observer views do not depend on the single-recipient top-level `upgrades` field.

For normal active-player snapshots, entity visibility and `visibleTiles` are projected from the
server-authoritative union of current fog grids contributed by living teammates on the recipient's
team. A defeated/disconnected teammate stops contributing live sight; if that player's team still
has a living member, their own connection continues to receive the surviving team's current
visibility. `exploredTiles` is the cumulative server-owned history of those team-visible tiles; it
is checkpointed and replayed, and clients replace their local exploration grid on every snapshot
instead of accumulating whichever observer perspectives they happened to render. Allied
non-resource entities visible through team current fog expose full read-only
inspection details: hp/state/facing/setup state, production or research kind/progress/queue length,
legacy Scout Plane queue presence, construction progress, gatherer latched node, active Breakthrough
status, and safe combat tracers.
Combat `targetId` and `weaponFacing` for allied units are sent whenever an attack-capable entity has
a visible acquired target, including autonomous acquisition while its explicit order state remains
idle. They are omitted when the target is not visible in the recipient's team-current actionable
fog, so allied units attacking hidden enemies do not reveal hidden target ids or target directions.
`steel`, `oil`, supply, `upgrades`, rallies, order plans,
construction activity hints, ability controls/autocast toggles, debug paths, and command authority
remain exact-owner-only in normal active-player and selected-player/team observer projections.

`worldCombatPosition` is the narrow global exception to fog-filtered combat detail. The server sets
the same coarse world point for every active player, spectator, replay, Lab, and dev recipient.
It changes only on 15-tick boundaries after hostile weapon fire/impact activity and remains present
through the last boundary no more than 60 ticks after the most recent activity. The point is the
centroid of distinct combat areas observed during the latest activity tick, snapped to a 32-tile
grid. It intentionally reveals approximate battle direction, but no player, team, entity, weapon,
count, composition, exact position, or live cadence. Clients use it only to pan one generic
constant-gain background combat bed; positional combat events and entities remain fog-filtered.
Full-world diagnostic projections are the explicit exception: they may project those per-entity
planning and setup details through each entity's real owner so lab/dev inspection overlays are
complete for every player.
Anti-Tank Guns that fire from fog create actionable temporary live fog for the recipients that see
the attack event, except when the resolved shot victim is a Tank Trap. The revealed gun is
projected as a normal non-`visionOnly` snapshot entity and
can validate direct attack commands or combat target acquisition until the firing reveal expires.
Its reveal-only actionable tile is deliberately omitted from `visibleTiles` and does not enter
`exploredTiles`; clients infer the explicit above-fog `shotReveal` presentation when a projected
enemy unit occupies a presentation-dark tile. This keeps the unit fully legible without clearing
the terrain tile beneath it.
The expiration is calculated from the firing tick plus that gun's firing-cycle cooldown plus
0.5 seconds (`TICK_HZ / 2`), not from a hardcoded wall-clock duration. The one-second counterfire
reaction applies only while a firing-reveal-stamped tile is the target's necessary source of
actionable sight;
ordinary live vision takes precedence and bypasses that reaction gate without changing weapon
reload timing. Explicit attacks and autonomous acquisition use team-current ordinary sight for
both legality and reaction bypass. Repeated shots extend one stable reveal
episode, so move orders or transient target switches cannot restart the same episode's reaction
deadline. Tile-level provenance covers colocated entities and remains tied to the stamped tile when
the firing entity moves before the next fog rebuild.
Artillery Fire creates the same kind of actionable temporary live fog for every enemy player,
subject to normal smoke suppression, when the shell is launched. The reveal exposes the firing gun
as a normal snapshot entity without exposing the target point, its terrain tile, surrounding
terrain, or pre-impact target marker.
Lingering death sight is ordinary temporary team sight for five seconds. It makes current
non-owned units/buildings visible as normal entities, contributes to `visibleTiles`, refreshes
remembered buildings, and can drive command validation and combat auto-acquisition while active.
Ability world
objects are projected separately in `abilityObjects`: normal players
receive only objects whose world position is visible in their current team fog, while full-world
dev snapshots include every object and spectator/replay snapshots use the existing union vision.
Enemy objects never carry owner-only state, and `sourceCasterId` is omitted unless the caster is
safe for the recipient or the recipient is an owner/spectator/full-world viewer.

MessagePack compact binary snapshot frames are the live WebSocket snapshot path. Each binary frame
starts with the ASCII magic `RTSM`, a one-byte snapshot codec version (`1`), then a MessagePack map
containing the same compact snapshot object shape shown below. The active snapshot codec is
`messagepack-compact`, codec version 1, compact snapshot version 46. `client/src/net.js` calls
`parseServerFrame`; the binary frame parser in `client/src/protocol_frame.js` returns the raw
compact snapshot object, then `decodeCompactSnapshot` expands it back into the semantic object above
before dispatching `S.SNAPSHOT`.

The rollout is direct and latest-version-only. Reliable non-snapshot messages (`welcome`, `start`,
`lobby`, `pong`, errors, room/lab/replay control messages, and game over) remain JSON text. The
server does not negotiate stale-client capability and does not maintain a compact JSON fallback mode
for live snapshots; rollback is a normal Git revert of the MessagePack snapshot change. Compact JSON
serialization remains available in local tooling and tests as a historical size baseline, and the
browser can still decode object-shaped JSON snapshots for narrow dev/test use, but that is not the
normal live path.

The live compression diagnostics are report-only. `snapshotBytes*` fields are browser-delivered
MessagePack application payload measurements after any transport extension would have been decoded
by the browser; they are not compressed wire bytes. `snapshotCodec`, `snapshotCodecVersion`, and
`snapshotFrameKind` identify the active snapshot path for the report window. `websocketExtensions`
mirrors the bounded browser `WebSocket.extensions` string, and `websocketCompression` is a
normalized label that is `permessage-deflate` only when that extension appears. With the current
Axum 0.8 / Tungstenite 0.29 server stack, direct `permessage-deflate` negotiation is not available,
so the expected live label is `none` until a future phase changes the WebSocket implementation or
adds an explicit application compression envelope.

```
{
  "t": "snapshot",
  "v": 46,
  "s": [tick, steel, oil, supplyUsed, supplyCap],
  "e": [
    [
      id, owner, kind, x, y, hp, maxHp, state,
      facing?, weaponFacing?, prodKind?, prodProgress?, prodQueue?,
      buildProgress?, latchedNode?, targetId?, setupState?, remaining?, rally?, oilUsed?,
      setupFacing?, orderPlan?, chargeCooldownLeft?, abilities?, breakthroughTicks?,
      visionOnly?, debugPath?, rallyPlan?, prodUpgrade?, buildActive?, deconstructProgress?,
      weaponRangeTiles?, occupiedTrenchId?, scoutPlane?, prodScoutPlaneQueued?,
      panzerfaustLoaded?, prodRepeatKinds?, prodWaiting?, breakthroughAuraTicks?, extractorActive?,
      prodUpgradeQueue?
    ]
  ],
  "r": [[id, remaining]],         // omitted when empty
  "sm": [[id, x, y, radiusTiles, expiresIn]], // omitted when empty
  "ao": [[id, owner, ability, kind, x, y, expiresIn?, sourceCasterId?, ownerState?]], // abilityObjects; omitted when empty
  "tr": [[id, x, y, radiusTiles]], // trenches; omitted when empty
  "fg": [firstValue, runLen, ...], // RLE visibleTiles; omitted when empty/no-fog
  "eg": [firstValue, runLen, ...], // RLE exploredTiles; omitted when empty/no-fog
  "wc": [1024, 2048],             // worldCombatPosition; omitted when inactive
  "mb": [[id, owner, kind, x, y, [[tileX, tileY], ...], observedTick]], // rememberedBuildings; omitted when empty
  "ma": [[id, owner, x, y, facing, observedTick]], // rememberedAntiTankGuns; omitted when empty
  "ev": [EventRecord],            // omitted when empty
  "pr": [[id, steel, oil, supplyUsed, supplyCap, apm, [upgradeCode...]]], // projected observer playerResources; omitted when empty
  "n": [serverLagMs, tickMs, flags, slowTickCount, headOfLineCount,
        predictionVersion?, lastSimConsumedClientSeq?, lastSimConsumedClientTick?]
}
```

Compact numeric codes:

| Vocabulary | Codes |
|------------|-------|
| `kind` | 1 `worker`, 2 `rifleman`, 3 `machine_gunner`, 4 `anti_tank_gun`, 5 `tank`, 6 `city_centre`, 7 `depot`, 8 `barracks`, 9 `training_centre`, 10 `factory`, 11 `steel`, 12 `oil`, 13 `steelworks`, 14 `scout_car`, 15 `mortar_team`, 16 `artillery`, 17 `research_complex`, 18 `command_car`, 19 `ekat`, 20 `zamok`, 21 `tank_trap`, 22 `golem`, 23 `pump_jack`, 24 `panzerfaust`, 25 `scout_plane` |
| `state` | 1 `idle`, 2 `move`, 3 `attack`, 4 `gather`, 5 `build`, 6 `train`, 7 `construct`, 8 `dead` |
| `setupState` | 1 `packed`, 2 `setting_up`, 3 `deployed`, 4 `tearing_down` |
| `orderStage` | 1 `move`, 2 `attackMove`, 3 `attack`, 4 `gather`, 5 `build`, 6 `smoke`, 7 `setupAntiTankGuns`, 8 `charge`, 9 `mortarFire`, 10 `pointFire`, 11 `breakthrough`, 12 `ekatTeleport`, 13 `ekatLineShot`, 14 `ekatMagicAnchor`, 15 `deconstruct`, 16 `ekatConsumeGolem`, 17 `blanketFire`, 18 `dismissScoutPlane`, 19 `scoutPlane`, 20 `holdPosition` |
| `ability` | 1 `charge`, 2 `smoke`, 3 `mortarFire`, 4 `pointFire`, 5 `breakthrough`, 6 `ekatTeleport`, 7 `ekatLineShot`, 8 `ekatMagicAnchor`, 9 `ekatConsumeGolem`, 10 `blanketFire`, 11 `dismissScoutPlane`, 12 `scoutPlane` |
| `abilityObject.kind` | 1 `returnMarker`, 2 `magicAnchor`, 3 `lineProjectile` |
| `upgrade` | 1 `methamphetamines`, 2 `anti_tank_gun_unlock`, 3 `tank_unlock`, 4 `artillery_unlock`, 5 `mortar_autocast`, 7 `ballistic_tables`, 8 `entrenchment`, 9 `smoke_plus`, 10 `panzerfausts` |
| `weaponKind` | 1 `worker_tools`, 2 `golem_fists`, 3 `rifleman_rifle`, 4 `machine_gunner_mg`, 5 `scout_car_mg`, 6 `anti_tank_gun`, 7 `panzerfaust_loaded_shot`, 8 `mortar_team_mortar`, 9 `artillery_gun`, 10 `tank_cannon`, 11 `tank_coax` |
| `notice.severity` | 1 `info`, 2 `warn`, 3 `alert` |
| `EventRecord` | `[1, from, to]` attack, `[1, from, to, reveal?, toPos?]` legacy attack with optional shooter reveal and target position, `[1, from, to, revealOrNull, toPosOrNull, weaponKind]` attack with compact weapon hint, `[2, id, x, y, kind]` death, `[3, id, kind]` build, `[4, msg]` notice, `[4, msg, severity]` position-free notice with severity, `[4, msg, severity, x, y]` positioned notice, `[5, [fromX, fromY], [toX, toY], delayTicks]` smoke launch, `[6, x, y, radiusTiles]` mortar impact/marker, `[6, x, y, radiusTiles, from?, reveal?]` mortar impact with optional shooter reveal, `[7, from, [x, y], radiusTiles, delayTicks]` artillery target marker, `[8, x, y, radiusTiles]` artillery impact, `[9, from, [fromX, fromY], [toX, toY], radiusTiles, delayTicks]` mortar launch, `[10, to]` overpenetration damage, `[11, owner, x, y, facing]` global artillery firing minimap marker, `[12, from, [fromX, fromY], [toX, toY], delayTicks]` Panzerfaust launch, `[13, x, y]` Panzerfaust impact, `[15, to]` missed direct shot (14 removed with conversion) |

#### 2.4.1 Boundary inventory

This inventory records the current source-of-truth map after the protocol mirror split. It does not
change the wire shape or compact snapshot version. Public lab catalog, import, export, and authoring
validation errors use checkpoint setup terminology; legacy `LabScenario` wire and type names remain
compatibility internals.

| Value/path | Rust owner | JS mirror path | Category | Current checker | Proposed future checker | Client-only exclusion reason | Compact version impact |
|------------|------------|----------------|----------|-----------------|-------------------------|------------------------------|------------------------|
| `ClientMessage`, `ServerMessage`, `Command`, HTTP lobby browser/create endpoints, HTTP lab setup catalog endpoint, HTTP map handoff endpoints, lobby/replay/branch message tags and fields | `server/crates/protocol/src/lib.rs`; lobby/lab catalog/map-handoff HTTP route handlers in `server/src/main.rs` and `server/src/map_handoffs.rs`; lab catalog source of truth in `server/src/lab_scenarios.rs`; room-task summaries in `server/src/lobby/**` | `client/src/protocol.js` `C`, `S`, `CMD`, `msg.*`, `decodeServerMessage`; `client/src/lab_catalog.js` consumes the HTTP lab catalog; `client/src/map_editor_handoff.js` consumes the map handoff API; future internal `client/src/protocol_*.js` or `client/src/protocol/**` files must re-export through `client/src/protocol.js` | wire/HTTP DTO | `tests/protocol_parity.mjs` compares the structured Rust protocol contract dump to JS tags/builders/decoder and asserts stable JS public exports; serde compile/tests plus `rts-protocol` public-surface integration coverage guard Rust export names; focused server tests cover lobby summary, create-lobby behavior, lab catalog loading, and map handoff binding | Remaining source-text checks for DTO/lobby assertions outside the current dump scope | Lab catalog rows and map handoff records are HTTP metadata, not mirrored through `protocol.js`; their app-owned clients validate response shapes | No compact bump unless a compact snapshot slot/code changes; normal JSON message changes still require Rust, JS, and docs together |
| Semantic start/snapshot/replay/analysis DTOs | `server/crates/contract/src/lib.rs`, re-exported by `server/crates/protocol/src/lib.rs` | `client/src/protocol.js` decoder output consumed by client modules | wire DTO | `tests/protocol_parity.mjs` fixture decodes selected compact fields; Rust serde tests cover local serialization | Structured contract/schema dump for semantic DTO fields plus compact round-trip fixtures | None; JS is a protocol mirror | Compact bump only when the live compact representation changes |
| `terrain` codes | `server/crates/protocol/src/contract_metadata.rs` `terrain`, re-exported by `lib.rs`; adapter test checks rules terrain constants | `client/src/protocol_constants.js` `TERRAIN` and `PASSABLE`, re-exported by `client/src/protocol.js` | wire DTO / compact transport code | `tests/protocol_parity.mjs` extracts Rust terrain codes | Structured protocol constants dump | None | No compact snapshot bump today; terrain is in the `start.map.terrain` payload, not the compact snapshot frame |
| `kinds` strings, `KIND`, `UNIT_KINDS`, `BUILDING_KINDS`, `RESOURCE_KINDS` | `server/crates/protocol/src/contract_metadata.rs` `kinds`, re-exported by `lib.rs`; domain identity is `rts-rules::EntityKind::stable_id()` | `client/src/protocol_constants.js` `KIND`, `UNIT_KINDS`, `BUILDING_KINDS`, `RESOURCE_KINDS`, re-exported by `client/src/protocol.js` | wire DTO plus domain adapter grouping | `tests/protocol_parity.mjs` checks kind code mapping; adapter tests round-trip every `EntityKind`; catalog parity checks many kind references | Structured protocol constants dump plus catalog export that classifies unit/building/resource groups | None | Bump only if compact kind codes or compact slots change; append-only codes otherwise |
| `server/src/protocol.rs` and `server/crates/sim/src/protocol.rs` kind conversion | Rules/sim-aware adapter modules | No direct JS mirror beyond the protocol kind strings | domain adapter mapping | Rust adapter tests in both modules | One shared rules-aware adapter path with a single round-trip test | Not client data | No compact impact unless output kind strings/codes change |
| `states`, `SETUP`, `NOTICE_SEVERITY`, `VISION_SELECTION`, `WEAPON_KIND`, and event discriminators | `server/crates/protocol/src/contract_metadata.rs` string vocabulary; compact event serialization lives in `server/crates/protocol/src/compact_snapshot.rs` | `client/src/protocol_constants.js` constants, re-exported by `client/src/protocol.js`; compact decoder lives in `client/src/protocol_snapshot.js` behind `decodeServerMessage` | wire DTO / compact transport code | `tests/protocol_parity.mjs` checks state, setup, notice severity, weapon kind, and event compact codes; selected decoder fixtures | Structured protocol constants and compact event-shape dump | None | Bump when compact event/entity slots change |
| `COMPACT_SNAPSHOT_VERSION`, `PREDICTION_PROTOCOL_VERSION`, compact top-level keys, optional entity slots, limits, and net status slots | `server/crates/protocol/src/contract_metadata.rs` owns versions and slot metadata; `server/crates/protocol/src/compact_snapshot.rs` compact serializer; `server/crates/protocol/src/messagepack_frame.rs` frame writer | `client/src/protocol_constants.js` `COMPACT_SNAPSHOT_VERSION` and `MAX_COMPACT_*` limits; `client/src/protocol_snapshot.js` compact decoder; `client/src/protocol_frame.js` binary frame parser | compact transport code | `tests/protocol_parity.mjs` source-text version checks and fixture decode | Structured compact schema dump including slot names, order, caps, and version | None | Direct owner of compact version; slot/order changes require bump unless strictly optional trailing additions preserve decoder compatibility by explicit decision |
| Compact code tables for kind, state, setup, order stage, ability, ability object kind, upgrade, weapon kind, notice severity, and event records | `server/crates/protocol/src/contract_metadata.rs` code tables and code functions; compact event serializer lives in `server/crates/protocol/src/compact_snapshot.rs` | `client/src/protocol_constants.js` `*_CODE` and reverse-code maps, re-exported through `client/src/protocol.js` where public; compact record decoder lives in `client/src/protocol_snapshot.js` | compact transport code | `tests/protocol_parity.mjs` extracts Rust functions/events and rejects duplicate or `255` real codes | Structured protocol constants dump generated from Rust instead of source scraping | None | `255` remains unknown/sentinel; real codes must not use it. New codes should append without reusing old values; incompatible reorder/removal requires compact version bump |
| Ability and upgrade ids in command/research/snapshot payloads | Typed identities and stable ids in `server/crates/rules/src/faction.rs`; intentional wire constants/codes mirror in `server/crates/protocol/src/contract_metadata.rs` | `client/src/protocol_constants.js` `ABILITY`, `UPGRADE`, `ABILITY_CODE`, `UPGRADE_CODE`, re-exported by `client/src/protocol.js`; command-card descriptors in `client/src/config.js` | domain identity plus wire DTO, compact transport code, faction catalog fact | Rust adapter tests check every rules-owned id against protocol vocabulary and compact-code coverage; `tests/protocol_parity.mjs` checks protocol ids/codes; `scripts/check-faction-catalog-parity.mjs` checks catalog-exposed ability codes and descriptors | Structured protocol dump plus complete faction catalog dump | None where mirrored from Rust; catalog descriptors are not UI-only when exported by Rust | Code/order changes can require compact bump; descriptor-only changes do not |
| `DEFAULT_FACTION_ID` | `server/crates/contract/src/lib.rs`, re-exported by protocol | `client/src/protocol.js` | wire DTO / faction catalog fact | `tests/protocol_parity.mjs`; `scripts/check-faction-catalog-parity.mjs` checks default catalog id | Structured contract/catalog dump | None | No compact impact |
| `PLAYER_PALETTE` lobby colors | `server/src/lobby/mod.rs` assigns authoritative lobby/start colors | `client/src/config.js` fallback palette | server-owned presentation data mirrored by client | `tests/protocol_parity.mjs` source-scrapes the Rust palette | Structured lobby/config dump | Not client-only because server sends assigned colors; JS is fallback/render mirror | No compact impact |

Compact entity records are positional arrays. Optional fields keep the semantic order above and
trailing missing optional fields are omitted; interior missing optional fields are encoded as
`null`. The `rally` slot is itself a two-element `[x, y]` array (or `null`).
The `orderPlan` slot is an owner-private array capped at 9 entries: exact-owner in normal and
selected observer projections, and per-entity-owner in full-world diagnostic projections. It contains
the current active stage first, followed by queued unit stages in execution order. Artillery Fire
stages carry the server-stored effective center after range locking, not the raw clicked point; the
selected radius is server-owned order state but is not projected in this compact stage. Clients may temporarily merge local pending
move/setup/fire stages for preview continuity while waiting for command acknowledgement, but the
snapshot `orderPlan` is the only authoritative queued-plan contract and stale local previews must
reconcile to it. Each compact stage is `[kind, x, y]`, where `kind` uses the `orderStage` compact
code table above.
Stages carry safe world points only, never target ids; hidden attack target stages may be omitted
rather than leaking enemy positions through fog. Production building rally points are exposed
separately through `rally` and `rallyPlan` and are not part of `orderPlan`. `rallyPlan` is appended
after `debugPath` in compact snapshots to preserve older optional slot positions; it follows the
same owner-private projection policy, is capped at four stages, and uses the same `[kind, x, y]`
compact stage encoding with `move` and `attackMove` stages.
The `abilities` slot is owner-only and capped at 8 entries. Each compact ability cooldown is
`[ability, cooldownLeft, remainingUses?, autocastEnabled?, activeObjectId?, availableTick?, lockoutUntilTick?, expiresIn?, chargeRechargeLeft?]`,
where `ability` uses the `ability` compact code table above. `charge` is legacy and currently has
no eligible carriers, cooldown, or runtime status.
The server projects ability affordances only when the owning player's faction catalog exposes that
ability for the entity's global kind and the registry marks it for command-card exposure. Artillery
projects one `pointFire`-backed Fire affordance for selected owning clients; the `blanketFire`
catalog row remains as the hidden order implementation for the unified command.
Scout Car `smoke` remains visible but disabled until the owner has a completed R&D Complex. The
server checks that building requirement when a Smoke command is issued and again when a queued
Smoke order promotes, so losing the R&D Complex relocks the ability.
`remainingUses` is present for charged or finite-use abilities; a value of `0` means the ability
cannot currently be used by that caster. Scout Car Smoke projects its current 0-to-2 charge count;
its authoritative sequential recharge restores one missing charge every 15 seconds.
`chargeRechargeLeft` is owner-private and present while a charged ability is regenerating. It is
the authoritative ticks remaining for the next charge and drives the command card's existing clock
animation independently of activation cooldown readiness.
`autocastEnabled` is present for Mortar Team `mortarFire` so the command card can display and
toggle autocast without exposing enemy data.
`activeObjectId`, `availableTick`, and `expiresIn` are owner-only per-caster affordance fields for
two-stage or persistent ability state such as Ekat's return marker, Magic Anchor, and the active
Breakthrough aura duration on the casting Command Car. `lockoutUntilTick` is available for
owner-only ability lockouts; Magic Anchor does not currently use a destruction lockout because the
anchor is not attackable.
`breakthroughTicks` is present only while the affected visible unit has active Breakthrough speed
status; it is not caster identity. `breakthroughAuraTicks` is present only on a visible casting
Command Car while its authoritative aura remains active, so renderers can keep the bright radius
visible without selection or owner-private ability affordances. Owner snapshots also expose the
Command Car's `breakthrough`
ability cooldown and, while the caster's aura is active, its caster-only `expiresIn` through
`abilities`.
`weaponRangeTiles` is present for owner/allied Tank views and carries the current authoritative
default-weapon range, including the stationary range ramp. Enemy views omit it; static unit catalog
range remains the fallback for render-only range overlays.
`occupiedTrenchId` is present while a visible eligible infantry unit is actively stopped in a
trench. It names the neutral trench terrain id already projected through `trenches`; it is omitted
while the unit is digging in, slotting is unavailable, merely near a trench, or moving out.
`panzerfaustLoaded` is present for visible Panzerfaust units. It is `true` until launch and `false`
for the rest of that unit's lifetime after launch, so the client swaps from loaded launcher art to
normal Rifleman art. It is omitted for Riflemen and all other entities.
`scoutPlane` is owner/full-world diagnostic private state for `scout_plane` entities. It carries
the current orbit center and source Command Car id; enemy projections that can see the plane omit
this state. Scout Plane
entities are not selectable or commandable by normal clients, and runtime movement is driven by the
server-side Command Car ability lifecycle.
`visionOnly` is a legacy/special projection flag for non-owned units/buildings that are sent as
render-only intel rather than normal visibility. Current lingering death sight is ordinary
temporary team sight and does not set `visionOnly`. Clients must not select `visionOnly` entities.
In `n.flags`, bit 0 = `slowTick` and bit 1 = `headOfLine`.
The optional compact `n` prediction fields are present only for live active player snapshots.
Spectators, replay viewers, and dev full-world viewers omit prediction acknowledgement metadata.
`debugPath` is present only when the room's projection policy enables movement-path diagnostics for
that recipient and only while the unit has remaining movement waypoints. Dev scenario rooms may
enable full projected movement paths. It carries `{ waypoints, goal, lastRepathTick, stuckTicks,
staticBlockedTicks, totalWaypoints }`, where `waypoints` are remaining `{x, y}` world-pixel path
points in traversal order and `waypoints[0]` is the current movement target. The compact slot
encodes this as
`[waypoints, goal, lastRepathTick, stuckTicks, staticBlockedTicks, totalWaypoints]`, with points
encoded as `[x, y]`; `waypoints` is capped at 128 entries for transport.

`AbilityObject`: `{ id, owner, ability, kind, x, y, expiresIn?, sourceCasterId?, ownerState? }`.
The compact `ao` slot uses ability ids from the existing ability code table and
`abilityObject.kind` codes from the table above. `ownerState` is owner/spectator/full-world data
encoded as `[earliestReturnTick?, hp?, radius?, destroyedLockoutTicks?, distanceTraveled?, ticksOut?]`.
Magic Anchor currently fills only `radius`; the hp and destroyed-lockout slots are retained as
optional compact slots for compatibility.
Normal enemy snapshots receive only the public object fields needed to render a marker at a visible
position.

`Trench`: `{ id, x, y, radiusTiles }`. Trenches are neutral persistent battlefield terrain, not
buildable entities, and do not carry an owner field. The `id` is stable for the trench lifetime,
`x`/`y` are the world-pixel center, and `radiusTiles` is the footprint/render radius used by later
slotting and rendering code. Normal active-player snapshots include trenches whose footprint is in
that recipient's current living-team fog, plus trench terrain the recipient has already discovered.
Spectator, replay, and lab selected-perspective snapshots use the selected real players' current
fog and discovered terrain memory; full-world dev snapshots include every trench. Remembered
trench terrain is terrain-only: it exposes no creator, owner, occupant, or current hidden unit
state. Clients treat each snapshot's `trenches` field as the complete current trench-terrain set
for that recipient; a missing or empty field clears prior client trench terrain. Rendering uses the
snapshot data below fog, so reconnects, replay seeks, and fog-memory refreshes restore trench ground
from the server instead of from local visual history.

`RememberedBuilding`: `{ id, owner, kind, x, y, footprint, observedTick }`. These records are
recipient-only last-seen enemy building memory, refreshed from team-current actionable observations
and sent only when the building is not currently projected as a live visible entity. They are stale
intel for normal building rendering below the fog overlay and coordinate targeting context; clients
must not make them selectable live entities or issue entity-targeted commands against them.
`footprint` is an array of `[tileX, tileY]` cells from the last visible state. The record
intentionally omits hidden live HP, current build progress, and destruction state. Artillery Fire
remains a world-coordinate ability; remembered buildings help the player know where to aim but do
not become target ids. Its selected radius and server-authoritative locked center do not make
remembered buildings target ids.
Union views build remembered buildings from the selected real
players' memory stores. If more than one selected player has stale memory for the same building id,
the server sends one record: the newest `observedTick` wins, with selected-player order as the
deterministic tie-breaker. This avoids adding a memory-source wire field while keeping one-player
replay vision isolated to that player's memory.

`ResourceDelta`: `{ id: u32, remaining: u32 }`. Resource node positions/kinds are static and come
from `start.map.resources`; clients keep last-known `remaining` locally. The server sends
`remaining` updates only for resource nodes currently visible to that recipient (dev full-world
watch rooms receive all resource updates).

`SmokeCloud`: `{ id: u32, x: f32, y: f32, radiusTiles: f32, expiresIn: u16 }`. Smoke clouds are
neutral world effects, not entities. `radiusTiles` and `expiresIn` reflect the cast-time smoke
rules, including completed `smoke_plus` research. Normal player snapshots include only clouds that
have at least one currently visible tile after smoke-suppressed team fog is recomputed, plus any
cloud currently containing one of that player's allied non-resource entities; spectator/dev full-world
snapshots may include all active clouds. Smoke-covered enemy units/buildings, target ids, death
events, and positioned notices remain fog-gated and are withheld when smoke hides the position.

`Entity` (lean; omit fields that don't apply):
```
{
  id: u32,
  owner: u32,                    // 0 = neutral (resources), else player id
  kind: string,                  // EntityKind: "worker","golem","rifleman","panzerfaust","machine_gunner","anti_tank_gun","mortar_team","artillery","scout_car","scout_plane","tank","command_car","ekat","city_centre","zamok","depot","barracks","training_centre","research_complex","factory","steelworks","tank_trap","pump_jack"
  x: f32, y: f32,                // world px (center)
  hp: u32, maxHp: u32,
  state: string,                 // "idle","move","attack","gather","build","train","construct","dead"
  facing?: f32,                  // radians, for unit body/hull orientation (optional)
  weaponFacing?: f32,            // radians, for independent weapon/barrel orientation (optional)
  // production buildings:
  prodKind?: string,             // unit currently being produced
  prodUpgrade?: string,          // upgrade currently being researched
  prodUpgradeQueue?: string[],   // owner/allied; ordered authoritative research queue
  prodProgress?: f32,            // 0..1
  prodQueue?: u32,               // queued count (including the in-progress one)
  prodRepeatKinds?: string[],    // owner/allied; ordered standing unit list for an empty queue
  prodScoutPlaneQueued?: bool,   // owner/allied; true if any queued item is a Scout Plane
  // buildings under construction:
  buildProgress?: f32,           // 0..1; when present and <1, render as scaffolding
  buildActive?: bool,            // owner-only; true when server advanced this scaffold this tick
  extractorActive?: bool,        // visible completed Pump Jack is currently extracting
  // Tank Trap deconstruction:
  deconstructProgress?: f32,     // 1..0 remaining dismantle progress; render reverse progress bar
  // gatherers:
  latchedNode?: u32,             // node id the Worker/Golem is currently harvesting (attached mining)
  // combat feedback:
  targetId?: u32,                // current attack target, for drawing tracers
  weaponRangeTiles?: f32,        // owner/allied Tanks only; current authoritative weapon range
  occupiedTrenchId?: u32,        // visible eligible infantry only while actively stopped in a trench
  panzerfaustLoaded?: bool,       // Panzerfaust units only; false after disposable launch
  scoutPlane?: {                 // owner/full-world diagnostics only; enemies omit this private state
    orbitCenter?: [f32, f32],
    sourceCommandCar?: u32
  },
  setupState?: string,           // machine_gunner/anti_tank_gun/mortar_team/artillery only:
                                  // "packed","setting_up","deployed","tearing_down"
  // unit-producing buildings:
  rally?: [f32, f32],            // first rally point (world px); owner-private except full-world diagnostics
  rallyPlan?: [                  // building rally stages; owner-private except full-world diagnostics
    { kind: "move"|"attackMove", x: f32, y: f32 }
  ],
  // tanks:
  oilUsed?: f32,                 // lifetime oil burned by movement, in resource units
  setupFacing?: f32,             // anti_tank_gun/mortar_team/artillery only: owner/allied deployed arc center; appended after oilUsed in compact snapshots
  orderPlan?: [                  // current + queued order stages; owner-private except full-world diagnostics
    { kind: "move"|"attackMove"|"holdPosition"|"attack"|"gather"|"build"|"deconstruct"|"smoke"|"mortarFire"|"pointFire"|"blanketFire"|"breakthrough"|"scoutPlane"|"dismissScoutPlane"|"ekatTeleport"|"ekatLineShot"|"ekatMagicAnchor"|"ekatConsumeGolem"|"setupAntiTankGuns", x: f32, y: f32 }
  ],
  chargeCooldownLeft?: u16,      // legacy; no longer projected by current server
  abilities?: [                  // owner-only ability affordance/cooldown data
    { ability: "smoke"|"mortarFire"|"pointFire"|"blanketFire"|"breakthrough"|"scoutPlane"|"dismissScoutPlane"|"ekatTeleport"|"ekatLineShot"|"ekatMagicAnchor"|"ekatConsumeGolem",
      cooldownLeft: u16, remainingUses?: u16, autocastEnabled?: bool,
      activeObjectId?: u32, availableTick?: u32, lockoutUntilTick?: u32, expiresIn?: u16 }
  ],
  breakthroughTicks?: u16,       // active Breakthrough speed status; visible only with the entity
  breakthroughAuraTicks?: u16,   // casting Command Car only; active aura duration, visible with caster
  visionOnly?: bool,             // legacy/special render-only intel; current death vision does not set this
  debugPath?: {                  // diagnostic policy only; remaining movement path; owner-only unless policy says full projected diagnostics
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
  toPos?: [f32, f32],
  weaponKind?: "worker_tools"|"golem_fists"|"rifleman_rifle"|"machine_gunner_mg"|"scout_car_mg"|"anti_tank_gun"|"panzerfaust_loaded_shot"|"mortar_team_mortar"|"artillery_gun"|"tank_cannon"|"tank_coax" } // feedback hint; unknown/missing hints fall back to attacker kind
{ e: "overpenetration", to: u32 }               // secondary penetration damage; no tracer/audio
{ e: "miss", to: u32 }                          // direct shot missed the target; no position
{ e: "death",  id: u32, x: f32, y: f32, kind } // for death poofs
{ e: "build",  id: u32, kind: string }         // building completed
{ e: "smokeLaunch", fromX: f32, fromY: f32, toX: f32, toY: f32, delayTicks: u32 }
{ e: "mortarLaunch", from: u32, fromX: f32, fromY: f32, toX: f32, toY: f32, radiusTiles: f32, delayTicks: u32 }
{ e: "mortarImpact", from?: u32, x: f32, y: f32, radiusTiles: f32,
  reveal?: { owner: u32, kind: string, x: f32, y: f32, facing?: f32, weaponFacing?: f32, setupState?: string } }
{ e: "artilleryTarget", from: u32, x: f32, y: f32, radiusTiles: f32, delayTicks: u32 }
{ e: "artilleryFiring", owner: u32, x: f32, y: f32, facing: f32 }
{ e: "artilleryImpact", x: f32, y: f32, radiusTiles: f32 }
{ e: "panzerfaustLaunch", from: u32, fromX: f32, fromY: f32, toX: f32, toY: f32, delayTicks: u32 }
{ e: "panzerfaustImpact", x: f32, y: f32 }
{ e: "notice", msg: string, severity?: "info"|"warn"|"alert", x?: f32, y?: f32 }
```
Notices default to `severity: "info"` with no position. `alert:`-prefixed notice ids are
gameplay alerts: active-player clients play alert audio and ping the minimap at `(x, y)` when
present, or pulse the minimap border when absent. Replay viewers and live spectators keep the visual
alert feedback but suppress notice alert audio. `alert:under_attack` is emitted at the damaged enemy
unit's position to the victim owner only; same-team recipients may still see the attack event through
shared vision, but they do not receive teammate under-attack alerts. Same-team friendly-fire damage
does not emit under-attack alerts. Unit attack events are sent to the attacker's team and to enemy
recipients whose team can currently see the shooter or target point. Shots resolved against Tank
Traps carry no shooter `reveal` and grant no actionable firing reveal to the Tank Trap's team. A
missed direct shot
additionally emits a `miss` event to the same recipient set, carrying only the receiving entity id;
clients anchor the tiny text feedback to the already projected entity and ignore the event if the
target is absent. Attack events include `reveal` so a shooter
that fires from fog can be rendered briefly as a semi-transparent, non-interactive silhouette above
the fog overlay; Anti-Tank Gun reveals additionally become the actionable snapshot visibility
described in §2.4. `toPos` lets tracers draw even when the hit target is no longer in the snapshot.
`weaponKind` is a closed, fog-safe feedback hint for attack events that would already be projected;
current default direct-fire attacks emit their default weapon id, Tanks emit `tank_cannon`, and
artillery self-reveal attacks emit `artillery_gun`. Clients must tolerate missing or unknown
weapon ids by falling back to the legacy attacker-kind feedback path. Overpenetration events are
sent for secondary entities damaged behind the primary target. They carry only the damaged entity id
and do not imply a separate fired shot, muzzle flash, tracer, shooter reveal, weapon recoil, or
attack sound.
Full-world dev-watch projections attach a deterministic
deduplicated union of the per-player event buckets so transient effects match the exposed world
state. Normal active-player views keep player-only event delivery. Live spectators and selected
player/team replay or lab views union only the real-player buckets selected by the view. Live
spectator event unions filter per-player, position-free, non-alert notices such as command
rejections or economy toasts; room-owned recipient notices, including late-spectator joins, are
appended separately after projection.

When a normal live match accepts a mid-match spectator attach, the server queues a position-free
info notice for every already-connected active player and spectator: `<name> has joined the match as
a spectator`. The joining spectator is excluded. The name comes from the server-sanitized join name
with control-only/empty results displayed as `Commander`; the message is a normal `notice` event
without an `alert:` prefix. Delivery is one-shot on each recipient's next live snapshot. If the match
is live-paused, snapshot fanout is skipped and the notice remains queued until the next emitted live
snapshot after unpause.
Death events are sent to the dead entity's team and to enemy recipients whose team can currently see
the death position; smoke-covered hidden death positions are withheld. Build completion events are
sent to the completed building's team and to enemy recipients whose team currently sees the site.
Smoke launch events are team-visible local feedback for the scout-car canister animation; enemies
do not receive hidden smoke launch data. The authoritative smoke cloud appears later in `smokes`
after the reported launch delay. Mortar launch events are always sent to the firing team, with
shooter id, shell origin, impact point, radius, and delay so clients can draw launch dust, recoil,
the projectile, and the warning marker until detonation. Autocast mortar launch events are also
sent to enemy recipients whose team currently sees the mortar; manual launch events remain hidden
from enemies without current team sight, so they do not receive pre-impact warning markers. Mortar
impact events are sent to the firing team, to enemy recipients with team-current visibility at the
impact point, and to enemy players whose entities were damaged by the shell. An enemy damaged victim
owner receives `from` + `reveal` so the attacking mortar can be shown briefly above fog after
indirect fire lands. Allied or owned entities can still take mortar splash damage, but that damage
is unattributed and does not reveal the firing mortar as hostile. Enemy players do not receive
hidden mortar launch data or hidden mortar impact markers unless their entities were hit or their
team sees the relevant point.
Artillery target events are sent to the firing team so enemies never receive pre-impact markers,
even if they have vision of the gun. Fire reports the final deterministic shell target after the
uniform selected-area sample, not the raw clicked center. The
`from` id lets allied clients recoil the specific gun and draw launch
dust. Every player receives a visual-only `artilleryFiring` event with the firing owner, shooter
position, and facing so the minimap can show a small global artillery firing marker; it does not
carry the shooter entity id, target point, terrain, or exploration. Separately, the server grants
every enemy player actionable temporary live fog on the firing gun, subject to normal smoke
suppression, so it is projected as a normal world entity and can be targeted during the firing
reveal window.
Enemy players also receive a visual-only `attack` event with a shooter `reveal` when their team
currently sees the firing gun, so the gun can be shown briefly without revealing terrain,
exploration, or the target point. Artillery impact events are sent to the firing team and to enemy
recipients whose team currently sees the impact point; they do not reveal terrain, update
exploration, or carry entity visibility. Artillery impact damage follows the same support-fire
friendly-fire attribution rule as mortar splash: owned and allied entities in the radius can take
damage, but same-team damage does not produce hostile reveal, under-attack, or score attribution.
Panzerfaust launch events are emitted by the one-shot anti-tank runtime. They carry the firing
unit id, launch point, intended visual endpoint, and travel delay, but never carry the target entity id.
Launch events are sent to the firing team and to enemy recipients whose
team-current fog can see the shooter or launch point; the endpoint must be withheld unless it is
visible to that recipient or otherwise already safe through the recipient's projection. Panzerfaust
impact events carry only the impact point and are sent to the firing team and to enemy recipients
whose team-current fog can see that point; they do not imply damage, target identity, terrain
reveal, or exploration. The firing entity remains a Panzerfaust throughout; `panzerfaustLoaded` flips
to false at launch while the detached projectile resolves through the launch/impact events.
Events are best-effort visual flavor; the client must not depend on receiving them.

#### 2.5.1 Projection contract summary

Projection modes select a view and an event policy separately from command authority. Normal
active-player views use the recipient's living-team current fog for entity
visibility but keep resources, upgrades, ability affordances, command authority, order plans, and
rally plans exact-owner-only. Lab operator command surfaces are the exception on the client side:
they are still spectator-shaped projections, but `issueCommandAs` names the selected real player as
the authoritative command issuer and rejects mixed-owner selections.

Live spectator, replay, dev-watch, and Lab views pass one shared `ObserverView` through one
projection seam: `Omniscient` or a non-empty set of real player ids. Omniscient means the complete
world, all-owner private detail, all visible and explored tiles, and the full event union; it is
not an all-player fog union. Selected ids drive effective team fog, `visibleTiles`,
`exploredTiles`, entities,
remembered-building memory, private detail for explicitly selected owners, `playerResources`, and
event unions. No observer view creates a command issuer. Lab `issueCommandAs` remains the only
cross-player command path and is independently authorized.
`artilleryFiring` remains an intentionally global visual event for minimap firing
markers, while artillery's actionable world reveal is modeled separately as temporary live fog on
enemy player grids without granting target-point or surrounding-terrain visibility.
`worldCombatPosition` is the second explicit global exception. Every projection receives the
identical 32-tile-quantized combat-area point. It intentionally conveys broad direction while
withholding exact location, owner, team, entity id, weapon kind, count, composition, and live
cadence. It may pan generic constant-gain audio only and never changes fog, exploration, targeting,
or entity projection.

When adding a projection-affecting field or event, use
[docs/projection-audit-checklist.md](../projection-audit-checklist.md) and update this section's
mode/event policy if the new data does not fit an existing row.

### 2.6 Room time state and vision selection

`roomTimeState` is a reliable server message that carries the shared room-controlled time
cursor/state. Replay rooms send it for playback cursor changes; dev scenario watch rooms and lab
rooms also send it after pause/resume and one-tick step controls so clients can confirm the
authoritative room-time speed and tick. Clients keep a pending room-time command across stale
authoritative frames and clear or recover its timeout notice when a later state confirms the
command. Lab rooms also send it after timeline baseline resets and new timeline keyframes, accepted
seeks, and future-history truncation:
```
{
  t: "roomTimeState",
  currentTick: u32,
  durationTicks: u32,
  keyframeTicks: u32[],
  speed: f32,
  paused: bool,
  ended: bool,
  controllerId?: u32
}
```
`keyframeTicks` lists the replay or lab keyframes the server has recorded so far. Replay and lab
clients may display them as seek marks, but a seek target is not limited to these ticks; the server
restores the nearest recorded keyframe at or before the requested tick and fast-forwards from there.
For schema 3 replay artifacts that begin from a checkpoint, the restored checkpoint tick is the
minimum playback tick; earlier seek targets are clamped to that tick.
Lab rooms expose recorded baseline and periodic keyframe ticks through the same field, with
`durationTicks` set to the current maximum retained lab tick. Lab history is bounded by retained
room-local keyframes and recorded entries; seek requests outside retained history are rejected with
`error` instead of rebuilding from discarded state.

Before an accepted replay seek starts its synchronous rebuild, the room broadcasts
`roomTimeSeekStarted` with the authoritative current and clamped target ticks. Every replay viewer,
including the controller, presents the direction and tick distance as seconds so a temporarily
frozen replay is visibly busy until the rebuilt `start`/`roomTimeState` sequence arrives.

`livePauseState` is a reliable server message that carries the authoritative live-match pause
state. Normal live and branch-live match recipients receive it after `start` and after accepted or
rejected pause/unpause transitions. Pause-capable active players and live spectators receive their
own remaining-count value plus pause/unpause authority:
```
{
  t: "livePauseState",
  paused: bool,
  pausedBy?: u32,
  pausesRemaining?: u8,
  pauseLimit: u8,
  canPause?: bool,
  canUnpause?: bool
}
```
Each active seat and live spectator connection has three successful pause starts per match. The
server decrements the count only when a request changes the room from unpaused to paused; any
pause-capable live recipient can unpause. While live
pause is active the room task skips the live simulation tick branch, so AI thinking, command-ack
consumption, `Game::tick`, live snapshot fanout, and defeat checks do not advance, while reliable
control-plane messages such as ping/pong, net reports, Give up, disconnect handling, and unpause
still run. Room-owned recipient notices queued during pause, such as late-spectator join notices,
are delivered on the next emitted live snapshot after unpause rather than through a separate
reliable message.

`VisionSelectionRequest` selects fog/vision per viewer:
```
{ mode: "all" }
{ mode: "player", playerId: u32 }
{ mode: "players", playerIds: u32[] }
{ mode: "omniscient" }
```
The server rejects unknown player ids, empty subsets, duplicate subset ids, active-player requests,
and requests outside a privileged observer surface. Selection is per connection. `mode: "all"`
uses the union of every active player's effective team fog, while `mode: "omniscient"` exposes the
complete world and every owner's private details. The all-player union is the default for live
spectators, replay, dev-watch, and Lab; omniscient remains available as an explicit selection. A
selected single/multi-player view uses the real selected owners and their effective team fog.
Owner-only details remain limited to explicitly selected owners, even when team fog reveals an
ally.

`LabClientOp` is tagged by `op`:
```
{ op: "exportMap" }
{ op: "spawnEntities", spawns: [{ owner: u32, kind: string, x: f32, y: f32, completed?: bool }] }
{ op: "applyUpdates", updates: [
  { operation: "move", entityId: u32, x: f32, y: f32 } |
  { operation: "reassign", entityId: u32, owner: u32 } |
  { operation: "resources", playerId: u32, steel: u32, oil: u32 } |
  { operation: "research", playerId: u32, upgrade: string, completed?: bool } |
  { operation: "godMode", playerId: u32, enabled?: bool }
] }
{ op: "deleteEntities", entityIds: u32[] }
// Existing singular browser/UI operations remain accepted for compatibility.
{ op: "spawnEntity", owner: u32, kind: string, x: f32, y: f32, completed?: bool }
{ op: "deleteEntity", entityId: u32 }
{ op: "moveEntity", entityId: u32, x: f32, y: f32 }
{ op: "setEntityOwner", entityId: u32, owner: u32 }
{ op: "setPlayerResources", playerId: u32, steel: u32, oil: u32 }
{ op: "setPlayerGodMode", playerId: u32, enabled: bool }
{ op: "setCompletedResearch", playerId: u32, upgrade: string, completed: bool }
{ op: "setVision", vision: LabVisionMode }
{ op: "issueCommandAs", playerId: u32, cmd: Command, ignoreCommandLimits?: bool }
{ op: "exportScenario", name?: string }
{ op: "importScenario", scenario: LabScenarioPayload }
{ op: "validateScenario", metadata: LabScenarioAuthoringMetadata }
```
`LabVisionMode` remains compatibility metadata for authored Lab scenarios and the legacy
`setVision` Lab operation. At runtime, `all` maps to the shared all-player union view and `team`
maps to a shared selected-player view containing the current real players on that team. Unknown
team selections are rejected. New UI selection uses `setVisionSelection`, just like live
spectators, replay, and dev-watch. Each connection's shared observer view drives the snapshot body,
events, remembered-building memory, `playerResources`, and completed research. `labState.vision`
and `start.lab.vision` retain the compatibility metadata; they are not command authority and must
not be used to infer which player the operator can command.
Plural mutation arrays contain 1–400 items and commit atomically. Success returns
`outcome.items: [{index, outcome}]` in input order, with each nested `outcome` retaining the
corresponding singular shape. Rejection returns `failedIndex`; placement rejection also returns
`details: {attempted:{x,y}, blockers:[...], suggestions:[...]}`. Blocker records use `kind` values
`entity`, `feature`, `terrain`, or `boundary`; features distinguish `building` and `resource` and
carry the blocking entity id/kind. Suggestions contain no more than eight deterministic points and
building points are authoritative snapped centers.
`issueCommandAs` queues a normal gameplay command as the selected player only when all selected
units belong to that player; mixed-owner selections are rejected instead of partitioned. When
`ignoreCommandLimits` is true, the lab command bypasses the normal command-supply budget and uses
the larger bounded lab command window instead of the ordinary live-player unit-id window. A
successful `labResult` carries
`outcome: {accepted:true, admission:"enqueued", playerId, queuedAtTick}` as an authoritative enqueue
receipt. `queuedAtTick` is the simulation tick at admission; the receipt does not claim that command
planning produced an effect or that the command has already completed.
`setPlayerGodMode` is lab-only room state: enabled players' units and buildings ignore incoming
damage, while resources keep normal damage behavior. The current enabled player ids are mirrored in
`start.lab.godModePlayers` and `labState.godModePlayers`.
`exportMap` returns `outcome.map: LabMapDraft` from the authoritative Lab map and deliberately
excludes entities, orders, player resources, fog, room time, operation history, and replay state.
It is the only Lab-to-Map-Editor transition payload.
Map mutation is not a `LabClientOp`; `exportMap` is read-only. The dedicated editor uses a bounded HTTP handoff contract:
```
POST /api/map-handoffs
{
  destination: "lab" | "editor",
  authoredMap: AuthoredMapV3,
  materializedMap: { name: string, size: u32, terrain: u8[], starts: LabMapTile[], baseSites: LabMapTile[] }
}
-> { handoffId: 32-lowercase-hex, expiresInMs: 120000 }

POST /api/map-handoffs/{handoffId}
-> { destination: "lab", room: privateLabRoom }
 | { destination: "editor", authoredMap: AuthoredMapV3 }
```
`AuthoredMapV3` has flat `startLocations` and `baseSites` arrays. Start locations determine the
supported player count; every base site is a permanent resource location, including unoccupied
start locations. Creation validates the complete authored-map schema and binds its terrain and flat
locations to `materializedMap` before storing it. Records are capped at 64, expire after two
minutes, and are removed on the first consume; unknown, expired, or already-used ids return HTTP 410.
The map body never appears in a URL. Consumption uses POST so browser or intermediary prefetching
cannot burn the one-use record. Consuming a Lab-directed record creates a private Lab from the
validated materialized map before the browser joins; an unjoined room has a short empty-room lease
and its reserved name cannot recreate a fallback room after disposal. Its only initial `start` is tick
zero on the edited map. Editor-directed records return only the map; simulation entities, orders, resources, fog,
timeline state, and replay history are not part of the contract.

`LabScenarioPayload` accepts only the current checkpoint-backed setup container. Setup exports,
validation previews, imports, and bundled catalog assets use `LabCheckpointScenarioV1`:
```
{
  schemaVersion: 1,
  kind: "labCheckpointScenario",
  name: string,
  seed: u32,
  map: {
    name: string,
    schemaVersion: u32,
    contentHash: string,
    materializedHash: string,
    data: {
      size: u32,
      terrain: u8[],
      starts: [{ x: u32, y: u32 }],
      baseSites: [{ x: u32, y: u32 }]
    }
  },
  metadata: {
    exportedTick: u32,
    lab: {
      vision: LabVisionMode,
      godModePlayers?: u32[],
      initialCamera?: { centerX: u32, centerY: u32 }
    },
    sourceEntityIdMap?: [{ oldId: u32, newId: u32 }]
  },
  checkpointPayload: string // GameCheckpointV1 JSON text
}
```
The map body is a setup-container sibling of the checkpoint payload, not part of
`GameCheckpointV1`. Import validates the setup map data and materialized hash, the embedded
checkpoint `mapBinding`, player/team/resource/research/entity/count bounds, lab metadata,
checkpoint byte limits, `metadata.lab.godModePlayers`, optional `metadata.lab.initialCamera`
inside the restored map's world bounds, and a one-to-one `sourceEntityIdMap` whose `newId` values
reference restored entities before replacing the live lab game. `sourceEntityIdMap` is returned as
`outcome.entityIdMap` on checkpoint import so imports and fresh exports preserve existing id-remap
callers. Legacy labScenario JSON is rejected by the checkpoint-only payload parser before it can
mutate a lab room.

Setup export returns `{ scenario: LabCheckpointScenarioV1 }` in `labResult.outcome` using the requesting
operator's current lab vision in `metadata.lab.vision` and the current room god-mode player ids in
`metadata.lab.godModePlayers`. Checkpoint-backed import applies setup vision to the requester and
future join default without overwriting already connected collaborators, restores god mode from the
embedded checkpoint payload, and returns the container `sourceEntityIdMap`.
`validateScenario` exports the current authoritative lab `Game` as a checkpoint-backed setup,
applies authoring metadata, pretty formats the setup JSON, checks duplicate catalog
ids/filenames, id-matched filenames, manifest limits, setup entity count, and setup JSON byte
limits, validates map metadata and checkpoint map binding, and restores through the same lab `Game`
API without mutating the room or writing server state.
`LabScenarioAuthoringMetadata` is:
```
{
  slug: string,        // future catalog id and filename stem, max 48 catalog-safe bytes
  name: string,        // exported setup name, max 80 bytes
  title: string,       // catalog title, max 96 bytes
  description: string, // catalog description, max 320 bytes
  tags?: string[]      // up to 8 catalog-safe tags, max 32 bytes each
}
```
`LabReplayArtifactV1` is the portable lab-session artifact contract owned by `rts-protocol`
because it is a shareable JSON file shape and reuses the public `Command` DTO for issued commands.
It is not sim-private `LabOp`, not `LabSession.operationLog`, and not the room-local
`LabTimeline` keyframe list. The first schema is latest-version-only:
```
{
  schema: "rts.labReplay",
  schemaVersion: 1,
  kind: "labReplay",
  serverBuildSha: string,
  authoring: {
    name: string,              // max 120 bytes
    author?: string,           // max 80 bytes
    createdAtUnixMs?: u64,
    description?: string,      // max 2000 bytes
    tags?: string[]            // max 16, each max 32 safe ASCII bytes
  },
  initialSetup: LabCheckpointScenarioV1,
  timeline: {
    initialTick: u32,          // must match initialSetup.metadata.exportedTick
    durationTicks: u32,        // final replayable tick
    keyframeIntervalTicks: 2000
  },
  operations: [{
    sequence: u64,             // contiguous from 0
    tick: u32,                 // nondecreasing and within timeline bounds
    requestId: u32,            // nonzero original lab request id
    operatorId: u32,           // nonzero room connection id, not necessarily a game player
    op: LabReplayOperation
  }]
}
```
Whole artifacts are capped at 8 MiB. The operation stream is capped at 50,000 entries, each
non-setup operation payload is capped at 64 KiB, and embedded `GameCheckpointV1` text payloads in
the initial setup are capped at 4 MiB. A lab replay may be saved/opened by Interact's bounded
local artifact path; it is not carried through the current WebSocket `lab` request envelope because
long lab sessions can exceed that control frame budget. The daemon-started private server enables
a loopback-only handoff only when `RTS_INTERACT_LAB_ARTIFACT_CAPABILITY` contains the driver's
random 256-bit capability. Requests echo that capability and address artifacts by expiring opaque
transfer id. Production startup leaves the seam unavailable. Export/import still execute on the
single-owner room task, so accepted ticks, operation order, truncation, and rebuild validation never
move into the browser or daemon.

`LabReplayOperation` deliberately promotes only replayable lab state changes:
```
{ op: "spawnEntities", spawns: [{ owner: u32, kind: string, x: f32, y: f32, completed?: bool }] }
{ op: "applyUpdates", updates: [LabUpdateSpec] }
{ op: "deleteEntities", entityIds: u32[] }
// Reader-only compatibility for artifacts written before plural operations:
{ op: "spawnEntity", owner: u32, kind: string, x: f32, y: f32, completed?: bool }
{ op: "deleteEntity", entityId: u32 }
{ op: "moveEntity", entityId: u32, x: f32, y: f32 }
{ op: "setEntityOwner", entityId: u32, owner: u32 }
{ op: "setPlayerResources", playerId: u32, steel: u32, oil: u32 }
{ op: "setPlayerGodMode", playerId: u32, enabled: bool }
{ op: "setCompletedResearch", playerId: u32, upgrade: string, completed: bool }
{ op: "issueCommandAs", playerId: u32, cmd: Command, ignoreCommandLimits?: bool }
```
New Interact writes use the plural replay vocabulary even for one-item requests. Existing
schema-version-1 artifacts containing singular spawn/delete/move/owner/player operations remain
readable.
`setVision` is excluded because it is per-operator projection metadata; reopening a lab replay
starts from `initialSetup.metadata.lab.vision` and connected viewers may choose their own current
lab vision afterward. `exportScenario` and `validateScenario` are checkpoint
setup UI/control requests and never enter the durable stream. Checkpoint setup import uses rebase semantics: the
artifact replaces `initialSetup` with the imported `LabCheckpointScenarioV1` and clears prior
operations instead of storing an import operation. That keeps later entity references unambiguous:
after a rebase, operation entity ids refer to the rebased setup's current ids, and
`initialSetup.metadata.sourceEntityIdMap` remains the only setup-import id-remap record. Seeking
into the past and then accepting a new lab mutation truncates future operation entries just like
the room-local timeline does.

Lab replay import validates schema/kind/version, artifact bytes, authoring metadata, checkpoint
map binding, player/team ids, lab metadata, entity ids and allocator facts, operation count,
operation payload sizes, non-finite coordinates, stale entity references, bad player ids, command
unit caps (`256` normally, `4096` when `ignoreCommandLimits` is true), timeline order, and rejects
excluded session/setup/import operations before mutating a live lab game.

Accepted setup validation returns `{ summary, preview }` in `labResult.outcome`; `preview` includes
`manifestEntry`, `manifestPath`, `scenarioPath`, and deterministic `scenarioJson`. The preview is
returned to the requesting operator only; no lab request writes setup files or repository state.
`facing` serializes unit body orientation, and `weaponFacing` serializes stable combat
weapon/turret orientation for entities with combat state. `setUp` serializes only stable deployed
support-weapon state for machine gunners, anti-tank guns, mortar teams, and artillery; omitted
`setUp` defaults to false on import. `setupFacing` is the authoritative deployed support-weapon
direction in radians. `setupTarget` is a legacy finite world point fallback used to reconstruct
that direction for older exported scenarios; new exports include `setupFacing` and may also include
`setupTarget` for compatibility. A setup entity must include `setupFacing` or legacy
`setupTarget`; both setup direction fields are rejected when `setUp` is false. `order` and
`queuedOrders` persist stable active and future command intents, including artillery point and
blanket fire orders for preview scenarios. Setup/teardown transition timers, path progress,
gather/build execution progress, production/research queues, rally plans, attack cooldowns,
ability cooldowns/uses/lockouts, projectile runtime state, transient snapshot fields, fog
recipient projections, events, command logs, interpolation state, and lab operation result metadata
are intentionally omitted because lab setup fixtures are not mid-match savegames.

Reliable lab server messages:

| `t` | Fields | Meaning |
|-----|--------|---------|
| `labState` | `room`, `operatorId`, `role`, `vision`, `godModePlayers`, `dirty`, `operationCount` | Recipient-scoped lab control metadata plus room-scoped active god-mode player ids. World state still travels through `snapshot`. |
| `labResult` | `requestId`, `ok`, `op`, `error?`, `outcome?` | Targeted reply for every lab request accepted by the room task. Rejected requests include `error`; accepted setup mutations may include typed outcome metadata such as `entityId`. |

Lab protocol deliberately omits seek controls, broad lab simulation flags such as globally disabled
damage, user-writable public setup storage, fine-grained multi-operator permissions, visual
iteration hot reload, and `/dev/scenario` migration. Bundled prebuilt setups are selected before
join through the HTTP lab setup catalog and direct lab URL `scenario=<id>` tokens, not through a
normal-lobby command or a lab client op. Pause, speed, step, and room-local timeline metadata use
the neutral room-time messages instead of overloading `LabClientOp`.

### 2.7 Observer analysis state

`observerAnalysis` is the wire tag for latest-only observer analysis overlay/tab data that cannot
be derived safely from the browser's current projected snapshot. The server delivers it on a
separate latest-only outbound lane behind snapshots; stale unsent observer-analysis payloads may be
replaced by newer payloads and must never block world-state snapshot delivery. In replay playback it
is sent to replay viewers after replay `start`/`roomTimeState`, after accepted seeks, after vision
selection changes, and during replay playback ticks. Live matches produce the same payload every
server tick, at the normal snapshot cadence, only when at least one spectator connection is present.
The server computes the live payload once per tick and sends it only to connections whose room
player state is `spectator: true`; active-player connections, including claimed branch-live seats,
must not receive this message.
```
{
  t: "observerAnalysis",
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
      upgrades?: string[], // completed permanent research; omitted when empty
      unitsLost: [{ kind: string, count: u32, steelValue: u32, oilValue: u32 }],
      resourcesLost: { steel: u32, oil: u32 },
      resources: {
        lifetime: { steel: u32, oil: u32 },
        last5s: { steel: u32, oil: u32 },
        lastMinute: { steel: u32, oil: u32 }
      },
      aiDiagnostics?: {
        profileId: string,
        traceTick: u32,
        lines: string[]
      }
    }
  ],
  mapAnalysis?: {
    mapWidth: u32,
    mapHeight: u32,
    tileSize: u32,
    layers: [
      {
        id: string,
        label: string,
        defaultVisible: bool,
        primitives: [
          {
            kind: "tileRect",
            id: string,
            tileX: u32,
            tileY: u32,
            tileW: u32,
            tileH: u32,
            fill: "#rrggbb",
            stroke: "#rrggbb",
            alpha: f32,
            label?: string,
            tooltip?: string
          } | {
            kind: "marker",
            id: string,
            x: f32,
            y: f32,
            radius: f32,
            shape: "circle" | "diamond" | "square",
            color: "#rrggbb",
            label?: string,
            tooltip?: string
          } | {
            kind: "line",
            id: string,
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
            color: "#rrggbb",
            alpha: f32,
            width: f32,
            label?: string,
            tooltip?: string
          }
        ]
      }
    ]
  }
}
```
`players` lists every active observed player. `units` is the current living unit inventory by kind.
`production` has one row for each owned building with a non-empty unit or research queue; `progress`
is the front item's completion fraction and `queueDepth` is that queue's total item count.
`upgrades` lists that player's completed permanent research using protocol upgrade ids.
`steelValue` and `oilValue` are aggregate row values (`count * configured cost`), not per-unit
costs. `unitsLost` is the authoritative unit-death count by kind. `resourcesLost` is intentionally
narrow: the spent steel/oil value of units that died, matching `unitsLost`; it does not include
buildings, current spending, cancelled production, refunds, harvesting, or stockpile deltas.
`resources` is authoritative mined income: `lifetime` counts all worker/golem harvest and Pump Jack
payouts for that player, `last5s` counts payouts in the most recent five simulated seconds, and
`lastMinute` counts payouts in the most recent sixty simulated seconds. Starting resources, lab
resource edits, refunds, deconstruction refunds, spending, and current stockpile deltas are
excluded.
`aiDiagnostics`, when present, contains the latest bounded live AI decision trace for that player:
the selected profile id, the AI observation tick that produced the trace, and the formatted trace
lines from the AI decision manager.
`mapAnalysis`, when present, contains static AI-owned map-analysis overlay primitives built from
public start-payload terrain, starts, and resource nodes, plus optional live AI plan overlays for
spectators. Live AI-vs-AI spectator diagnostics use it to draw region fills, choke bands with
approach markers, base markers, resource-cluster markers, and AI intent lines/markers such as turtle
defended choke lines, Machine Gunner slots, Anti-Tank Gun backlines, and setup-facing rays. `label`
is short always-visible overlay text; `tooltip` is longer human-readable hover text explaining what
the primitive represents. `mapAnalysis` is optional because replay analysis and non-AI live rooms do
not currently own an AI controller cache for this data.

Observer analysis follows each viewer's observer selection. Narrowed views contain only selected
player rows and omit unscoped `mapAnalysis`; omniscient views retain all player rows and map layers.
It is observer-only data for analysis overlays, not an active-player information surface.
Replay playback recomputes the payload from the current authoritative replay `Game` state after
normal playback ticks and after `ReplaySession::rebuild_to()` restores a keyframe and fast-forwards
to the target tick. Analysis state is not serialized separately in `ReplayKeyframe`.

---
