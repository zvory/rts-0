## 3. Rust server — modules & the Game core API

Crate layout (`server/`):
```
Cargo.toml
src/
  main.rs        # tokio runtime, axum router: static files + /ws, room manager task
  protocol.rs    # server-shell protocol adapter shim; serde DTOs live in crates/protocol
  config.rs      # server-shell balance shim; authoritative values live in crates/rules
  lab_scenarios.rs # bundled lab scenario manifest loader and restore validator
  lobby/         # Lobby API plus room task, connection writers, snapshots, dev replay, crash replay
crates/
  contract/      # semantic DTOs shared below protocol/sim/server
  protocol/      # semantic wire DTOs and compact snapshot transport encoding
  rules/         # pure domain, balance, terrain, economy, and combat rules
  ai/            # live AI controllers, shared AI strategy core, and self-play harnesses
  sim/           # reusable simulation crate; no Tokio/Axum/server transport/AI dependency
    src/game/
    mod.rs       # Game struct + public API (the seam below)
    command.rs   # SimCommand domain commands + protocol translation helpers
    map.rs       # Map: handcrafted terrain asset loading, passability, base-site validation
    entity/      # Entity, EntityKind, Order state machines, grouped state, and EntityStore
    pathfinding.rs # A* over the tile grid, with optional turn-cost route shaping for tanks
    fog.rs       # per-player live visibility grids; snapshots union living teammate grids
    building_memory.rs # server-only per-player last-seen enemy building records
    systems.rs   # orchestrator: runs services in order each tick
    services/    # per-tick services: commands, order_planner, move_coordinator, movement (incl. unit collision), combat, economy, production, construction/deconstruction, death, entrenchment, occupancy, supply, pathing, geometry, standability, line_of_sight
    replay.rs    # tick-stamped command log replay harness for determinism checks
    src/rules/projection.rs # fog-gated entity/event projection seam
```

Dependency policy is part of the development contract:

- `rts-server` may depend on every lower crate and is the only package that owns Axum/Tokio room
  transport.
- `rts-ai` may depend on `rts-sim`, `rts-rules`, `rts-protocol`, and `rts-contract`; `rts-sim`
  must not know AI exists.
- `rts-sim` may depend on `rts-rules`, `rts-protocol`, and `rts-contract`; it must not import
  lobby, Axum, Tokio room machinery, or `rts-server`.
- `rts-protocol` may depend only on `rts-contract` among workspace crates.
- `rts-rules` and `rts-contract` are lower-layer crates and must stay free of server/sim imports.

`scripts/check-crate-boundaries.mjs` checks these edges in `cargo metadata` and scans lower crates
for server-only imports. `server/src/protocol.rs`, `server/src/config.rs`,
`server/crates/sim/src/protocol.rs`, `server/crates/sim/src/config.rs`, and
`server/crates/sim/src/rules/mod.rs` remain intentional adapters while call sites are migrated;
new code should prefer the owning crate directly when that does not make local code less clear.

### 3.1 `game::Game` public API (seam between `game` and `lobby`/`main`)
The `lobby`/networking layer interacts with the simulation ONLY through this surface.
`game-core` implementer: provide exactly these. `server-shell` implementer: call only these.
The server shell may also serve non-simulation HTTP routes such as `/wiki`, `/wiki/`, and
`/wiki/{*path}`. Those routes generate the wiki index from the allowlisted packaged Markdown docs
roots (`docs/context` and `docs/design`), route canonical `/wiki/docs/...` doc pages while
preserving legacy short aliases, rewrite allowlisted relative Markdown doc links under `/wiki` with
anchors preserved, render allowlisted docs Markdown as no-cache HTML, and do not call into `Game`.

```rust
pub struct Game { /* private */ }

impl Game {
    /// Create a match for the given players (ids + colors + names already assigned by lobby).
    /// Loads the hardcoded handcrafted map and assigns authored spawn slots to the ordered
    /// `PlayerInit` list. Singleton-team FFA matches keep the legacy behavior: select one matching
    /// authored layout by `seed`, shuffle that layout's complete main/naturals slots, then assign
    /// them in player order. Team matches evaluate every matching authored layout and slot
    /// assignment, preferring lower teammate spread, higher nearest enemy-team distance, lower
    /// exposure imbalance, and finally a deterministic seed-influenced tie break. Each selected
    /// slot keeps its authored main/naturals grouping, so maps can define different fair naturals
    /// for adjacent, cross, safe-base, or other spawn layouts and can grant more than one neutral
    /// expansion per player. Generated oil clusters place each oil patch on a unique passable tile
    /// center near the intended layout, keep one tile between oil patches, and reject sites whose
    /// Pump Jack footprint would collide with non-oil resources while preserving City Centre
    /// resource-distance bounds. Lab-restored oil nodes are normalized to passable tile centers and
    /// keep one free tile between oil patches.
    /// AI players are spawned as normal match participants; external AI orchestration owns any
    /// controller/profile selection.
    pub fn new(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a live lobby match where each AI chooses one strategy from the live profile pool.
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game;

    /// Compatibility helper for tests that still need explicit starting Steel/Oil. Production
    /// replay/lifecycle reconstruction should use per-player `PlayerStartingLoadout` records
    /// instead.
    pub fn new_with_starting_resources(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Compatibility helper for callers that still name AI profile setup plus explicit starting
    /// resources. AI controllers are owned by the caller, not by `Game`.
    pub fn new_with_starting_resources_and_random_ai_profiles(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Rebuild replay playback with recorded per-player faction loadouts, map, and map metadata.
    /// Commands are injected by the replay runtime rather than live AI controllers.
    pub fn new_for_replay_with_map_metadata(
        players: &[PlayerInit],
        seed: u32,
        starting_loadouts: &[PlayerStartingLoadout],
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game;

    /// Create a lab match around an already validated map/player setup. Lab rooms still use the
    /// normal `Game` simulation; the lab constructor only names the mode-specific setup seam.
    pub fn new_lab(players: &[PlayerInit], seed: u32, map: Map, map_metadata: MapMetadata) -> Game;

    /// Apply one typed, validated lab mutation and repair derived sim state before returning.
    /// Accepted operations can spawn, delete, move, reassign, set resources, set completed
    /// research, or restore a versioned lab scenario. Bad lab input returns `LabError`; room code
    /// must not mutate entity stores or player state directly.
    pub fn apply_lab_op(&mut self, op: lab::LabOp) -> Result<lab::LabOpOutcome, lab::LabError>;

    /// Export authoritative lab setup data as versioned JSON-friendly scenario state, without
    /// treating snapshots, fog, transient events, command logs, or room-owned lab metadata as the
    /// scenario format.
    pub fn export_lab_scenario(&self) -> lab::LabScenarioV1;

    /// Restore a versioned lab scenario through the same validation/repair path used by
    /// `apply_lab_op(LabOp::RestoreScenario(...))`, remapping scenario entity ids to fresh
    /// authoritative ids.
    pub fn restore_lab_scenario(
        &mut self,
        scenario: lab::LabScenarioV1,
    ) -> Result<lab::LabOpOutcome, lab::LabError>;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply domain command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand);

    /// Ordinary retreat commands for AI-owned workers hit on the previous tick.
    pub fn worker_retreat_commands_for(&self, player: u32) -> Vec<SimCommand>;

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick. Entity visibility and
    /// visibleTiles use the union of living teammates' current fog; resources/upgrades stay local.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Same projection as `snapshot_for`, with explicit room-projection diagnostic options such as
    /// owner-only movement paths. The default `snapshot_for` includes no movement diagnostics.
    pub fn snapshot_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot;

    /// Build a spectator snapshot from the union of the selected players' current fog, stale
    /// building memory, and resource rows.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot;

    /// Same projection as `snapshot_for_spectator`, with explicit room-projection diagnostic options.
    pub fn snapshot_for_spectator_with_options(&self, visible_players: &[u32], options: SnapshotOptions) -> Snapshot;

    /// Build a full-world snapshot for a room projection that intentionally exposes all state.
    /// Normal gameplay must not use this.
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot;

    /// Same full-world projection, with explicit room-projection diagnostic options.
    pub fn snapshot_full_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot;

    /// Player ids still alive. Humans need at least one building; AI players also need a unit.
    pub fn alive_players(&self) -> Vec<u32>;

    /// Team ids with at least one alive member, in stable start/lobby order.
    pub fn alive_team_ids(&self) -> Vec<TeamId>;

    /// First alive player on a team in stable start/lobby order.
    pub fn first_alive_player_on_team(&self, team_id: TeamId) -> Option<u32>;

    /// Whether this player's team still has at least one alive member.
    pub fn team_has_alive_player(&self, player_id: u32) -> bool;

    /// Frozen score-screen rows for every match participant, in start/lobby order.
    pub fn scores(&self) -> Vec<PlayerScore>;

    /// Authoritative observer analysis state for configured spectator-only or all-recipient audiences.
    pub fn observer_analysis(&self) -> ObserverAnalysisPayload;

    /// Remove all of a player's entities (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32);

    pub fn tick_count(&self) -> u32;

    /// Authoritative commands applied so far, stamped with the tick that applied them.
    pub fn command_log(&self) -> &[CommandLogEntry];

    /// Reconstruct the player specs used to create this match for replay/crash artifacts.
    pub fn player_inits(&self) -> Vec<PlayerInit>;
}

pub type TeamId = u32;
pub struct PlayerInit { pub id: u32, pub team_id: TeamId, pub faction_id: String, pub name: String, pub color: String, pub is_ai: bool }
pub struct CommandLogEntry { pub tick: u32, pub player_id: u32, pub command: Command }
pub struct SnapshotOptions {
    pub include_movement_paths: bool,
    pub movement_paths_for_all_projected: bool
}
```
`SimCommand` is the internal command enum from `game::command`; live `ClientMessage::Command`
envelopes and replay artifacts are translated into it at the boundary. Live transport metadata
such as `clientSeq` stays in the room/connection layer and is not part of the sim command or replay
command-log contract. `game::upgrade::UpgradeKind` is public because `SimCommand::Research` carries
it and external AI controllers construct ordinary `SimCommand`s. `CommandLogEntry.command` remains
the serde `Command` from `rts-protocol` so replay JSON stays wire-compatible. `StartPayload`,
`Snapshot`, `Event`, and `PlayerScore` are also serde types from `rts-protocol`.

`PlayerInit.is_ai` marks a computer-controlled player. AI players are full players in every
respect (they get a start position, City Centre, workers, economy, and count toward
win/elimination); the only difference is they have no socket. `Game` does not own AI controllers;
the room task or tool harness asks `rts-ai` controllers for ordinary `SimCommand`s and enqueues
them through this API before ticking — see §8.

Lab mutation types live under `game::lab`. `LabOp` is intentionally narrow rather than a debug
backdoor: entity mutations validate known unit/building kinds, real players, finite in-map
positions, placement/collision legality, and stale ids before changing the world. Accepted lab
mutations clear stale orders and reservations where needed, then rebuild supply, spatial index,
fog, and building memory before returning. `LabScenarioV1` is setup data keyed by map identity,
player resources, completed research, entity records including stable body/weapon/setup facing, and
small lab metadata such as scenario name and exported tick;
room-owned protocol export adds the requesting operator's current lab vision metadata before
sending JSON to the browser.
Restore loads the named map, validates faction/research/kind data, recreates entities with fresh
ids, repairs derived state, and returns the id remap for callers that need to reconcile UI
selection. Snapshot-only projections, transient events, projectile runtime state, active commands,
production queues, rally plans, cooldowns, and command logs are not part of the scenario format.

`PlayerInit.team_id` is canonical team identity. Phase 1 preserves FFA gameplay by assigning each
seated player a unique nonzero team by default; deserialized or hand-built fixtures with
`team_id == 0` are normalized to `team_id = id` when constructing a `Game`. Relationship helpers
on `Game` are available for future team-aware systems: `team_of_player`, `same_team_player`,
`same_team_owner`, `is_enemy_player`, `is_enemy_owner`, and `allied_player_ids`. Neutral owner `0`
is never allied with a player.

`PlayerInit.faction_id` is canonical faction identity. The default current faction is
`kriegsia`, and the server/lobby layer validates requested or recorded faction ids before match
assembly. That policy is separate from `rules::faction` catalog existence: normal lobby, AI,
self-play, and dev starts default missing requests to Kriegsia, explicit
`kriegsia` and `ekat` requests are accepted as playable factions, replay paths require explicit
recorded faction ids, and `phase2_empty_fixture` is accepted only by test-fixture contexts. The
lower rules/sim layer also fails closed: empty faction ids may default only at the narrow
compatibility boundary, while unknown non-empty ids get no faction catalog loadout, no starting
entities/resources, no supply credit for faction units/buildings, and no legal
build/train/research/gather/ability surface.

Command validation, queued attack promotion, combat target acquisition, direct damage attribution,
shot interception, overpenetration, support-weapon splash attribution, worker-retreat metadata, and
under-attack notice routing use `TeamRelations` snapshots derived from `PlayerState`. Hostile target
checks must call `is_enemy_owner` through that relationship surface rather than relying on raw
`owner != player`; this covers explicit attack commands, ordered attack retention, attack-move and
idle auto-acquisition, shoot-while-moving target retention, Anti-Tank Gun tank preference, hostile
building target acquisition, direct-fire damage attribution, and overpenetration victims. Raw
`owner == player` checks remain correct for strict authority and economy surfaces such as
selected-unit ownership, production/research/cancel authority, build/gather ownership, rally
control, supply, upgrades, and resource spending. Snapshot entity visibility and `visibleTiles`
use the union of current fog from living teammates on the recipient's team. A defeated or
disconnected teammate has no live entities and no longer contributes current sight; a defeated
player whose team still has a living member still receives that surviving team vision. Allied
visible entities project full read-only inspection details, but command authority, economy,
resources/supply/upgrades, rally/order plans, ability controls, and debug path overlays remain
exact-owner-only. Combat target ids and weapon facing for allied entities are projected only when
the target is team-visible, so allied inspection does not reveal hidden enemy ids or directions.
Victory resolution is team-aware:
the room task ends 2+ player matches only when at most one nonzero team still has an alive member,
and a defeated player does not receive an individual loss screen while any teammate keeps that team
alive.

### 3.2 Concurrency model
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`). Room registry handles carry per-room identity tokens; registry disposal removes only the matching identity and signals that room task to shut down.
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` to push to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `SetSpectator`, `SetFaction`,
  `Command`, `GiveUp`, `PauseGame`, `UnpauseGame`, `SetRoomTimeSpeed`, `StepRoomTime`,
  `SeekRoomTime`, `SeekRoomTimeTo`, `SetVisionSelection`, `Lab`). The room task is the single writer
  of game state — no locks around `Game`. Replay vision selection and successful replay seeks immediately send affected
  viewers fresh fog-scoped snapshots instead of waiting for the next replay tick.
- Room-mode, phase, and match-composition dependent lobby checks use a lobby-local `SessionPolicy`
  descriptor for the current room mode and phase matrix, including dev-watch, replay-room,
  branch-staging, speed-only live-game room-time controls, countdown, speed-source, and match-history
  decisions. Live-match handlers live in `room_task/live.rs`,
  replay-branch handlers live in `room_task/branch.rs`, lab request handling lives in
  `room_task/lab.rs`, dev-watch scenario handling lives in `room_task/dev.rs`, and room lifecycle
  bookkeeping lives in `room_task/lifecycle.rs`; `RoomTask` remains the owner of mutation and tick
  authority. Plain `/lab` is a client-side catalog selector. Direct lab URLs keep compatibility:
  `scenario=lategame` requests the bundled catalog scenario, `scenario=blank` keeps blank lab
  startup, and custom map or seed lab URLs stay blank unless they set an explicit scenario. Bundled
  lab scenario ids are safe tokens listed in `server/assets/lab-scenarios/manifest.json`; the
  loader in `server/src/lab_scenarios.rs` validates manifest metadata, safe filenames, duplicate
  ids, JSON parseability, map/player-count consistency, and restore compatibility through the
  public lab `Game` API before a scenario is exposed or launched. Bundled lab scenario startup uses
  the same restore-scenario path as manual imports and starts with `operation_count=0` plus a tick-0
  timeline keyframe. Lab god mode is lab-only state: `setPlayerGodMode` marks that player's units
  and buildings
  invulnerable, applies across lab mutations, owner changes, spawned assets, and timeline replay
  state, and is mirrored in start/labState metadata.
- The public lobby browser asks room tasks for bounded summaries over `RoomEvent::Summary` instead
  of reading room internals. Normal lobby/countdown/live-match rooms and persisted match-history
  replay staging lobbies are summarized; dev, replay-artifact, replay playback, replay-branch, and
  lab modes stay hidden. `GET /api/lobbies` collects those summaries with a short timeout and
  returns browser-safe DTOs sorted by joinability then age; the client polls that route every 1.5
  seconds and preflights a clicked row against the latest route response before sending `join`. No
  WebSocket push message currently exists for the browser list; the HTTP poll cadence is the
  accepted freshness target.
- The room task, each tick: enqueue live AI commands for AI players → `game.tick()` → build
  per-audience snapshots through the lobby-owned `ProjectionPolicy` → send through
  `SnapshotFanout`. `ProjectionPolicy` names live player fog, spectator union vision, selected
  perspective projection, full-world projection, projected movement-path diagnostics, and
  observer-analysis recipient scopes; `SnapshotFanout`
  owns compacting, net status, and perf accounting. Lobby phase: broadcast `lobby` on changes.
- Live-match pause state belongs to `RoomTask`, not `Game` and not `tick_control.rs`. Normal live
  and branch-live active seats plus live spectator connections can spend up to three successful
  pause starts per match; replay viewers, dev-watch viewers, lab viewers, and AI-only room-time
  viewers cannot spend live-match pauses. While paused, the room
  event loop continues handling reliable control messages, Give up, disconnects, and unpause, but
  the live scheduled tick returns before constructing `LiveTickDriver`, so AI thinking,
  command-ack consumption, `Game::tick`, snapshot fanout, and defeat checks do not advance.
  `prepare_live_match_launch`, live-match teardown/replay transition, and empty-room reset all
  clear pause counters and paused state.
- Normal live rooms reject active mid-match joins but accept `join { spectator: true }` as a
  gameplay-read-only live spectator attach with shared pause controls. Spectators receive
  `StartPayload.spectator = true` and live
  `game.snapshot_for_spectator(active_player_ids)` snapshots plus event unions for those active
  player buckets, with per-player position-free non-alert notices filtered out, but are not
  included in `PlayerInit`, command routing, elimination, or match-player counts.
  `RoomTask` captures the existing connected recipient ids before inserting a late spectator and
  queues a room-owned
  position-free info notice for those recipients only: `<name> has joined the match as a
  spectator`, with `Commander` used for empty/control-only names. The queue is keyed by connection
  id and appended after normal live projection in `LiveTickDriver`, then cleared for each recipient
  only after snapshot fanout accepts that recipient's next live snapshot. While live pause is active
  no snapshot fanout occurs, so queued late-spectator notices wait for the next emitted live
  snapshot after unpause. Replay-branch live rooms use the same observer attach shape for late joins
  to the private branch room; they do not return to branch staging or create another original-seat
  mapping.
- Lab rooms are hidden `RoomMode::Lab` rooms that start a drain-tracked real `Game` on first join
  with a room-owned collaborator session record. Existing lab rooms remain joinable during deploy
  drain, but a lab that has not launched yet rejects the first join instead of creating a new
  authoritative session. Direct lab joiners currently receive the operator role; the original
  joiner remains in `operatorId` metadata for compatibility, not as the only mutation authority.
  They use the shared launch helper with `StartPayload.lab` metadata and prediction disabled. Lab
  setup mutations call `Game::apply_lab_op`; issue-as commands call `Game::issue_lab_command_as`,
  which rejects mixed-owner selections before queuing a normal command. Lab state, dirty flags,
  viewer roles, per-operator selected vision, the future-join vision default, shared room-time
  speed/pause/controller state, and append-only operation log records stay in the room task rather
  than in `Game`. Scenario PR submission also starts in `room_task/lab.rs`: the room exports the
  authoritative lab `Game`, validates authoring metadata, rate-limits the room to one PR job, and
  then hands the already-validated preview to a bounded background service so GitHub or git work
  never runs on the room tick path. The submission service rechecks catalog duplicates, safe
  filenames, path allowlists, payload/entity caps, branch safety, and the exact scenario plus
  manifest file set before opening a draft PR with a reviewer checklist. Paused lab room-time
  suppresses scheduled ticks; one-tick lab steps and running lab ticks use the same
  `LiveTickDriver` path as ordinary live simulation.
- Dev scenario watch rooms are a special-case room mode inside the same task model: they own a
  normal `Game`, drive authored scenario setup and optional scripted movement, and use the shared
  projection and fanout helpers to send watchers full-world snapshots for the configured view
  player. Saved self-play artifacts are normal `ReplayArtifactV1` files and load through
  `Phase::ReplayViewer` via the neutral replay-artifact room path.
- Replay viewer rooms use `Phase::ReplayViewer`, which owns a lobby-local
  `replay_session::ReplaySession`: the immutable `ReplayArtifactV1`, rebuilt `Game`, command
  cursor, shared playback speed, and per-viewer fog selection. Replay snapshots use `game.snapshot_for_spectator(selected_player_ids)`
  so viewers see authoritative union-fog or single-player fog, selected-player resource rows, and
  selected-player remembered building memory, never full-world state.

Lobby-owned runtime boundaries stay in `server/src/lobby/`; none of these helpers move transport,
AI controllers, or Tokio coordination into `rts-sim`. `RoomTask` remains the single Tokio owner of
room membership, phase state, room-owned control state, and the active `Game`; the implementation is
split into focused room-local modules:

- `room_task.rs` is the actor shell. It defines the `RoomTask` state, constructs rooms, owns the
  run loop and event receiver, maps `RoomEvent` variants to mode handlers, exposes phase/policy
  helpers, and keeps tiny shared send utilities. It should stay small enough to load first when
  orienting around room behavior.
- `room_task/types.rs` contains room-owned data types, constants, and constructors shared by the
  room-task modules, including `RoomPlayer`, `AiSlot`, `Phase`, `RoomMode`, lab/dev room config,
  and replay/lab tick payload stamps.
- `room_task/lobby.rs` owns ordinary public lobby behavior: summaries, joins/leaves, readiness,
  host fallback, team and faction selection, AI seats, spectator flags, selected map, quickstart,
  countdown entry, and lobby broadcasts.
- `room_task/live.rs` owns live-match room controls: command routing, command receipts, active
  player pause and unpause, speed-only live-game room-time state, give-up, late spectator
  attach, live start-payload glue, pending recipient notices, and live snapshot notice plumbing.
- `room_task/lab.rs` owns lab sessions: first-join launch, lab role/vision metadata, request
  authorization, mutation and issue-as routing, result delivery, dirty state, operation logging,
  state broadcasts, room-time controls, scenario export/import, and authoritative scenario PR
  submission dispatch.
- `room_task/dev.rs` owns dev-watch and authored scenario rooms: dev joins, scenario launch, script
  driver glue, room-time controls, and dev start-payload sends.
- `room_task/replay.rs` owns replay viewer rooms: replay joins and prompts, replay start-payload
  sends, room-time seek/speed/step, per-viewer vision, observer analysis, replay ticks, and
  return-to-lobby replay behavior.
- `room_task/branch.rs` owns replay-branch room handling: staging joins, original-seat
  claim/release, staging broadcasts, branch launch preparation, and branch-live attach.
- `room_task/lifecycle.rs` owns start/end/reset/drain bookkeeping around the room actor:
  countdown completion, match launch setup, game-over handling, match-history persistence gates,
  post-match replay transition, empty-room disposal, drain warnings, and slow-tick logging.
- `room_task/helpers.rs` contains small shared room-task helpers such as countdown duration,
  server-build metadata, and automated match-history room classification.

Empty private dev, replay, replay-artifact, and lab rooms are disposable; empty private
replay-branch rooms reset their live/staging state but keep `RoomMode::ReplayBranch`, so reserved
`__replay_branch__` names never decay into public normal lobbies while preserving the in-memory
branch seed.
- `session_policy.rs` is the explicit internal descriptor for the current room mode and phase. It
  names the state source, join, clock, authority, mutation, visibility, diagnostics,
  persistence/export, drain launch/accounting, start-payload, and UI-affordance choices used by the
  rest of the lobby helpers. Persistence is split into match-history eligibility, transient
  post-match replay capture, match-history replay-artifact attachment, and room-local lab operation
  logging. Room-controlled clock policy keeps tick routing sources such as replay playback, dev
  scenario, lab, and live game separate from neutral operation profiles such as speed-only,
  speed-and-step, speed-and-seek, and full seekable. Product identity still selects real setup paths
  such as replay-artifact loading, dev scenario construction, replay-branch seeding, and lab room
  initialization; lower-level helpers should consume the explicit policy fields when the behavior is
  shared.
- `participants.rs` is the connected-user and active-seat helper. It owns host fallback, active
  human and AI seat lists, spectator visible-seat lists, branch-live connection-to-original-seat
  aliases, and command issuer resolution.
- `tick_control.rs` maps the session clock policy, replay pause/speed, room-controlled live-game
  pause/speed, dev-watch and lab pause state, and countdown state to the room ticker interval and
  scheduled action. `RoomTask` still owns the Tokio interval and remains the only task that advances
  a room.
- `lab_timeline.rs` owns room-local in-memory lab rewind recording outside the simulation crate. It
  records a baseline keyframe after lab `Game` creation or scenario import, records accepted lab
  world mutations and issue-as commands in authoritative room order, stores periodic cloned `Game`
  keyframes, rebuilds lab seeks from the nearest retained keyframe, and truncates future history
  after a past seek plus a new accepted lab operation or issue-as command.
- `projection.rs` owns snapshot projection and observer-analysis decisions for client fanout. Live
  active players get player fog, live spectators get active-seat union fog, replay viewers get their
  selected perspective from vision selection, lab viewers get their room-owned per-operator lab vision,
  branch-live active players use original-seat aliases, and dev-watch viewers get full-world
  scenario snapshots. New event types and snapshot fields should be audited with
  [docs/projection-audit-checklist.md](../projection-audit-checklist.md) so the owner, selected
  player ids, full-world policy, and private-notice behavior are named before fanout code changes.
- `launch.rs` owns the lobby start-payload builder and send loop for live, replay-branch-live,
  lab, dev-watch, and replay viewer starts. The builder consumes `SessionPolicy`, recipient role,
  projection-derived diagnostics, prediction eligibility, pending snapshot behavior, and
  source-specific metadata to stamp player id, spectator flag, prediction build/version,
  recipient capabilities, diagnostics, replay metadata, and lab metadata. `Game::start_payload()`
  remains the source of static simulation start data, while `replay_session.rs` keeps replay
  playback state and exposes replay start metadata for the builder.
- `live_tick.rs` runs one live simulation tick around the existing `Game` seam: AI command enqueue,
  `Game::tick`, recipient-specific room notice injection after projection, snapshot fanout,
  observer analysis, defeat/game-over checks, and panic replay capture.
- `replay_session.rs` owns replay playback state, seek/keyframe policy, per-viewer vision selection,
  and post-match/dedicated replay start payloads.
- `replay_branch.rs` owns branch staging state, original replay-seat claim/release policy, and
  branch live-launch preparation while `room_task.rs` still owns connected members and final phase
  changes.
- `snapshot_fanout.rs` and `snapshots.rs` centralize compacting, replace-latest snapshot delivery,
  net-status metadata, and union-event helpers for live, spectator, replay, branch, and dev views.
- `connection.rs`, `dev_replay.rs`, `crash_replay.rs`, `faction_validation.rs`, and
  `replay_validation.rs` are lobby-local support modules for connection sinks, dev artifact
  loading, panic artifacts, and server-side lifecycle validation.

`/dev/scenario` remains mode-local for scripted setup and driver selection: each scenario still
chooses a dedicated `Game::new_*_scenario` constructor and optional tick driver before joining the
shared clock, projection, and launch helpers. Moving those constructors into a generic launch path
would either widen this behavior-preserving refactor or add scenario registration machinery, so
future lab work should consume the extracted primitives first and migrate scenario setup only with a
separate product-approved design. `scripts/check-lobby-architecture.mjs` guards the now-stable
fanout boundary by failing new production lobby calls to `Game::snapshot_for*` outside
`projection.rs`, except for the existing AI think context in `live_tick.rs`. The same guardrail
keeps accepted lab mutation and issue-as calls centralized in `room_task/lab.rs`, where role-based
operator authorization, result routing, dirty state, and the append-only operation log live. The
checker also ratchets the post-split room-task shape: `room_task.rs`, each production child module,
and the production room-task total all have explicit line budgets, and any new child module must add
its own budget instead of silently becoming another hotspot.

### 3.3 Rules layer (`rules/`)

`server/crates/rules/src/` contains classification, formula, terrain, and economy functions with
no simulation state dependency. `server/crates/sim/src/rules/projection.rs` is the explicit
state-reading exception: it reads `Entity`, `Fog`, and smoke state so snapshot and event visibility
policy is centralized instead of scattered through services.

- `rules::defs` — immutable unit/building/node definition tables keyed by `EntityKind`. These
  records are the source of truth for kind-specific stats, armor class, weapon class, production
  chains, tech requirements, and resource-node amounts.
- `rules::faction` — faction catalogs keyed by stable faction id. Catalogs reference global
  `EntityKind`, upgrade id, ability id, and Steel/Oil/Supply costs; reuse a global id across
  factions only when gameplay semantics are identical for every faction that can use it. Divergent
  behavior, stats, production role, or ability meaning requires a distinct global id gated through
  catalog availability. The default catalog is `kriegsia`; `ekat` exposes the current Ekat hero,
  Zamok, and Golem slice; `phase2_empty_fixture` exists only as a command-validation test fixture.
  Server-side lifecycle policy lives in `server/src/lobby/faction_validation.rs`.
- `rules::combat` — default weapon-profile ids and policy metadata, AP/armor predicates (`is_ap`,
  `is_armored`), target-ranking classifiers (`target_threat_role`, `default_weapon_target_fit`),
  compatibility helpers such as `attack_profile(kind) -> AttackProfile`, and weapon-aware direct
  damage/miss/facing helpers such as `effective_damage_for_weapon(profile, victim_kind, base_dmg,
  victim_terrain) -> u32`.
- `rules::economy` — tech/production predicates (`trainable_units_for_faction`,
  `build_requirement_met_for_faction`, `train_requirement_met_for_faction`,
  `can_research_for_faction`), resource-node amounts, and cost/supply wrappers (`cost`,
  `supply_cost`, `supply_provided`). Legacy non-faction helpers remain as default-faction
  compatibility surfaces for older call sites and tests.
- `rules::terrain` — `TerrainKind` plus movement, cover, concealment, and static line-of-sight
  opacity modifiers. It is intentionally small today (`Open` returns current defaults; raw stone
  terrain blocks LOS) so the forest/road/hill feature has one rules file to grow in.
- `rules::projection` — fog-gated `EntityView` construction, legacy/special `visionOnly`
  projection support, and event visibility predicates.

### 3.4 Ability system (`game/ability.rs`, `game/services/ability_orders.rs`)

`rules::faction` owns the faction-aware ability registry. Each `AbilityCatalogEntry` records the
stable id, label/icon/hotkey/title, legal carriers, target mode, optional min/max range, cooldown,
finite charges, Steel/Oil cost, tech requirement, queue policy, autocast support, command-card
visibility, and compact protocol/order-stage codes. `game/ability.rs` keeps the typed
`AbilityKind` and converts those registry rows into the sim-facing `AbilityDefinition`; it is not a
second source of metadata. Adding a registry-backed ability means adding a global `AbilityKind` and
protocol id, adding the faction catalog entry, updating the client mirror/parity check, and then
adding only the effect-specific code that the registry cannot express.

`AbilityDefinition` also carries a sim-local `AbilityEffectHook` discriminator for the reusable
effect shapes that actually exist today: legacy no-op (`charge` compatibility), owned area
status (`breakthrough`), delayed world effects (`smoke`, `mortarFire`), dash return, line
projectile, Magic Anchor placement, Golem consumption, and the intentionally one-off artillery
point-fire path. The hook receives the owning player's faction id at execution time through the
normal command/order helpers, so wrong-faction ability use fails before effects, resource spending,
cooldowns, or events are applied. Artillery point fire locks each raw click to the issuing gun's
valid 25-to-55 tile range band, stores that effective point, and owns any needed in-place setup or
redeploy before the first shot. It records temporary live-fog firing reveal sources for enemy
players when a shell launches, using the firing-cycle-plus-half-second lifetime and smoke
suppression used by other actionable firing reveals. The hook is deliberately not a generic script engine. Phase 11 signature abilities
should first use one of the existing shapes; if they cannot, add either a narrow explicit hook or a
named one-off path with faction validation, cost validation, and fog-safe event tests rather than
widening the hook into generic scripting.

`services::ability_orders` owns the tick-path execution helpers:
- `order_or_launch_world_ability` — for `WorldPoint` abilities: if the caster is in range, launch
  immediately; otherwise compute a staging point inside range and issue an `Order::Ability`
  movement order via `MoveCoordinator`.
- `launch_world_ability` — reads range/cost/cooldown from the registry, deducts resources, sets
  the caster's cooldown, clears the active order,
  and dispatches a delayed-world effect hook (currently: schedules a smoke cloud or delayed mortar
  shell). Manual mortar fire also enters the mortar weapon firing cycle: launching a manual shell
  starts the weapon cooldown, and both immediate in-range manual fire orders and queued MortarFire
  promotions wait while the mortar ability cooldown or weapon cycle is reloading instead of
  launching early or being cleared. The active manual fire order remains eligible for aiming during
  reload, so the mortar can rotate toward the target before the weapon cycle is ready. A scheduled mortar shell resolves from its
  scheduled impact point even if the firing mortar dies before impact; reveal data at impact is
  emitted only when the original mortar entity is still alive and valid. Guards:
  caster exists + alive + owner + not under construction + correct kind + not on cooldown +
  required tech present + in range + affordable.
  All guards are checked without panicking; missing/stale casters are no-ops.
- `launch_self_ability` — validates the self-targeted registry row and dispatches owned-area-status
  or Golem-consumption hooks. Breakthrough remains an owned-unit area buff; Ekat Consume removes the
  nearest owned living Golem in range and restores Ekat to full HP; legacy Charge remains decodable
  but has no current carriers, cooldown, or runtime status.
- `caster_can_attempt`, `tech_requirement_met`, `caster_in_range` — pure predicates used by both
  command validation and order-queue promotion.

Active `Order::Ability` movement orders run through `services::order_queue::promote_ready_orders`:
when the caster arrives (phase `Arrived`), `launch_world_ability` is called; when pathing fails
(phase `PathFailed`), the order is cleared silently. Stale queued ability intents (caster dead,
tech gone, target point off-map, or cooldown active for a skip-if-not-ready ability) are skipped at
promotion time via `ability_intent_valid`. Wait-until-ready world abilities instead promote into an
active ability order and hold there while cooldown or weapon readiness catches up.

Services in `server/crates/sim/src/game/services/` orchestrate tick logic and call into `rules::*` for classification.
Rules functions have no imports from `services/`; classification and formula rules read
kind-specific data from `rules::defs`. `config.rs` holds scalar constants and compatibility
wrappers such as `unit_stats(kind)` / `building_stats(kind)`, which return the stats embedded in
defs.

Snapshot ability affordances are projected from the owning player's faction catalog, so fixture or
future factions do not inherit Kriegsia command-card buttons merely because they reuse a global
entity kind.

Complex ability runtime state lives in `game::ability_runtime`. Its `AbilityRuntime` owns
deterministic active instances and lightweight ability world objects that are not normal entities:
they do not participate in supply, pathing, production, selection, scoring, or combat target
queries unless a later phase explicitly adds such behavior. `Game::snapshot_for`,
`snapshot_for_spectator`, and `snapshot_full_for` project active world objects through
`Snapshot.abilityObjects`, filtered by the same current-team fog / spectator union / full-world
mode used by other snapshot data. Enemy-visible objects expose only public render fields; owner-only
payload state and safe caster ids are withheld from enemies.

Neutral trench terrain lives in `game::trench::TrenchStore`, owned by `Game` rather than the entity
store. Trenches have deterministic ids, stable world-pixel centers, and a radius from the
Entrenchment rules, but they do not consume supply, block construction/pathing, take damage, count
for scoring, or participate in entity death cleanup. Snapshot projection sends currently visible
trench terrain through the same active-player, selected-player, and full-world policies as other
world objects, and records per-player discovered terrain so a scouted trench remains visible after
it becomes fogged. That remembered trench record is terrain-only; it does not expose creator,
owner, current occupants, or hidden unit state.

`services::entrenchment` updates unit-facing trench state after normal collision cleanup and before
final snapshot indexing. Riflemen, Machine Gunners, and Panzerfausts owned by a player with
completed Entrenchment research create a neutral trench after holding ground on untrenched terrain
for 90 consecutive simulation ticks. Engineers/Workers are not eligible: they neither dig new
trenches nor occupy existing trenches. Holding ground means the unit has no movement path, no path
movement delta for the tick, no collision displacement after pre-collision derived-state rebuild,
and an order that is effectively stationary: Idle, Hold Position, an in-range Attack order, or an
arrived Attack Move. Firing, target changes, body/weapon facing, and Machine Gunner setup/teardown
do not reset that timer; Move, Attack Move while still travelling, Gather, Build, Deconstruct,
ability movement, artillery point-fire, path movement, and non-slotting forced movement reset it.

Existing trenches are neutral. Any eligible Rifleman, Machine Gunner, or Panzerfaust can occupy an empty
one without owning Entrenchment research when it is stopped in the trench footprint. Each trench can
actively hold only one infantry unit; once occupied, it is skipped as an occupation candidate for
other units. A stopped eligible unit within one tile of an empty trench may be slotted by at most
one tile into a legal position inside the trench footprint; slotting validates static
standability, the swept static segment, and unit-body overlap against the current live entity
positions. Slotting does not issue a move order or path, so the unit can still fire normally.
Move and attack-move formation assignment also prefers a nearby known, unoccupied trench for
eligible infantry when the trench footprint is within two tiles of the unit's normal formation
goal. This uses only current team-visible trench terrain plus the issuing player's remembered
trench terrain; hidden server-only trenches never influence movement goals. Non-eligible units,
occupied trenches, blocked trench points, and farther trenches fall back to ordinary formation
spreading.
`entity::active_trench_occupation(entity)` is the simulation predicate for active occupation;
digging progress, failed slotting, and merely standing near trench terrain do not set it. Visible
occupied units project `occupiedTrenchId`; remembered trench terrain never exposes hidden
occupants.

Entrenched combat benefits consume only active occupation through
`entrenchment_combat::is_actively_entrenched`. Active entrenched Riflemen, Machine Gunners, and
Panzerfausts gain one tile of weapon range through `entrenchment_combat::attack_range_tiles`, and idle
target acquisition treats them like Hold Position: they can acquire and fire at legal targets inside
current weapon range but do not request idle chase paths. Explicit Attack and other player orders
remain authoritative and may move or chase the unit out of the trench; command application and lab
moves clear active occupation before later combat decisions use it.

Incoming direct-fire miss policy uses the highest applicable independent chance. Existing
Anti-Tank Gun infantry miss chance is 65%; entrenched eligible infantry add a 70% miss chance, so
an Anti-Tank Gun firing at an entrenched Rifleman, Machine Gunner, or Panzerfaust rolls 70%, not a
composed probability. Area effects call `entrenchment_combat::reduce_area_damage` after their
normal falloff and armor calculations, so Mortar and Artillery splash deal 30% of their current
post-formula damage to actively entrenched eligible infantry. Direct-fire over-penetration stops at
an entrenched primary victim, and actively entrenched secondary candidates are skipped rather than
taking over-penetration damage or emitting secondary hit feedback.

The `entrenchment_inspection` dev-watch scenario seeds a two-player inspection map with researched
Entrenchment for player 1, a dig-capable Rifleman, friendly and enemy eligible reuse units, a
Machine Gunner for crowded slotting checks, and several neutral trenches including a nearby
connected pair. Its script driver does not issue movement commands after setup, so humans can pause,
step, and manually inspect trench rendering, reuse, fog-memory behavior, and occupied-unit
projection from the initial state.

Per-caster recast state is exposed to the owner through `EntityView.abilities`: active return marker
id, availability tick, and remaining lifetime are projected only for the owning player's command
card. Ekat's `ekatTeleport` world-point activation is a dash: it validates static standability,
moves Ekat, and creates a four-second return marker at the original position. `recastAbility`
commands are explicit and validate a live owned caster plus matching active runtime state; missing
state, same-tick/too-early return, stale caster ids, and invalid return destinations are ignored
rather than overloading world-point `useAbility` commands. A valid recast returns Ekat to the
marker and consumes it.

Ekat's `ekatLineShot` world-point activation clamps the endpoint to ability range, spawns an
ability-runtime line projectile at Ekat's current position, and starts cooldown when the projectile
is accepted. The projectile travels outbound to the clamped endpoint, then returns toward Ekat's
current server position each tick, so moving or dashing after firing can bend the return path.
Enemies intersecting the swept line are damaged once per leg; stale or dead casters remove the
projectile without resolving further hits.

Ekat's `ekatMagicAnchor` world-point activation places one replacement-style, non-blocking,
non-attackable runtime object at the target point. It naturally expires after 10 seconds. While
active, it creates a 3-tile pull field: units moving away from the anchor are slowed, units moving
toward it are boosted, and stationary units are pulled toward it. Pull strength increases closer to
the anchor, and stationary pull is reduced by the same footing resistance used by collision so
braced and heavy units move less than soft infantry. If an active anchor exists when `ekatLineShot`
is accepted, the runtime spawns one projectile from Ekat and one from the anchor toward the same
cursor point; both return toward Ekat's current server position.

Ekat's `ekatConsumeGolem` self activation has no cooldown or resource cost. It finds the nearest
owned, living Golem within the ability range, releases any active mining slot held by that Golem,
removes the Golem permanently, restores Ekat to max HP, and emits an owner-visible positioned
notice. If no Golem is in range, the command is a no-op. Ekat has no passive regeneration.

Mortar shells are delayed AOE effects resolved by `game::mortar` after their flight timer expires.
Every manual and autocast shell scatters from its intended impact point when scheduled: targets
visible to the firing team use a one-tile median miss radius, while blind target points use a
four-tile median miss radius.
They damage owned, allied, and enemy units/buildings with the same falloff and armor rules; resource
nodes are ignored. Same-team mortar damage is intentionally real friendly fire, but it is
unattributed: it does not update `last_damage_owner`/position/tick, does not trigger AI worker
retreat, does not emit enemy under-attack notices, and does not award kill credit or combat score.
Idle/attack-move autocast is conservative and requires completed `mortar_autocast` research: before
scheduling a shell, combat checks the scattered predicted impact point against owned and allied
units/buildings at their current positions and holds fire if any would be inside the damaging radius. Autocast
target acquisition uses the same safety check, so Mortar Teams face the nearest target that can be
autocast safely instead of tracking an unsafe closer enemy. Manual mortar fire is intentionally
allowed onto same-team positions, so players can still take risky shots deliberately. Mortar
autocast is stored on the authoritative combat state, is enabled for current and future Mortar Teams
when research completes, and can be toggled through `SetAutocast(mortarFire, enabled=<bool>)`;
disabled mortars still accept manual `mortarFire` commands.

Artillery point-fire shells follow the same support-weapon friendly-fire contract: blast damage can
hit owned and allied entities in the radius, but same-team damage is unattributed and cannot award
enemy kill credit. Direct-fire weapons are the opposite rule: normal target selection, ordered
attacks, shot interception, and overpenetration only damage enemies. Allied entities may block a
direct line of fire through the friendly-blocker safety rule, but they are not legal direct-fire or
overpenetration victims.

`server/crates/archcheck` classifies each top-level service module before accepting
service-to-service imports. The roles are intentionally coarse and are part of the command/order
boundary:

- tick systems are the phases called by `systems.rs`;
- command adapters translate queued or immediate command facts into plans and call narrow
  executors;
- pure policy modules accept facts and return decisions without mutable world state;
- query/index services read derived or immutable world state;
- mutation helpers perform narrow execution work below a caller-owned phase.

Every service import must be present in the exact import allowlist and must also be legal under the
role matrix. A role failure explains why the edge is forbidden, usually because orchestration should
stay in `systems.rs` or because command-family growth should use command input -> issue-time facts
-> pure plan -> narrow executor. Residual `services::commands` and `services::order_queue` imports
into tick-system helpers are named one by one in the role allowlist; there is no blanket broad
adapter exception. New command/order service edges therefore need both an exact import allowlist
entry and a role-matrix justification.

`game::systems::run_tick` owns the tick pipeline and the lifecycle of tick-scoped derived state.
It rebuilds named phase state at explicit boundaries: pre-command state for command validation,
pathing, and movement; post-movement state for combat and economy queries; pre-collision state
after production/construction/death mutations; collision-displacement snapshots for entrenchment;
and final state for snapshot interest filtering.
Systems should consume the derived-state object for their phase instead of carrying occupancy or
spatial indexes across later mutations.

### 3.5 Command planning and queued order semantics

The authoritative command model is: clients compose intent; the server validates and plans it.
Keyboard latching, double-tap quick-cast, Shift lifetime, and cursor previews are client UX. The
simulation contract begins when a `SimCommand` reaches `services::commands`: the command service
dedupes and caps unit-id lists, rejects over-budget human unit-list commands, builds issue-time
facts for the referenced units/targets, and must produce unit-local actions that match the policy
below. The budget scalars live in the sim-owned `command_budget` helper so parity checks can dump
them without moving ownership into rules. Human command budget is supply-based: 24 base command
supply plus `COMMAND_CAR_SUPPLY_CAP_BONUS = 20` per submitted owned Command Car plus that Command
Car's own mirrored supply weight, so Command Cars offset their own weight before adding bonus
capacity.
AI-owned players are exempt from this budget because live AI
still issues ordinary `SimCommand`s through
`Game::enqueue`. Lab `issueCommandAs` can also opt into a lab-only admission mode that bypasses
the command-supply budget and uses a larger bounded unit-id window for scenario-scale commands.
`services::order_planner` is the pure
reference implementation of this planning policy. The planner has no `EntityStore`, fog, pathing,
economy, or cooldown mutation dependency; it accepts plain facts and emits one of three effects:

- `ReplaceActive` — replace this unit's active order and clear future queued intents.
- `AppendQueued` — append one future intent to this unit's queue.
- `ExecuteAbilityNow { preserve_orders: true }` — execute an immediate ability without replacing
  the active order or queued intents.

`services::order_execution` is the shared narrow mutation helper for order-state transitions that
are needed by both issue-time command application and queued promotion, such as support-weapon
setup, artillery point-fire targeting, and artillery teardown before movement. It should not grow
new validation policy or tick orchestration; those responsibilities remain with command admission,
queued promotion, or the owning tick system.

Tank weapon range is dynamic in the simulation: tanks keep their base 5-tile range while moving,
then linearly ramp to 14 tiles after three stationary seconds. Path-driven translation or hull
rotation resets the ramp to base range; turret aiming and external pushes do not.

Combat weapon cooldowns and firing-reveal response delays are keyed by
`rules::combat::WeaponKind` inside `CombatState`. The legacy `Entity::attack_cd()`,
`set_attack_cd()`, and `tick_attack_cd()` shims operate only on an entity kind's default weapon;
new multi-weapon code should use `weapon_cooldown`, `set_weapon_cooldown`,
`tick_weapon_cooldowns`, and `start_weapon_firing_reveal_response_delay`. Ability cooldowns,
lockouts, and uses remain separate from weapon cooldown state.

Auto-acquisition prefers unit targets before building cleanup targets by default. Building fallback targets still use weapon-fit ranking among eligible cleanup targets.

Overpenetration checks use the target's pre-damage entrenchment state, so lethal primary hits keep the same entrenched blocking decision used before damage resolution.

Entrenchment auto-occupation chooses the nearest trench that has a legal occupation slot for the
unit. A closer trench with no legal slot does not block searching for a farther usable trench. For
dig-in progress, explicit attack orders count as holding ground only after combat advances them to
the `Firing` phase; chasing or unreachable attack orders do not create trench progress. Lab move
operations clear trench occupation and dig-in state when repositioning an entity so snapshots do not
retain stale `occupiedTrenchId` values before the next tick.

Construction build-site checks classify the current site state before deciding whether work can
start or continue. The status distinguishes invalid terrain, existing buildings or scaffolds,
resource nodes, and relevant unit bodies, while preserving Tank Trap placement rules. Pump Jack
placement is the resource-node exception: it is only valid when the Pump Jack footprint center is
associated with non-depleted oil without another extractor on that same patch, and completed Pump
Jacks, not workers, extract oil. Pump Jacks are treated like field infrastructure rather than
survival buildings for defeat checks, so a player with only Pump Jacks remaining is eliminated.
Build orders can enter a `WaitingAtSite` phase and track unit-blocked ticks with a grace period
derived from `TICK_HZ`. Otherwise-valid build orders may be issued and promoted while the player
is short on resources; once the worker arrives, construction waits and retries until resources are
available.
Workers also wait through temporary unit blockers for that grace period, but cancel if permanent
blockers claim the footprint. Existing owned scaffolds can be resumed without charging again.

General rules:

- Commands must be valid at issue time. The server checks ownership, unit capability, target
  validity/visibility, finite points, ability carrier kind, ability readiness/cooldown/uses, and
  other command-specific facts before planning. It does not project future movement, future
  cooldown expiry, future tech, or future affordability.
- Same-tile movement goals count as arrived rather than path-failed. Plain move orders clear back to
  idle on arrival; attack-move orders keep their aggressive stance.
- `HoldPosition` clears each selected unit's active order and queued intents, then marks the unit as
  held. Held units do not voluntarily move or chase targets, but they keep normal collision behavior
  and may fire at enemies already inside current weapon range.
- Direct attack orders against visible enemies keep the explicit target when a friendly or enemy
  hard blocker would absorb the current shot; mobile attackers then use the existing chase path to
  seek a fireable position. Shared line-of-sight raycasts stop as reached when a grid-corner side
  step enters the target tile, while preserving opaque-target handling and the two-stone corner
  blocker behavior. Tank Traps are not combat shot blockers, so damage continues to the
  target behind them; tanks and normal buildings still block shots. Building targets and statically
  blocked target tiles use a passable perimeter chase goal instead of the blocked footprint center.
  Tank Traps keep generic building targeting and cleanup behavior but do not count for elimination
  survival. Infantry Move steering treats Tank Traps as passable but applies a small local avoidance
  bias when open space exists; vehicles remain hard-blocked by Tank Traps. Attack-move target
  acquisition remains stricter while the movement path is active: it chooses currently fireable
  targets first, and only uses out-of-range acquisition/chase targets when no current target is
  fireable. Setup weapons that stopped to engage during an unfinished attack-move keep their
  emplacement for a one-second no-target grace period; if the attack-move order still exists after
  that grace, they tear down and continue toward the original attack-move destination.
- Active moving-fire `Move` and `AttackMove` orders preserve the player-issued destination while
  they are still in `MovePhase::AwaitingPath`, `Moving`, or `PathFailed`. Their auto-acquisition is
  opportunistic: it may retain, aim at, expose, and fire on targets that are currently inside
  weapon range and pass hostile, visibility, smoke, terrain line-of-sight, and blocker checks, but
  it must not request chase paths, vehicle standoff paths, or enemy-directed replacements for
  `path_goal`.
  Once an `AttackMove` reaches `MovePhase::Arrived`, its existing aggressive post-arrival behavior
  applies. Direct `Attack` orders and idle-aggressive behavior remain separate and may still pursue.
- Normal combat auto-acquisition first filters already-legal hostile candidates in
  `services::combat::acquisition`, then chooses between them through the sim-local
  `services::combat::priority` ranker. The ranker owns priority terms such as default-weapon fit,
  Tank immediate-threat order, shoot-while-moving target retention, unit-over-building preference,
  and nearest/id tie-breaks; it does not decide fog, smoke, line-of-sight, blocker, ownership, or
  acquisition-radius legality. Unit attackers rank legal unit targets above buildings, so buildings
  remain last-resort cleanup targets unless explicitly ordered or covered by a special obstruction
  policy. Default small-arms weapons prefer soft targets while keeping armored or hard targets as
  fallbacks. Default anti-armor weapons prefer anti-armor threats and armored/hard units, with Tanks
  treating in-range Anti-Tank Guns as the top immediate threat.
  Vehicle-body units rank enemy Tank Traps as high-priority breach targets only when
  `services::occupancy` reports that the trap is on the current bounded route segment or forms a
  closed-gap pinch across that route; irrelevant nearby traps remain legal fallback targets but lose
  to real combat targets. The obstruction query is read-only, uses the current waypoint, `path_goal`,
  or movement intent, and does not run pathfinding during target ranking.
  Retention is intentionally a stickiness term inside the ranker rather than a separate branch:
  Tanks, Scout Cars, and Methamphetamines-upgraded Riflemen keep a still-legal current target when
  competing targets have the same material rank, but they switch when a higher-rank default-weapon
  threat appears.
  Dead, friendly, hidden, smoke-covered, or non-fireable retained targets are filtered out before
  ranking and cannot bypass legality.
- The auto-acquisition ranker chooses only for the current default attack profile. Future grenades,
  satchels, sticky bombs, melee demolition, or other special attacks must be represented as separate
  profiles with explicit activation policy; explicit-only special attacks can be added without
  changing default auto-acquisition, and autocast special attacks need their own conservative plan
  and tests.
- Resource costs are paid at execution time, not queue time. Queued abilities that become
  unaffordable at promotion are skipped or rejected, but queued and immediate build orders do not
  require current affordability at issue or promotion time. Build promotion checks the worker,
  faction/build requirements, map bounds, permanent footprint blockers, and resumable matching
  owned scaffolds; otherwise it records the build intent and walks the worker to an outside staging
  point near the footprint.
- When a worker reaches build-arrival range, construction re-validates the intent against the live
  world. Resuming an owned, matching scaffold at the footprint is free even if the owner cannot
  currently afford a new building. New scaffolds charge resources only when spawned. If the
  footprint is legal but the player lacks resources, the worker enters `WaitingAtSite`, clears its
  path, emits one shortage notice when entering the wait, and retries silently until resources are
  available. Waiting does not reserve resources, so another spend can still win the race before the
  scaffold appears.
- Build-arrival blockers are classified. A building, scaffold, resource node, terrain/out-of-bounds
  footprint, missing tech requirement, unknown building kind, or missing builder eligibility cancels
  the active build order. Relevant unit bodies put the worker in `WaitingAtSite` for up to
  three seconds (90 simulation ticks); if the unit blocker clears before timeout, normal placement
  and resource retry resumes, and if it persists through timeout the active build order is dropped
  with a single `Cannot build there` notice. Tank Trap placement keeps its vehicle-body-only policy:
  infantry-like bodies do not block it, while vehicle bodies use the same three-second unit-block
  grace. Clearing an active build order, completing construction, or losing a scaffold preserves
  any queued handoff orders; because queued promotion runs before construction in each tick, those
  handoffs promote on the next eligible promotion pass after the active build order becomes idle.
  Constructed buildings spawn with their full max HP but only 10% current HP, then linearly gain
  current HP with construction progress until completion restores them to full health; prebuilt
  starting buildings are unchanged.
- Omitted `queued` means immediate. Ordinary immediate unit orders replace active state and clear
  future intents. `stop` always clears both active and queued unit orders.
- Queueable commands append future unit-local intents. Unit queues are capped at 8 intents today;
  a valid append rejected only because the queue is full should emit a player notice.
- Invalid commands are no-ops except where a notice is explicitly useful. Stale queued stages are
  skipped at promotion time rather than retried forever.
- Queue planning is issue-time only, but ability queue policy decides what "eligible" means.
  `QueueSkipIfNotReady` abilities, such as Smoke, require a ready carrier at issue time and are
  skipped if stale at promotion. `QueueWaitUntilReady` abilities, such as Mortar Fire, may append
  while the carrier is otherwise valid but on ability cooldown or weapon reload; promotion turns the
  stored intent into an active ability order that waits for readiness before firing. Finite-use
  abilities also reserve already-active and already-queued same-ability intents at issue time, so
  queued Smoke clicks cannot exceed the scout car's remaining smoke uses.
- Later orders still apply to every compatible selected unit. Earlier specialized stages do not
  remove non-carriers from the plan; for example, a queued smoke applies to one scout car, while the
  following queued attack-move applies to all selected units that can receive attack-move.

Allocation rules:

- Point orders (`move`, `attackMove`) apply to every selected owned unit that can receive orders.
- Target/resource orders apply to every selected compatible owned unit after the target has passed
  issue-time validation. Occupied resource nodes are still valid gather targets; when a gatherer
  arrives and the patch is already occupied, the economy service redirects it to the nearest
  unoccupied same-resource node within ten tiles, or moves it to nearby open grass if none exists.
  Build and Tank Trap deconstruct orders allocate one compatible selected worker per click after the
  target has passed issue-time validation: immediate orders prefer idle workers and then closest
  worker to the footprint/target center; queued orders prefer the lowest work assignment load, then
  closest worker. Work assignment load is the worker's current queued-order count plus one when its
  active order is already a build or deconstruct intent. Deconstruct targets must be completed Tank
  Traps; friendly/allied traps are always legal targets for their team's workers, while enemy traps
  must be visible when accepted or promoted. Deconstruction takes half of the Tank Trap's build
  time, is not accelerated by assigning multiple workers to the same trap, and refunds the Tank Trap
  cost to the deconstructing player.
- Legacy Charge has no eligible carriers after the Methamphetamines research conversion. It remains
  decodable for old command logs but does not create queued/immediate ability work, cooldowns, or
  runtime status.
- World-targeted abilities allocate one carrier per click. For queued commands the planner chooses
  an eligible selected carrier with the shortest current queue, which gives round-robin behavior
  across repeated clicks. Skip-if-not-ready abilities require a ready carrier at issue time;
  wait-until-ready abilities may use a carrier whose cooldown or weapon cycle is still pending. If
  all eligible carriers are full, emit queue full notices; if no carrier is eligible at issue time,
  ignore the click.
- Immediate world-targeted abilities may be noninterrupting when the ability can fire now without
  replacing the active order. This is the reactive smoke case: a moving scout car that already has
  the target in range may launch smoke and continue its previous move and queued plan. If a
  world-targeted ability cannot execute noninterruptingly, the immediate order may replace the
  chosen idle caster's active order with an ability movement order. Abilities may also explicitly
  allow interrupting a moving caster; manual Mortar Fire uses that path so a non-queued fire order
  stops the mortar's current movement and clears future queued intents.
- Anti-Tank Gun setup is a queueable facing intent for selected Anti-Tank Guns only. The stored point means
  "face toward this world point from wherever the gun is when the setup stage promotes"; mixed
  selections ignore non-setup-capable units for setup but keep them for later compatible orders.
- Artillery Point Fire and Blanket Fire are queueable, terminal per-gun fire orders. Issue-time
  admission stores a locked effective fire point for Point Fire or locked blanket center for
  Blanket Fire, not the raw clicked point. Immediate fire commands can accept packed artillery and
  set it up in place, or redeploy already-deployed artillery when the effective point is outside the
  current cone. Queued fire commands lock from the active or queued future move destination when
  available, use a preceding queued setup stage as the zero-length fallback facing, and promote into
  the same setup/redeploy-owned fire order. Promotion and firing recheck liveness, ownership, kind,
  construction state, path state, faction ability eligibility, stored target map/range/cone
  validity, ammo, and deployment before any shell is launched.
- Client world-view and minimap previews may temporarily combine local pending move/setup/fire
  stages with owner-only `orderPlan` snapshots so queued Point Fire and Blanket Fire appear to aim
  from the future origin before the server echo arrives. This is preview-only: command admission,
  target locking, terminal queue behavior, stale-stage skipping, and the projected `orderPlan`
  remain server-owned. Minimap targeting sends the same world-coordinate `useAbility` command as
  viewport targeting; there is no separate minimap simulation path.
- Point Fire samples normal artillery scatter around the stored point, with Ballistic Tables
  tightening repeated Point Fire shots only. Blanket Fire samples each shot deterministically and
  uniformly within `ARTILLERY_BLANKET_RADIUS_TILES` around the stored center using authoritative
  shot inputs; sampled impacts are not re-clamped to the artillery cone or range band. Both modes
  share ammunition cost, reload, shell delay, impact radius, damage, fog-gated impact events,
  global firing markers, and firing-reveal behavior.

Examples:

- **Smoke wall then attack.** The player right-clicks selected scout cars to move to a staging
  point, holds Shift, holds/taps Smoke, and clicks four smoke targets. Each smoke click appends one
  smoke intent to one ready scout car, rotating across eligible cars by queue length. The player
  then keeps Shift held, arms Attack, and clicks attack-move points. The smoke carriers execute
  their smoke stages before the later attack-move; selected non-carriers skip smoke and still
  receive the attack-move stages.
- **Legacy Charge in queues.** Old command logs may contain queued Charge stages. Because Charge no
  longer has eligible carriers, those stages are skipped and later queued orders still promote.
- **Packed anti-tank guns.** The player orders packed anti-tank guns to move, then Shift-arms setup and clicks a
  facing point. The setup stage promotes after movement and computes facing from the gun's actual
  arrived position toward the stored world point.
- **Move then queued artillery fire.** The player orders artillery to move, then Shift-arms Point
  Fire or Blanket Fire and clicks a target. The queued stage stores the effective fire point or
  blanket center locked from the future move destination; when promoted, the gun sets up or
  redeploys in place and never walks to repair range.
- **Reactive moving smoke.** A scout car already moving past cover receives an immediate Smoke
  command for an in-range point. The planner emits a noninterrupting ability execution, so the
  smoke launches without dropping the car's current move order or queued future orders.

Promotion is centralized in `services::order_queue`: idle units, arrived/path-failed move orders,
completed or invalid explicit attacks, and completed ability movement orders pop the next valid
intent. Move and attack-move promotions are batched by owner/destination through deterministic
`BTreeMap` ordering, while attack, gather, build, setup, and ability promotions are issued per unit.
Active gather and build orders remain terminal until their own systems mark them complete or clear
them.

Production buildings keep an owner-only rally plan capped at four stages. Non-queued `setRally`
replaces the whole plan; queued `setRally` appends a `move` or `attackMove` stage if space remains,
or establishes the first stage when the plan is empty. Newly produced units receive the first rally
stage as their active order and any later stages as queued unit-local intents, so every trained unit
follows the same accepted building rally chain. The first stage also drives spawn-exit and vehicle
facing preference.

`game::smoke::SmokeCloudStore` owns active neutral smoke clouds as world effects, not entities:
clouds have stable ids, center points, radius, spawn tick, and expiry tick, and they do not
participate in pathing, collision, scoring, supply, or target queries. Scout-car smoke launch
schedules a pending cloud rather than spawning it immediately: impact is delayed by up to 3 ticks
(100 ms) at max range and scales down with launch distance. The server emits a transient
owner-visible `smokeLaunch` event with caster and target positions for the client canister visual,
but the projectile itself is not simulated as an entity. `services::line_of_sight`
owns terrain raycasts used by fog and combat and can be constructed with the active smoke store or
a fog-only building-footprint blocker mask as dynamic blocker input. Stone/rock tiles block vision
and ranged attacks. Non-Tank-Trap building footprints block authoritative fog for every player, so
units do not see through friendly or enemy structures; the visible edge of a building footprint can
still reveal that building's footprint for projection. Blocking buildings stamp their own footprint
as visible but do not project sight through themselves. Fog may reveal the blocking stone tile
itself and the visible edge of a smoke cloud, but not tiles behind blockers.
Units inside smoke do not stamp vision; friendly units inside smoke remain owner-visible through
projection, while enemy units inside smoke are withheld and cannot be targeted. Combat
auto-acquisition and firing both use the smoke-aware LOS query; explicit attack orders may chase
toward terrain- or smoke-blocked targets but cannot fire until the shot is clear. Direct-fire shot
projection also checks hard entity blockers: tanks and non-Tank-Trap building footprints intercept
shots, while Tank Traps do not. Future forest visibility/cover rules should extend the terrain rules
and this service instead of adding ad hoc checks to fog or combat.

`Game` still recomputes raw live fog per player after each tick, because command validation and
combat targeting depend on owner-local current vision. Event visibility and building-memory
refreshes use team-current views derived from those raw live grids. Normal player snapshots then
build a temporary team-current fog by unioning the raw live grids of living teammates only.
Hostile unit shots from outside a victim player's current live fog add temporary firing-reveal
sources to live fog for players on the victim's team, not for third-party observers who merely see
the combat event. These sources reveal only the firing unit's current tile, are actionable for
command validation and combat targeting, and expire at
`fired_at_tick + firing_cycle_cooldown + TICK_HZ / 2` so the duration tracks the weapon's firing
cycle plus 0.5 seconds. Combatants that first engage a target through one of these firing-reveal
sources spend a one-second response delay before their first counter-shot, so firing-reveal
counterfire plays out as shot/counter-shot rather than an instant simultaneous chain.
Lingering death sight is stamped into live fog as ordinary temporary team sight for five seconds.
The invisible source stays at the dead unit/building's final position, uses that entity's sight
radius, respects smoke and line-of-sight blockers, and is stamped into every tracked teammate fog
grid. Because it is normal fog, snapshots project current enemy positions without `visionOnly`,
direct and queued attacks validate through it, remembered buildings/trenches refresh from it, and
idle/attack-move auto-acquisition can choose targets it reveals. Unit live fog stamps a
center-origin sight circle. Building live fog stamps the whole building footprint plus `sight_tiles`
outward from each footprint edge, so a building with 1-tile sight sees itself and the one-tile
perimeter around its edges. Neutral resource nodes never stamp vision.

`game::building_memory::BuildingMemory` is server-only stale intel owned by `Game`. After live,
smoke-aware fog is recomputed, the store records one latest-seen entry per
`(viewer_player_id, enemy_building_entity_id)` for non-neutral enemy buildings currently
projectable to that viewer through team-current actionable fog. Records copy id, owner, kind,
center position, footprint tiles, hp/max hp, construction progress/completion state, and the tick
observed. Five-second lingering death vision is normal temporary fog, so it refreshes remembered
building intel while the source remains active. If a remembered building no longer exists, the
record remains while its footprint is hidden from the viewer's team and is removed once that team
scouts any remembered footprint tile. This keeps hidden destruction stale until the location is
checked without adding any wire-protocol fields.

`services::geometry` owns shared body primitives: infantry unit bodies are circles centered on
`(x, y)` with the configured unit radius, tanks use an oriented vehicle hull derived from their
body `facing`, configured length/width, and a small clearance margin, building bodies are
axis-aligned rectangles derived from footprint tiles, and resource node bodies are circles for
build-site blocking. `services::occupancy` separates terrain, all-ground static blockers, physical
vehicle-body-only blockers, and the owner-aware path-planning view of vehicle-body-only blockers.
Tank Trap pairs exactly two tiles apart close the single tile between them for physical vehicle body
legality while remaining infantry-passable and shot-transparent. Vehicle path planning keeps own and
allied Tank Traps in the static blocker layer but treats enemy Tank Traps as breachable obstacles, so
paths can route into an enemy wall and combat can attack it. Path-cache fingerprints include the same
owner/team relation as the path request. Movement, collision, and standability still use physical
legality, so live enemy Tank Traps are not globally non-colliding.
`services::standability` owns reusable legality predicates for unit bodies and building sites.
Production spawn exits, construction/build intent, movement landing, steering
candidates, collision push targets, and formation goal selection all use this shared standability
layer for static/body legality. Swept segment checks sample the same body shape along a straight
segment, and broad-phase queries use each body's conservative bounding radius. Movement separates
oriented vehicle body legality from drive behavior: tanks and anti-tank guns use pivot-drive
locomotion that can rotate in place before advancing, while scout cars use car-drive path following
where hull facing changes through translation/curvature. These helpers are pure and do not change
the wire protocol or client contract.

---
