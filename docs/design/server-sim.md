## 3. Rust server — modules & the Game core API

Crate layout (`server/`):
```
Cargo.toml
src/
  main.rs        # tokio runtime, axum router: static files + /ws, room manager task
  protocol.rs    # server-shell protocol adapter shim; serde DTOs live in crates/protocol
  config.rs      # server-shell balance shim; authoritative values live in crates/rules
  lab_scenarios.rs # bundled lab checkpoint setup manifest loader and restore validator
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
    /// Loads the hardcoded handcrafted map and assigns ordered players to fixed authored start
    /// locations. A map owns flat `startLocations` and `baseSites`: start locations determine its
    /// capacity, while every base site always receives its resource cluster. Singleton-team FFA
    /// matches shuffle fixed start locations by `seed`; team matches choose ordered starts from the
    /// same fixed set, preferring lower teammate spread, higher nearest enemy-team distance, lower
    /// exposure imbalance, and finally a deterministic seed-influenced tie break. No player-count
    /// layouts or player-owned natural groups exist in authored maps. Generated oil clusters place each oil patch on a unique passable tile
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
    /// Plural LabOp variants validate and commit atomically through this same seam.
    pub fn apply_lab_op(&mut self, op: lab::LabOp) -> Result<lab::LabOpOutcome, lab::LabError>;

    /// Export map-only Lab state for the dedicated editor boundary; excludes all simulation state.
    pub fn export_lab_map(&self) -> protocol::LabMapDraft;

    /// Export authoritative lab setup data as a checkpoint-backed setup container. The JSON-friendly
    /// transport name is still `LabScenarioPayload`, but the payload is `LabCheckpointScenarioV1`.
    pub fn export_lab_checkpoint_scenario(
        &self,
        name: String,
        server_build_sha: &str,
    ) -> Result<lab::LabCheckpointScenarioV1, lab::LabError>;

    /// Restore a checkpoint-backed setup into a fresh lab `Game` after validating map binding,
    /// checkpoint shape, player metadata, lab metadata, and entity-id remap metadata.
    pub fn restore_lab_checkpoint_scenario(
        scenario: lab::LabCheckpointScenarioV1,
    ) -> Result<Game, lab::LabError>;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply domain command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand);

    /// Ordinary retreat commands for AI-owned workers hit on the previous tick.
    pub fn worker_retreat_commands_for(&self, player: u32) -> Vec<SimCommand>;

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick. Entity visibility and
    /// visibleTiles use living-team current fog and exploredTiles use server-owned team history;
    /// resources/upgrades stay local.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Same projection as `snapshot_for`, with explicit room-projection diagnostic options such as
    /// owner-only movement paths. The default `snapshot_for` includes no movement diagnostics.
    pub fn snapshot_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot;

    /// Build a read-only privileged observer projection. `Omniscient` exposes the complete
    /// world/all-owner private detail; `Players` combines selected real-player perspectives.
    /// This value never conveys command authority.
    pub fn snapshot_for_observer(&self, view: &ObserverView) -> Snapshot;
    pub fn snapshot_for_observer_with_options(&self, view: &ObserverView, options: SnapshotOptions) -> Snapshot;

    /// Build a spectator snapshot from the union of the selected players' current fog, stale
    /// building memory, and resource rows.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot;

    /// Same projection as `snapshot_for_spectator`, with explicit room-projection diagnostic options.
    pub fn snapshot_for_spectator_with_options(&self, visible_players: &[u32], options: SnapshotOptions) -> Snapshot;

    /// Build a full-world snapshot for a room projection that intentionally exposes all state,
    /// including per-entity planning details through each entity's real owner. Normal gameplay
    /// must not use this.
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot;

    /// Same full-world projection, with explicit room-projection diagnostic options.
    pub fn snapshot_full_for_with_options(&self, player: u32, options: SnapshotOptions) -> Snapshot;

    /// Player ids still alive. Humans need at least one building; AI players also need a unit.
    pub fn alive_players(&self) -> Vec<u32>;

    /// Player ids whose starting main base is still alive. AI-only live outcome checks use this
    /// objective query instead of the normal elimination rule.
    pub fn primary_base_alive_players(&self) -> Vec<u32>;

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
command-log contract. `rts-rules::faction::UpgradeKind` is re-exported through `game::upgrade`
because `SimCommand::Research` carries it and external AI controllers construct ordinary
`SimCommand`s. `CommandLogEntry.command` remains
the serde `Command` from `rts-protocol` so replay JSON stays wire-compatible. `StartPayload`,
`Snapshot`, `Event`, and `PlayerScore` are also serde types from `rts-protocol`.

`PlayerInit.is_ai` marks a computer-controlled player. AI players are full players in every
respect (they get a start position, City Centre, workers, economy, and count toward
win/elimination); the only difference is they have no socket. `Game` does not own AI controllers;
the room task or tool harness asks `rts-ai` controllers for ordinary `SimCommand`s and enqueues
them through this API before ticking — see §8.

Lab mutation types live under `game::lab`. `LabOp` is intentionally narrow rather than a debug
backdoor: entity mutations validate known unit/building kinds, real players, finite in-map
positions, placement/collision legality, and stale ids before changing the world. Plural spawn,
update, and delete requests accept 1–400 items, run against a cloned scratch `Game`, preserve input
ordering, and replace live state only after the whole request succeeds. Non-move updates apply to
scratch state in order; every moved entity is then removed from scratch occupancy before
destinations are validated, so swaps and translations are simultaneous while destination conflicts
remain illegal. Accepted batches clear stale orders and reservations where needed and repair supply,
derived state, fog, and building memory once. Placement errors carry the attempted point, typed
entity/feature/terrain/boundary blockers, and up to eight deterministic suggestions produced by the
same standability predicate against the transactional prefix. Lab setup import/export uses checkpoint-backed
`LabCheckpointScenarioV1` containers: materialized map data and binding live beside an embedded
`GameCheckpointV1` payload, and small lab metadata carries setup name, exported tick, selected
vision, god-mode players, and optional source entity-id remap rows. Restore validates map binding,
checkpoint payload, player and lab metadata, repairs derived state, and returns the id remap for
callers that need to reconcile UI selection. Snapshot-only projections, transient events,
projectile runtime state, active commands, production queues, rally plans, cooldowns, and command
logs are not part of the setup format. Legacy `kind:"labScenario"` setup JSON is rejected before it
can mutate a lab room.

#### 3.1.1 `Game` State Ownership Registry

`Game` is a private ownership tree with only two top-level state roots:
`GameState` (`server/crates/sim/src/game/state.rs`) for durable authoritative state and setup/replay
compatibility metadata, and `DerivedState`
(`server/crates/sim/src/game/derived_state.rs`) for rebuildable cache/search state. Both owners are
private to `crate::game`; public `Game` methods remain the only seam for lobby, replay, lab, AI,
server, and snapshot callers.

This registry is the source of truth for classifying fields currently owned by `GameState` or
`DerivedState`. Any change that adds, removes, or changes the semantics of a `GameState` or
`DerivedState` field must update this table in the same change. The categories are:

- `authoritative/serialized` - durable simulation state that can affect future ticks, command
  validity, fog/projection, replay output, scoring, entity ids, or checkpoint restore.
- `derived/rebuildable` - cache, index, or search state that can be cleared at a tick boundary and
  rebuilt from authoritative state without changing semantic results or fog-filtered snapshots.
- `transient` - intentionally dropped runtime state that cannot affect future authoritative
  behavior.
- `compatibility metadata` - replay/API/setup metadata retained with an explicit checkpoint policy,
  even when it does not directly mutate tick results.

No current state field is classified as `transient`; room/session transient state remains outside
`Game` in `server/src/lobby/`. Field names in this table must match current `GameState` or
`DerivedState` fields exactly; `rts-archcheck` treats missing, stale, or wrongly categorized rows as
architecture failures.

| Field | Category | Checkpoint policy | Evidence and notes |
| --- | --- | --- | --- |
| `map` | `authoritative/serialized` | Internal cold checkpoints serialize the full live `Map` value. Public `GameCheckpointV1` payloads do not embed a map body; they carry `mapBinding` facts and import only with the exact container-supplied `Map`. See §3.1.3. | `Game::new_inner_with_map` stores the generated or supplied map; `systems::run_tick`, pathing, fog, placement, resource setup, and `start_payload` all read it. Runtime ownership and external artifact composition are intentionally separate contracts. |
| `entities` | `authoritative/serialized` | Serialize the full `EntityStore`, including stable entity ids, allocator/high-water state, HP, orders, queues, movement state, selected waypoints, path goals, weapon cooldowns, ability charges and charge-recharge timers, episode-keyed firing-reveal reaction gates, combat state, production/build progress, rally plans, Scout Plane source-car/orbit/remaining-lifetime state, resource reservations, body/weapon/setup facing, and entity flags. | `systems::run_tick` mutates the store every tick; snapshots, score, survival, command validation, replay determinism tests, and the Phase 0.5 comparator all treat entity state as semantic authority. Chosen movement paths and aerial orbit state live on entities, not in `pathing`. Scout Plane entities are excluded from standard fog sight stamping and contribute independent team aerial vision through the dedicated smoke-only pass. |
| `fog` | `authoritative/serialized` | Serialize the latest 15 Hz actionable visibility sample and its bounded per-viewer firing-reveal provenance map. | `recompute_live_fog` atomically records whether each sampled revealed entity needed its firing reveal before stamping the actionable tile. Combat, commands, and entity projection consume that held sample; snapshot `visibleTiles` removes reveal-only stamps so presentation fog remains covered. Phase 0.5 compares per-player actionable tiles as semantic state. |
| `building_memory` | `authoritative/serialized` | Serialize remembered enemy-building entries per player. | `BuildingMemory::refresh` records last-seen enemy building state and only removes hidden destroyed entries after the footprint is scouted again; spectator/player snapshots project remembered buildings while fogged. |
| `players` | `authoritative/serialized` | Serialize all `PlayerState` rows, including id/team/faction/name/color/start tile, current Steel/Oil, supply, AI flag, score counters, mined-resource lifetime totals, rolling mined-resource income history, and completed upgrades. | Economy, command authority, team relations, alive checks, scores, observer-analysis resource income, faction-specific tech, and snapshot resource rows are all read from `players`. |
| `pending` | `authoritative/serialized` | Serialize unapplied pending commands unless a future checkpoint caller explicitly proves it captures only immediately after command drain with `pending` empty. | `Game::enqueue` appends commands between ticks; `tick_inner` drains `pending`, records them in `command_log`, and applies them. Dropping a non-empty queue would skip authoritative player/AI intent. |
| `command_log` | `compatibility metadata` | Serialize command history for replay/crash/API continuity; do not replay it during normal checkpoint import unless building a replay artifact. | `Game::command_log` is public, schema 3 replay artifacts finalize the stored launch-time `ReplayStartComposition` with this log, and tick logic only appends/applies new pending commands instead of reading old log entries. |
| `tick` | `authoritative/serialized` | Serialize the exact tick count. | `tick_inner` increments it before systems, logs commands with it, drives expirations/cooldowns, passes it to projection/runtime stores, and advances `PathingService` to the same tick. |
| `last_world_combat_tick` | `authoritative/serialized` | Serialize the most recent hostile-weapon activity tick. | Records exact observation time without exposing it to clients. Snapshot projection uses the separate published deadline below. |
| `last_world_combat_position` | `authoritative/serialized` | Serialize the latest authoritative combat-area centroid before publication. | The point is held between 15-tick publication boundaries so direction changes do not expose live weapon cadence. |
| `world_combat_active_through_tick` | `authoritative/serialized` | Serialize the last published 15-tick boundary through which the global signal is active. | Keeping published state separate from exact observation prevents new activity inside a quantization bucket from suppressing an already-active signal while preserving boundary-only changes. |
| `world_combat_position` | `authoritative/serialized` | Serialize the published coarse combat point used while the global signal is active. | The point is snapped to a 32-tile grid and shared identically with all projections for direction-only background audio. |
| `lingering_sight` | `authoritative/serialized` | Serialize all active death-vision sources and their expiry ticks. | `retain_active_visibility_sources` prunes by `tick`; the next 15 Hz fog sample stamps active sources into team fog, affecting command legality, combat visibility, and snapshots. |
| `firing_reveals` | `authoritative/serialized` | Serialize active firing-reveal sources with stable episode-start and expiry ticks. | Anti-Tank Gun and artillery/mortar reveal logic records temporary actionable sight. Repeated shots extend one continuous episode without changing its start; the next 15 Hz fog sample stamps active sources into viewer fog until a later sample observes expiry. |
| `smokes` | `authoritative/serialized` | Serialize the full `SmokeCloudStore`, including next id, active clouds, pending clouds, locations, radii, spawn/due/expiry ticks. | Smoke blocks line of sight, combat projection, and the next authoritative fog sample; `tick_inner` retains active smoke and systems may resolve pending smoke. |
| `trenches` | `authoritative/serialized` | Serialize the full `TrenchStore`, including deterministic trench ids, terrain positions, discovery/memory data, and any store allocator state. | Trenches are persistent neutral terrain outside `EntityStore`; entrenchment services create/discover/update them and snapshots project current plus remembered trench terrain. |
| `ability_runtime` | `authoritative/serialized` | Serialize active ability runtime state, object ids, world objects, projectiles, cooldown-linked runtime payloads, and expiry/return data. | `AbilityRuntime` owns deterministic active instances and non-entity world objects; systems and snapshots read it for Ekat return markers, line projectiles, anchors, and owner/enemy projection. |
| `mortar_shells` | `authoritative/serialized` | Serialize all scheduled mortar impacts with owner, attacker, impact point, and impact tick. | `MortarShellStore::schedule` records delayed impacts; later ticks resolve area damage/events even if the firing mortar dies before impact. |
| `artillery_shells` | `authoritative/serialized` | Serialize all scheduled artillery impacts with their owners, source data, impact points, and impact ticks. | The artillery store mirrors the delayed-shell contract used by the tick pipeline; dropping it would cancel future area damage and reveal/event output. |
| `panzerfaust_shots` | `authoritative/serialized` | Serialize all launched Panzerfaust loaded shots with owner/source facts, locked target id, launch-safe impact point, and impact tick. | `PanzerfaustShotStore::schedule` records detached loaded-shot impacts; later ticks resolve the direct damage/event even if the firing Panzerfaust dies before impact. |
| `seed` | `compatibility metadata` | Serialize the original match seed as setup/replay metadata. Do not use it as a substitute for `rng`. | Constructors and replay artifacts expose `seed`; the current map is stored separately and the current random stream lives in `rng`. |
| `starting_loadouts` | `compatibility metadata` | Serialize per-player starting loadout records for replay/setup compatibility. | Replay constructors and artifacts persist these records so mixed faction/resource starts can be reconstructed without a global resource pair. |
| `map_metadata` | `compatibility metadata` | Serialize stable authored map identity/version metadata alongside the map. | Replay/lab setup paths expose map metadata; it identifies the authored map but is not the live terrain grid used by systems. |
| `active_construction_sites` | `authoritative/serialized` | Serialize the set when checkpointing a post-tick state that may immediately produce snapshots; otherwise it should normally be empty at the next tick start. | `tick_inner` clears it before systems; `construction_system` inserts sites that received progress this tick; `snapshot` passes it to projection so same-tick construction progress is visible. |
| `lab_god_mode_players` | `authoritative/serialized` | Serialize the canonical enabled-player set and resync mirrored entity invulnerability flags after import. | Lab ops mutate the set; `sync_lab_god_mode_flags` mirrors it onto unit/building invulnerability, and damage checks consume those entity flags. |
| `starting_loadout` | `compatibility metadata` | Serialize until legacy/global-starting-resource compatibility constructors are retired. Checkpoint import should prefer `starting_loadouts` for per-player setup facts. | The field is set by setup constructors and dev scenarios but does not feed the per-tick systems after match creation. |
| `rng` | `authoritative/serialized` | Serialize the exact current generator state or an equivalent deterministic draw-stream state. Re-seeding from `seed` is not valid after any random draw. | `systems::run_tick` passes `&mut self.state.rng` into combat damage/miss logic; Phase 0.5 probes cloned RNG output as semantic state. |
| `final_spatial` | `derived/rebuildable` | Do not serialize. Rebuild with `SpatialIndex::build(&entities, map.size)` after import, after lab mutations that change entity positions/existence, and after any derived-state wipe. | `DerivedState` owns the final post-tick spatial index used by snapshots. `tick_inner` stores the final `systems::run_tick` spatial result, and Phase 0.5/2/4 checkpoint proofs compare snapshots after clearing and rebuilding it. |
| `pathing` | `derived/rebuildable` | Do not serialize reusable pathing cache/search entries. Recreate `PathingService` with the live default budget, cache capacity, and current tick alignment during import or derived-state rebuild. | `PathingService` cache/search bookkeeping only affects performance. Chosen unit paths, movement phases, waypoints, path goals, and throttling remain serialized on entities. Phase 0.5/2/4 tests prove clearing this cache does not change semantic state or fog-filtered snapshots. |

The Phase 0.5 and Phase 2 derived-state wipe harnesses confirm the current derived boundary: only
`final_spatial` and `pathing` are cleared and rebuilt. Every current `GameState` field is treated as
authoritative or compatibility metadata until a later phase adds a deterministic rebuild proof and
updates this registry. No current field has an unresolved ownership category or checkpoint-policy
blocker.

#### 3.1.2 Ownership guardrails and checkpoint-readiness audit

`server/crates/archcheck` turns the registry above into an executable guardrail. The
`check-sim-architecture` command fails if:

- `Game` stores any top-level state field other than `state: GameState` and
  `derived: DerivedState`;
- a `GameState` or `DerivedState` field is missing from the registry, has a stale registry row, or
  is categorized outside its owner (`GameState` accepts `authoritative/serialized` or
  `compatibility metadata`; `DerivedState` accepts only `derived/rebuildable`);
- a registry row omits a concrete checkpoint policy or evidence/notes cell, which would allow a
  new authoritative state owner to bypass the DTO/import decision record;
- production `rts-sim::game` code adds module-level mutable state such as `static mut`,
  `Mutex`/`RwLock` statics, `OnceLock`/`LazyLock` mutable singletons, or `thread_local!` state;
- a service module lacks a role classification, adds an unapproved service edge, exposes broad
  mutable world-state APIs, or grows direct entity/player mutation sites beyond the baseline.

Durable state owners belong under `GameState`; rebuildable cache/index/search owners belong under
`DerivedState`. Services may own invariants and narrow mutation/query APIs, but long-lived state
that changes future authoritative behavior must be stored in the explicit tree. Room/session
exceptions stay documented in §3.2 and `server/src/lobby/`: sockets, room identity/lifecycle,
replay session cursors/keyframes, replay branch seeds, lab timeline keyframes and durable replay baselines/operation entries, selected vision,
participant capabilities, AI controller memory, persisted match-history writes, and test fixtures
are outside the cold checkpoint contract unless a future plan promotes them into authoritative
recorded actions.

Private checkpoint export/import remains internal test machinery. `GameCheckpointV1` payload helpers
under `rts-sim::game` are used by the semantic comparator and validation tests; they do not define a
public route, command, replay artifact format, lab setup format, UI affordance, or lobby/server
call path by themselves. Lab world mutations reset the full `DerivedState` shell before rebuilding
spatial state, so pathing cache and search state are cleared at the same rebuildable boundary.

Phase 7 release audit for the ownership sequence:

- Public `Game` API signatures remain the §3.1 seam. Lobby, replay, lab, AI, server, and snapshot
  callers still go through public `Game` methods rather than reading internal state owners. Normal,
  replay-compatible, lab, and dev-scenario setup composes a map plus `GameCheckpointV1`, then
  validates checkpoint import before the authoritative start becomes live. The private
  `new_direct_start_for_test` path is retained only as a setup parity oracle.
- Wire protocol mirrors, protocol DTOs, compact snapshots, start payloads, and
  `client/src/protocol.js` are not changed by the private checkpoint path.
- Replay artifact schema 3 captures the replay start state as a launch-time
  `ReplayStartComposition` containing the map binding plus `GameCheckpointV1`, then finalizes with
  recorded commands and end metadata. Schema 2 and older replay artifacts reject with the
  unsupported-schema error. Replay seek still uses recorded commands plus in-process
  `clone_for_replay_keyframe` keyframes after the start game is rebuilt. Schema, map, faction, and
  loadout drift reject with explicit messages while build-SHA drift remains warning-compatible.
- Lab timeline seek still replays lab timeline entries from in-process keyframes. Current lab
  import/export UI uses checkpoint-backed `LabCheckpointScenarioV1` containers:
  materialized map data/binding lives beside an embedded `GameCheckpointV1` text payload, and
  `sourceEntityIdMap` preserves existing import remap callers. Old `kind:"labScenario"` setup
  files are rejected by the checkpoint-only payload parser.
  Portable lab sessions use the protocol-owned `LabReplayArtifactV1` contract: an initial
  checkpoint-backed lab setup plus ordered replayable lab mutations and issue-as commands. The
  portable stream is not `LabSession.operation_log` and not retained `LabTimeline` keyframes;
  `setVision` stays per-viewer session metadata, while checkpoint setup import rebases the
  artifact by replacing the initial setup and clearing prior operation entries.
- Projection privacy remains enforced by normal snapshot/event projection tests plus
  checkpoint/privacy coverage; checkpoint helpers must not expose fog-hidden entity ids, positions,
  targets, ability payloads, remembered occupants, or private events.
- Persistence is unchanged outside replay artifact contents: match history stores the same
  versioned `ReplayArtifactV1` JSON row, now with schema 3 `startState` for new writes, and the DB
  reader rejects schema 2 and older artifacts the same way file/dev replay loading does.
- Operational rollback remains a normal server rollback or revert. New schema 3 artifacts are
  rejected by old binaries that do not know the schema; current binaries reject removed schema 2
  replay artifacts and legacy lab setup JSON, so old artifacts need to be recreated through the
  current checkpoint-backed lab setup or replay flow. No generic checkpoint upload route exists
  outside the bounded lab setup/replay surfaces.

Remaining follow-up product decisions are deliberately outside this checkpoint plan: whether to
replace in-process replay/lab keyframes with checkpoint keyframes, and whether a separate product
plan should expose any public checkpoint save/load surface with rollout observability.

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
player whose team still has a living member still receives that surviving team vision.
`exploredTiles` is accumulated authoritatively from the effective team fog after all ordinary,
Scout Plane, lingering-death, and firing-reveal stamps, then persisted in checkpoints. Allied
visible entities project full read-only inspection details, but command authority, economy,
resources/supply/upgrades, rally/order plans, ability controls, and debug path overlays remain
exact-owner-only. Combat target ids and weapon facing for allied entities are projected only when
the target is team-visible, so allied inspection does not reveal hidden enemy ids or directions.
Attack events may carry a closed `weaponKind` feedback hint, but it is added only to attack events
that were already projected and must not change event recipients or reveal hidden target facts.
Missed direct shots emit a separate `miss` event to that same projected attack-recipient set and
carry only the target id, so the client can anchor text to an already visible unit without receiving
a hidden position.
Victory resolution is team-aware:
the room task ends 2+ player matches only when at most one nonzero team still has an alive member,
and a defeated player does not receive an individual loss screen while any teammate keeps that team
alive.

#### 3.1.3 `GameCheckpointV1` Embeddable Payload Contract

`GameCheckpointV1` is the first public checkpoint payload contract. It is a versioned UTF-8 JSON
text payload that can be embedded inside a replay artifact, lab setup container, match-start
artifact, or debug document. It is not a standalone product file format, network command, route, UI
option, replay schema change, or lab schema change by itself. Schema 3 replay artifacts may embed a
checkpoint at a nonzero start tick; replay start capture rejects games with pending commands so
playback cannot duplicate pending intent. Their top-level command log retains only commands after
that tick, and their top-level seed, player, and starting-loadout metadata must agree with the
checkpoint start state or artifact validation rejects them. The internal implementation exports
`Game + supplied map binding` through explicit DTOs to JSON text bytes, imports only with the exact
container-supplied `Map`, and rebuilds derived state before returning a live `Game`. Export sorts
entity DTOs by id so normalized payload text does not depend on entity-store iteration order, and
rejects payloads above the same v1 byte limit enforced by import.

The payload schema is named `rts.gameCheckpoint` and starts at version `1`. Field names are
camelCase. DTOs must be explicit Rust types with serde support; do not serialize private
`GameState`, `Entity`, store, service, or snapshot internals directly as the stable persisted
contract. Persisted DTOs should use strict deserialization (`deny_unknown_fields` or an equivalent
manual check) except for explicitly versioned compatibility metadata. Unknown required features
must fail closed; unknown optional features may be ignored only when the schema says the field is
non-authoritative.

Top-level shape:

```json
{
  "schema": "rts.gameCheckpoint",
  "version": 1,
  "compatibility": {
    "createdBy": "server|replay|lab|debug",
    "serverBuildSha": "...",
    "simSchemaVersion": 3,
    "rulesVersion": 1,
    "protocolVersion": 1,
    "requiredFeatures": [],
    "optionalFeatures": []
  },
  "mapBinding": {
    "name": "Chokes",
    "schemaVersion": 2,
    "contentHash": "...",
    "materializedMapHash": "...",
    "size": 64,
    "playerCount": 2
  },
  "seed": 1234,
  "tick": 900,
  "rng": {
    "algorithm": "rts-small-rng-0.8-draws-v1",
    "seed": 1234,
    "drawsConsumed": 0
  },
  "players": [],
  "startingLoadouts": [],
  "startingLoadout": {},
  "entities": {},
  "pendingCommands": [],
  "commandLog": [],
  "commandLogMetadata": {},
  "fog": {},
  "buildingMemory": {},
  "lingeringSight": [],
  "firingReveals": [],
  "smokes": {},
  "trenches": {},
  "abilityRuntime": {},
  "mortarShells": [],
  "artilleryShells": [],
  "panzerfaustShots": [],
  "activeConstructionSites": [],
  "labGodModePlayers": []
}
```

The `rng` field is a deterministic draw-stream descriptor, not a raw serde dump of `SmallRng`.
`drawsConsumed` counts draws from `seed` under the named algorithm. Import reconstructs the stream
by seeding and advancing, or rejects the payload if the running binary cannot prove the same
algorithm. Re-seeding from `seed` alone is invalid once any random draw has occurred. If a later
runtime replaces `SmallRng`, it must either provide a migrator from this descriptor or bump the
checkpoint version and reject old payloads with a compatibility error.

`commandLog` carries replay/API/crash continuity history. Normal checkpoint import installs the
log as metadata and must not replay it to reconstruct state. `commandLogMetadata` records the
command DTO/protocol version, first and last command ticks, whether the log is complete from tick
zero, and any replay-base tick used by a containing artifact. Replay containers may choose to
store a separate authoritative command stream, but they must not infer live state by replaying
`commandLog` during checkpoint import.

Map policy:

- `GameState.map` remains authoritative runtime state because systems read terrain, selected starts,
  and permanent base sites on every tick. Internal cold checkpoints may still clone the full `Map` while
  they are private test machinery.
- `GameCheckpointV1` never embeds map JSON, terrain bytes, starts, or base-site bodies. It
  embeds `mapBinding` only.
- The containing artifact supplies the exact map data. A replay artifact stores or references the
  launch-time map composition beside the checkpoint; a lab setup container embeds or references
  the authored map data it is editing; a match-start artifact references the selected map asset and
  frozen materialized map facts; a debug document may include a sibling `map` object next to the
  checkpoint payload for convenience.
- Import receives `(container metadata, exact supplied Map, GameCheckpointV1)`. Before constructing
  a live `Game`, it validates `mapBinding.name`, `schemaVersion`, authored `contentHash`, `size`,
  `playerCount`, and `materializedMapHash` against the supplied map. `materializedMapHash` is a
  stable hash over the live `Map` fields that affect simulation (`size`, row-major terrain,
  selected starts, and base sites). If any binding fact differs, the importer rejects the
  payload; it must not fall back to regenerating a map from seed or silently accepting a nearby map.

Field map for Phase 2 DTO conversion:

| Runtime field | `GameCheckpointV1` strategy |
| --- | --- |
| `map` | Excluded as a body; represented by `mapBinding`. Import writes the exact container-supplied `Map` into `GameState.map` only after binding validation succeeds. |
| `entities` | `EntityStoreV1` with allocator/high-water state and explicit entity DTOs. Entity DTOs must cover stable ids, owners, kind, HP, flags, construction/production/resource state (including whether an unfinished scaffold's construction cost was paid), combat cooldowns, targets, bounded reveal-reaction gates, body/weapon/setup facing, entity-local active orders, queued order intents, selected movement paths, selected waypoints, path goals, rally plans, Scout Plane source-car/orbit/remaining-lifetime state, reservations, occupants/transport-like references if added later, and all entity-local timers. |
| `fog` | `FogStateV1` latest sampled visibility grids plus bounded per-viewer firing-reveal provenance. The sampled provenance may lead or trail the current source list by one simulation tick, and ids remain bounded by the entity allocator high-water mark. |
| `building_memory` | `BuildingMemoryV1` remembered enemy-building entries per player, including last-seen state and footprint facts needed for projection after restore. |
| `players` | `PlayerStateV1` rows with id, team id, faction id, name, color, start tile, resources, supply, AI slot flag, score counters, mined-resource lifetime totals, rolling mined-resource income history, and completed upgrades. |
| `pending` | `pendingCommands` entries with issuer, command DTO, admission/lab context, and original order. Import preserves them so the first post-restore tick drains them exactly once. |
| `command_log` | `commandLog` plus `commandLogMetadata`. It is compatibility metadata, installed but not replayed by normal import. |
| `tick` | Top-level `tick`, serialized exactly. Timers and expiry ticks remain absolute tick values. |
| `last_world_combat_tick`, `last_world_combat_position`, `world_combat_active_through_tick`, `world_combat_position` | Optional top-level activity tick, pre-publication point, bounded 15-tick publication deadline, and published coarse point. Points must be finite and inside the bound map. |
| `lingering_sight` | `LingeringSightSourceV1` entries with owner/team visibility, position/radius, and expiry tick. |
| `firing_reveals` | `FiringRevealSourceV1` entries with revealer, viewer/team policy, stable episode-start tick, position or source facts, and expiry tick. |
| `smokes` | `SmokeCloudStoreV1` with next id, active clouds, pending clouds, locations, radii, spawn/due/expiry ticks, and owner/source facts. |
| `trenches` | `TrenchStoreV1` with deterministic trench ids, geometry, occupation/discovery/memory state, and allocator state. |
| `ability_runtime` | `AbilityRuntimeV1` with active ability instance ids, cooldown-linked runtime payloads, world objects, projectiles, return/expiry data, owner facts, and any visibility-relevant projection state. |
| `mortar_shells` | `MortarShellStoreV1` scheduled impacts with owner, attacker/source, impact point, damage/reveal facts, and impact tick. |
| `artillery_shells` | `ArtilleryShellStoreV1` scheduled impacts with owner/source, scatter/impact point, damage/reveal facts, and impact tick. |
| `panzerfaust_shots` | `PanzerfaustShotStoreV1` scheduled loaded-shot impacts with owner/source facts, locked target id, launch-safe impact point, and impact tick. |
| `seed` | Top-level `seed` compatibility value. It is retained for setup/replay metadata and for `rng.seed`, but it is never enough to restore current RNG state by itself. |
| `starting_loadouts` | `startingLoadouts` compatibility records, preserving per-player faction/loadout/resource start facts. |
| `map_metadata` | Folded into `mapBinding.name`, `schemaVersion`, and `contentHash`; not duplicated as an independent state body. |
| `active_construction_sites` | `activeConstructionSites` entity-id set. Import validates ids against `entities`; normally empty at tick boundaries but preserved for same-tick snapshot parity. |
| `lab_god_mode_players` | `labGodModePlayers` player-id set. Import validates players and resyncs mirrored entity invulnerability flags after restore. |
| `starting_loadout` | `startingLoadout` legacy compatibility object until global-start constructors are retired. Import prefers `startingLoadouts` when both are present and rejects contradictory values. |
| `rng` | Top-level `rng` draw-stream descriptor with algorithm, seed, and consumed draw count. |
| `final_spatial` | Omitted. Rebuild with `SpatialIndex::build(&entities, map.size)` after import and after any import repair. |
| `pathing` | Omitted. Recreate `PathingService` with the live default budget, cache capacity, and current tick alignment; selected unit paths and goals are serialized on `entities`. |

Snapshots, compact snapshots, fog-filtered events, observer projections, selected debug-path
projections, pathing caches/search queues, room sockets, connection buffers, replay playback
cursors/keyframes, replay branch runtime, lab timeline history, match-history write tasks, AI
controller memory, and test fixtures are outside `GameCheckpointV1` unless a later phase promotes
one of them into explicit authoritative DTO state.

Validation model and bounds:

- Parse and byte caps first: reject payload text above 4 MiB before JSON parsing; reject a
  start-state container section above 6 MiB, excluding any replay command-stream attachment that has
  its own cap; reject a sibling map body above 1 MiB until a custom-map phase chooses a larger cap.
- Version and feature checks happen before field validation: `schema == "rts.gameCheckpoint"`,
  `version == 1`, supported `compatibility.simSchemaVersion`, known required features, and a
  compatible RNG algorithm are mandatory.
- Count caps for version 1: at most 8 players, 2,000 entities, 1,024 pending commands, 200,000
  command-log entries when a replay container explicitly allows command history, 256 total active
  plus pending smoke clouds, 4,096 trenches, 512 ability runtime world objects/projectiles, 4,096
  scheduled mortar shells, 4,096 scheduled artillery shells, 4,096 scheduled Panzerfaust shots, 32
  completed upgrades per player, and 8 queued orders per entity.
- Numeric validation rejects non-finite coordinates/facing values, out-of-map world positions,
  invalid tile coordinates, overflowing footprint math, negative or overflowing timers after JSON
  conversion, supply/resources above configured caps unless a documented lab/debug adapter allows
  them, and tick/expiry relationships that would make an active item already impossible.
- Reference validation rejects duplicate player ids, duplicate entity/object/shell/trench ids,
  unknown owners except neutral owner `0`, unknown team references, unknown entity targets, dangling
  command/order/rally/ability references, invalid active-construction ids, invalid lab god-mode
  players, and entity ids at or above the serialized allocator high-water mark.
- Semantic validation rebuilds derived state, recomputes or verifies supply where needed, validates
  faction/kind/upgrade ids against the running rules catalogs, validates queued command unit caps,
  checks command-log ticks are sorted and not in the future of `tick`, and confirms privacy-sensitive
  fields are authoritative state rather than fog-filtered projection output.

Errors returned to artifact-loading surfaces use stable codes and short user-facing messages for
malformed JSON, unsupported version, unsupported required feature, payload/container too large, map
not found, map schema/hash/materialized binding mismatch, count cap exceeded, invalid player/faction
or unit kind, and incompatible RNG algorithm. Developer-only detail, including the exact duplicate
id, dangling reference, field path, or invariant that failed, is logged or exposed only in debug
tooling. The importer must validate all of this before constructing a live `Game`; partially
constructed state is not allowed to escape.

Compatibility policy for version 1 is strict. Same-version readers may add optional compatibility
metadata only when old readers can ignore it without changing authoritative state. Adding,
removing, renaming, or changing the meaning of authoritative fields requires a new checkpoint
version and either an explicit migrator or a stable rejection reason. Existing replay and lab assets
remain on their current schemas until their phases introduce containers around this payload.
Simulation schema 2 adds the authoritative construction-cost payment receipt; schema 1 payloads are
rejected because their unfinished scaffolds cannot be refunded safely. Bundled lab checkpoint assets
use schema 2 and contain no in-progress construction that needs a receipt backfill.

The canonical Hellhole server benchmark is a direct `Game`-API harness, not a live room. Running
`scripts/hellhole-perf-harness.sh` restores `supply-300-hellhole`, issues its deterministic Lab
movement commands through the public Lab command seam, replenishes missing central units through a
single replayable `SpawnEntities` batch before the next tick, calls `tick()`, and produces one
full-world snapshot through the normal compaction and MessagePack encoding path for every tick. It starts no
HTTP listener, WebSocket, or browser and does not pace itself from wall time, so a slow renderer
cannot become its bottleneck. The scenario is one coherent 2v2 match: Players 1 and 3 share a team,
as do Players 2 and 4. The separate checked-in snapshot stream is the client-only lane and records
Player 1's normal fog-filtered body and recipient events, including shared team visibility. A
live Lab server and Pixi client may be run together only through the explicit `--integrated` mode
for visual/end-to-end inspection; that mode is not server-isolation evidence.

The churn driver runs before every tick. Players 1 and 2 are mortal; the driver tracks their
authoritative unit ids and original kinds, preserves spent Panzerfaust kinds, and restores
each owner's canonical 85-unit roster. A replacement that dies during its first tick is recovered by
the owner-count backstop even though the driver never observed its id. Placement considers at most
504 nearest-center candidates, builds body occupancy once per missing request, accounts for earlier
planned spawns, and emits at most one spawn batch. Unresolved requests are reported and retried.
`Game::lab_owned_units` and `Game::lab_plan_unit_spawns` are the typed Lab queries at this seam;
callers do not inspect simulation stores.

Players 3 and 4 remain invulnerable. At every 30-tick boundary the driver statelessly ranks their 85
ids from scenario seed, player, epoch, and id, selects exactly 43, and sends one unqueued move to a
deterministically selected passable integer tile in the active diagonal endpoint corridor. The
endpoint leg still changes every 900 ticks. Request ids, selection, jitter, and replay operations are
deterministic across fresh runs and Lab seek reconstruction.

The former static Hellhole's 8.0x target is not comparable to this churn workload: that fixture sent
two full-army commands in 900 ticks and could not exercise death or path-request admission. Treat the
new counters and serial timings as a measured regression baseline until repeated reference-machine
runs establish a replacement headroom gate. Keep simulation, projection, compaction, encoding, and
driver work on the same serial lane, and do not hide driver cost outside the measured round trip.

### 3.2 Concurrency model
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`). Room registry handles carry per-room identity tokens; registry disposal removes only the matching identity and signals that room task to shut down.
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` for reliable server messages and a dedicated latest-only slot for observer-analysis payloads sent to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `SetSpectator`, `SetFaction`,
  `Command`, `GiveUp`, `PauseGame`, `UnpauseGame`, `SetRoomTimeSpeed`, `StepRoomTime`,
  `SeekRoomTime`, `SeekRoomTimeTo`, `SetVisionSelection`, `Lab`). The room task is the single writer
  of game state — no locks around `Game`. Replay vision selection and successful replay seeks immediately send affected
  viewers fresh fog-scoped snapshots instead of waiting for the next replay tick. Lobby-owned
  connection handling records the command room-enqueue milestone after reserving room-channel
  capacity, so time spent waiting on a saturated room queue remains visible in lifecycle
  diagnostics. When lifecycle samples overflow the bucket table, reported p95 uses the observed
  maximum sample.
- Room-mode, phase, and match-composition dependent lobby checks use a lobby-local `SessionPolicy`
  descriptor for the current room mode and phase matrix, including dev-watch, replay-room,
  branch-staging, speed-only live-game room-time controls, countdown, speed-source, and match-history
  decisions. Authored map minimum and maximum player counts are exposed in the lobby map catalog.
  The selected map's minimum and maximum bound match start eligibility, while its maximum limits
  joins, AI seats, spectator returns, and lobby browser slots. Selecting a lower-capacity map removes overflow AI seats first, then
  moves overflow humans to spectators. Live-match handlers live in `room_task/live.rs`,
  replay-branch handlers live in `room_task/branch.rs`, lab request handling lives in
  `room_task/lab.rs`, dev-watch scenario handling lives in `room_task/dev.rs`, and room lifecycle
  bookkeeping lives in `room_task/lifecycle.rs`; `RoomTask` remains the owner of mutation and tick
  authority. Plain `/lab` is a client-side catalog selector. Its Blank Lab entry launches blank startup on
  the current default `1v1` map, while catalog setups retain their selected maps. Direct lab URLs keep
  compatibility: `scenario=lategame` requests the bundled catalog setup, `scenario=blank` keeps
  blank lab startup, and custom map or seed lab URLs stay blank unless they set an explicit setup. Bundled
  lab setup ids are safe tokens listed in `server/assets/lab-scenarios/manifest.json`; the
  loader in `server/src/lab_scenarios.rs` validates manifest metadata, safe filenames, duplicate
  ids, JSON parseability, map/player-count consistency, and restore compatibility through the
  public lab `Game` API before a setup is exposed or launched. Bundled lab setup startup uses
  the same setup restore path as manual imports and starts with `operation_count=0` plus a tick-0
  timeline keyframe. Lab god mode is lab-only state: `setPlayerGodMode` marks that player's units
  and buildings
  invulnerable, applies across lab mutations, owner changes, spawned assets, and timeline replay
  state, and is mirrored in start/labState metadata. Bundled scenario launch and manual JSON import
  restore exported god-mode player ids through the public lab `Game` API.
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
  `SnapshotFanout`. `ProjectionPolicy` names live player fog and the shared observer views
  (omniscient or an explicit player subset), plus projected movement-path diagnostics and
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
  all-player union observer snapshots by default; they can select one or more real-player
  perspectives through the same connection-scoped observer-view state used by replay, dev, and
  Lab. Normal live observer event unions filter per-player position-free non-alert notices.
  Spectators are not included in `PlayerInit`, command routing, elimination, or match-player counts.
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
  A Map Editor handoff is validated and consumed through the bounded HTTP store before the private
  Lab room is created. Its `LabRoomConfig.map_draft` is applied through `Game::apply_lab_op` during
  launch construction, before the first `start` payload, so the room begins once at tick zero on
  the edited map. The `/map-editor` browser itself does not create a room task or `Game`; that is the
  frozen-session guarantee, and stale/used handoff ids cannot address rooms or files.
  They use the shared launch helper with `StartPayload.lab` metadata and prediction disabled. Lab
  setup mutations call `Game::apply_lab_op`; issue-as commands call `Game::issue_lab_command_as`,
  which rejects mixed-owner selections before queuing a normal command. Lab state, dirty flags,
  viewer roles, per-operator selected vision, the future-join vision default, shared room-time
  speed/pause/controller state, and append-only operation log records stay in the room task rather
  than in `Game`. Bundled catalog and authoring validation restore checkpoint-backed
  `LabCheckpointScenarioV1` containers through the lab `Game` API; legacy lab setup JSON is
  rejected before exposing a setup. Authoring validation exports the authoritative lab `Game` as a
  checkpoint setup and returns a bounded preview without writing files or repository state. Paused lab room-time
  suppresses scheduled ticks; one-tick lab steps and running lab ticks use the same
  `LiveTickDriver` path as ordinary live simulation.
  Bundled scenario drivers may emit a bounded ordered sequence of typed issue-as commands and
  replay-serializable `LabOp` mutations immediately before that shared tick path. The room applies
  each action independently through the same timeline-cap, future-truncation, durable-operation,
  and operation-log rules as accepted operator work; direct benchmark and snapshot-stream tools
  apply the same typed actions through the public `Game` Lab APIs without room or WebSocket state.
  A rejected scripted action logs a warning and does not stop later actions or the room tick.
- Dev scenario watch rooms are a special-case room mode inside the same task model: they own a
  normal `Game`, drive authored scenario setup and optional scripted movement, and use the shared
  projection and fanout helpers to send watchers full-world snapshots for the configured view
  player. Saved self-play artifacts are normal schema 3 `ReplayArtifactV1` files and load through
  `Phase::ReplayViewer` via the neutral replay-artifact room path; schema 2 and older artifacts
  reject in the same loader.
- Replay viewer rooms use `Phase::ReplayViewer`, which owns a lobby-local
  `replay_session::ReplaySession`: the immutable versioned `ReplayArtifactV1`, rebuilt `Game`,
  command cursor, shared playback speed, and per-viewer fog selection. Schema 3 rebuilds the start
  `Game` from `startState.checkpointPayload` and the exact generated map; schema 2 and older
  artifacts reject before playback. Replay snapshots use `game.snapshot_for_spectator(selected_player_ids)`
  so viewers see authoritative union-fog or single-player fog, selected-player resource rows, and
  selected-player remembered building memory, never full-world state.
- Replay and Lab reconstruction are commit-on-success room operations. Ordinary replay seek clones
  the selected keyframe into a candidate `Game`, command cursor, and keyframe list, replays entirely
  against that candidate, and replaces the active session fields only after reaching the requested
  tick. Lab timeline seek retains its existing candidate `Game` and commits seek/cooldown metadata
  only after reconstruction succeeds; Lab replay import prepares one replacement bundle containing
  the duration-tick game, timeline, cleared scenario driver, imported operator/default vision,
  initial camera, clean flag, and cleared operation log before its first active-room assignment.
  `lobby::reconstruction::contain_reconstruction` is the deliberately bounded panic-containment
  seam for these three workflows: it converts ordinary errors and unwind payloads into contextual
  internal failures for room logging and client errors, while discarded candidates leave the prior
  authoritative game and all room/session metadata usable. It is not a general room transaction or
  rollback mechanism; successful reconstruction and unrelated room operations retain their existing
  ownership and commit paths.

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
  state broadcasts, room-time controls, and scenario export/import/validation.
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
  records a baseline keyframe after lab `Game` creation or setup import, records accepted lab world
  mutations and issue-as commands—including bundled scenario actions—in authoritative room order,
  stores periodic cloned `Game` keyframes, rebuilds lab seeks from the nearest retained keyframe,
  and truncates future history
  after a past seek plus a new accepted lab operation or issue-as command. Portable
  `LabReplayArtifactV1` files rebuild this room-local timeline from their initial checkpoint setup
  and durable operation entries after load; the retained keyframes remain a seek cache, not a
  persisted artifact source of truth.
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
  target priority policy ids (`default_weapon`, `vehicle_default_weapon`, `tank_cannon`, and
  `tank_coax_machine_gun`), compatibility helpers such as
  `attack_profile(kind) -> AttackProfile`, and weapon-aware direct damage/miss/facing helpers such
  as `effective_damage_for_weapon(profile, victim_kind, base_dmg, victim_terrain) -> u32`. The
  Tank coax profile is a live secondary Tank weapon (`tank_coax`, 6 tiles, 4 damage, 6-tick
  cooldown, small arms, direct-fire overpenetration). The Panzerfaust loaded-shot target predicate
  for Scout Cars, Tanks, and Command Cars lives here as rules vocabulary while the one-shot state machine stays in
  the sim combat service.
- `rules::target` — pure `TargetFacts` snapshots for target policy consumers. Facts include unit,
  building, resource-node, armor class, weapon class, anti-armor threat, support weapon, field
  obstacle, vehicle-body, economy-unit, and future Tank coax infantry-priority classification.
- `rules::economy` — tech/production predicates (`trainable_units_for_faction`,
  `build_requirement_met_for_faction`, `train_requirement_met_for_faction`,
  `can_research_for_faction`), resource-node amounts, and cost/supply wrappers (`cost`,
  `supply_cost`, `supply_provided`). Legacy non-faction helpers remain as default-faction
  compatibility surfaces for older call sites and tests.
- `rules::terrain` — `TerrainKind` plus movement, cover, concealment, and static line-of-sight
  opacity modifiers. `Open`, bare road, and all four marked road orientations project to passable
  `Road` terrain and share combat/visibility defaults, raw stone blocks LOS, and `Road` supplies
  the authoritative 1.5x movement multiplier sampled from a moving unit's center tile each tick.
  Future forest/hill behavior grows through the same rules seam.
- `rules::projection` — fog-gated `EntityView` construction, legacy/special `visionOnly`
  projection support, and event visibility predicates.

Production buildings may carry an ordered server-authoritative `repeat_units` list. The production
system silently retries the current list entry only while the unit queue is empty, using the same
economy, supply, tech, and producer predicates as direct training. It advances to the next entry
only after a repeated unit is admitted, so two active units produce in stable A/B/A/B order. Once
admitted, a repeated unit is an ordinary FIFO entry, so later manual train commands append behind
it. Enabling a unit appends it to the list if absent, disabling it removes only that unit, and any
production cancel clears the whole list before removing the latest queued item. The list and its
next-unit cursor are durable entity state, so checkpoints, replay branches, and Lab rewinds
preserve them without recording synthetic train commands on every retry.

Explicit manual train and research commands use bounded eight-entry FIFO queues. The front entry
pays immediately when the queue is empty and its cost (plus unit supply) is available; otherwise it
is stored unpaid at zero progress and retries each production tick. Entries appended behind existing
work never prepay. An unpaid unit reserves neither resources nor supply, cancellation and producer
death refund only paid entries, and owner/team projections expose `prodWaiting` so clients do not
extrapolate false progress. Production buildings are visited in stable entity-id order, before
construction, when multiple waiting purchases compete for the same newly available resources.
Standing repeat production deliberately does not use unpaid entries: it continues retrying while the
ordinary queue is empty and inserts a normal paid item only after cost and supply succeed.
Dependent research is admissible when its prerequisite is complete or already earlier in the same
building's FIFO. Owner/team projections expose the ordered `prodUpgradeQueue`, allowing the command
card to enable Heavy Guns behind queued Medium Guns and Artillery Fire Control behind queued Heavy
Guns. The FIFO completion path remains the authority for when each prerequisite actually takes
effect.

### 3.4 Ability system (`game/ability.rs`, `game/services/ability_orders.rs`)

`rules::faction` owns `AbilityKind`, `UpgradeKind`, `AbilityTargetMode`, their stable ids, and the
faction-aware ability/upgrade catalog rows. Each `AbilityCatalogEntry` records its typed kind,
label/icon/hotkey/title, legal carriers, target mode, optional min/max range, cooldown,
optional charges and sequential charge-recharge interval, Steel/Oil cost, tech requirement, queue policy, autocast support, command-card
visibility, and compact protocol/order-stage codes. `game/ability.rs` thinly re-exports the
rules-owned types and converts total typed catalog lookups into the sim-facing `AbilityDefinition`;
it is not a second identity or metadata source. Its planner codes and effect hooks remain
simulation-only details. Raw command, replay/Lab, and checkpoint strings still parse fallibly at
their trust boundaries. Adding a registry-backed ability means adding the rules-owned typed
kind/catalog row and dependency-required protocol wire mirror, extending their parity coverage,
updating the client mirror, and then adding only effect-specific code the registry cannot express.

`AbilityDefinition` also carries a sim-local `AbilityEffectHook` discriminator for the reusable
effect shapes that actually exist today: legacy no-op (`charge` compatibility), reserved no-op
(`blanketFire`), owned area status (`breakthrough`), delayed world effects (`smoke`,
`mortarFire`), Scout Plane dispatch, dash return, line projectile, Magic Anchor placement, Golem
consumption, and the intentionally one-off artillery point-fire path. Panzerfaust damage resolves against the locked live vehicle when it is enemy-owned or owned by the
firing player; allied teammate and neutral targets remain non-damageable. Deliberate same-owner
hits do not emit under-attack notices or receive enemy kill attribution. Impact feedback uses the
stored launch endpoint when a damageable target has moved outside the firing team's current
visibility; enemy victim under-attack notices use the victim's actual position.
Dead loaded Panzerfausts do not advance pending windup state, so they cannot launch before
normal death cleanup. The reserved Blanket Fire hook returns before
command planning, so commands do not spend resources, start cooldowns, or replace artillery
orders. The hook receives the owning player's faction id at execution time through the
normal command/order helpers, so wrong-faction ability use fails before effects, resource spending,
cooldowns, or events are applied. Artillery point fire derives the target direction from the raw click with `atan2`, locks even very
large finite click coordinates to the issuing gun's valid 25-to-55 tile range band, stores that
effective point, and owns any needed in-place setup or redeploy before the first shot. It records temporary live-fog firing reveal sources for enemy
players when a shell launches, using the firing-cycle-plus-half-second lifetime and smoke
suppression used by other actionable firing reveals. Non-targetable Scout Planes are excluded from splash damage, projectile impacts, and Magic Anchor effects.
The hook is deliberately not a generic script engine. Phase 11 signature abilities
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
`snapshot_for_observer`, and `snapshot_full_for` project active world objects through
`Snapshot.abilityObjects`, filtered by the same current-team fog / selected-player / omniscient
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
final snapshot indexing. Riflemen and Machine Gunners owned by a player with
completed Entrenchment research create a neutral trench after holding ground on untrenched terrain
for 90 consecutive simulation ticks. Engineers/Workers are not eligible: they neither dig new
trenches nor occupy existing trenches. Holding ground means the unit has no movement path, no path
movement delta for the tick, no collision displacement after pre-collision derived-state rebuild,
and an order that is effectively stationary: Idle, Hold Position, an in-range Attack order, or an
arrived Attack Move. Firing, target changes, body/weapon facing, and Machine Gunner setup/teardown
do not reset that timer; Move, Attack Move while still travelling, Gather, Build, Deconstruct,
ability movement, artillery point-fire, path movement, and non-slotting forced movement reset it.

Existing trenches are neutral. Any eligible Rifleman or Machine Gunner can occupy an empty
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
`entrenchment_combat::is_actively_entrenched`. Active entrenched Riflemen, Panzerfausts, and Machine Gunners gain
one tile of weapon range through `entrenchment_combat::attack_range_tiles`, including the loaded
Panzerfaust launcher shot. Combat target acquisition treats actively entrenched units, including
units retaining an arrived Attack Move stance, and Machine Gunners that are setting up or deployed
like Hold Position: they can acquire and fire at legal targets inside current weapon range but do
not request enemy-directed paths or tear down the weapon to pursue. A fresh Move, Attack Move, or
direct Attack order clears active occupation before later combat decisions use it; direct Attack
then follows its normal target-pursuit rule and may tear down a Machine Gunner to close range.

Incoming direct-fire accuracy is weapon-specific: Anti-Tank Gun and Tank cannon shots give each
infantry body they intersect an independent 50% chance to dodge, while entrenchment adds no miss
chance. A clump therefore gives one shot multiple independent opportunities to connect even if the
selected target dodges. After a direct hit's normal
weapon, armor, and facing calculations, `entrenchment_combat::reduce_direct_damage` reduces damage
by 50% for actively entrenched eligible infantry. Area effects call
`entrenchment_combat::reduce_area_damage` after their normal falloff and armor calculations, so
Mortar and Artillery splash deal 75% of their current post-formula damage to actively entrenched
eligible infantry. Direct-fire over-penetration stops after hitting
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
Idle/attack-move autocast is conservative and requires completed `mortar_autocast` research plus a
fully deployed Mortar Team. Acquisition and firing are restricted to the full 360-degree field of
fire and the five-to-17-tile range band. Autocast targets each unit's current position without
movement lead, then applies normal deterministic scatter. Before
scheduling a shell, combat checks the deterministic scattered impact point against owned and allied
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

`services::scout_plane` is a mutation helper for the Scout Plane aerial unit. Command admission
calls it from the Command Car `scoutPlane` ability after validating a selected owned Command Car,
player resources, and that Command Car's cooldown. The helper records the
source Command Car, spawns the plane there, flies it to the clicked point, starts a 20-second total
lifetime at launch, and despawns it when that timer expires whether or not it reached the orbit. Scout Plane sorties have no City
Centre dependency or return leg. Live fog recompute stamps Scout Plane sight as team aerial vision that ignores terrain
and building line-of-sight blockers while still using active smoke clouds as blockers.

`game::systems::run_tick` owns the tick pipeline and the lifecycle of tick-scoped derived state.
It rebuilds named phase state at explicit boundaries: pre-command state for command validation,
pathing, and movement; post-movement state for combat and economy queries; pre-collision state
after production/construction/death mutations; collision-displacement snapshots for entrenchment;
and final state for snapshot interest filtering. The three phase occupancy snapshots compare an
exact id/owner/kind/position-bit vector for every building and share immutable blocker/clearance
data within the tick when that topology is unchanged. A construction placement, building removal,
owner change, or relocation rebuilds occupancy at the next boundary; spatial indexes continue to
rebuild at every boundary. This preserves the named phase semantics without repeating the two
full-map clearance-field builds after unit-only movement.
Systems should consume the derived-state object for their phase instead of carrying occupancy or
spatial indexes across later mutations.

### 3.5 Command planning and queued order semantics

Entrenchment research takes 20 seconds.

Deployed anti-tank guns rank in-field automatic target candidates ahead of out-of-arc candidates. If no in-field candidate exists, acquisition retains the out-of-arc target so the weapon clamps toward the fixed field edge without firing.

Automatic acquisition considers only legal enemy candidates inside the attacker's current weapon range, then applies the existing target-priority ranking. Explicit Attack orders may target the issuing player's own units or buildings, but not allied teammate entities, and retain their commanded target while it remains legal and visible. A direct attack pursues that target to the current weapon range band, stops to fire, and repaths if the same target moves out of range; it never switches targets while the commanded target remains valid. Opportunistic moving-fire acquisition for a plain Move uses the same in-range boundary. Attack Move may pause for an in-range engagement and resumes only its original player-issued destination afterward.

Command Cars activate Scout Plane on the C grid slot for 50 Steel and 75 Oil. Activation launches immediately from a selected ready Command Car without a City Centre requirement and starts a 30-second cooldown on that Command Car. Sorties are independent: any number may coexist and each contributes its own team aerial vision. Activation does not replace or clear the selected Command Car's active or queued orders. The plane has a 20-second total lifetime from launch: transit consumes that lifetime, it orbits only for any time remaining after arrival, and it despawns when the timer expires even if it never reaches the target. Scout Planes have no fuel reserve, Oil upkeep, selected-plane retargeting, return leg, or dismissal commands.

Group move formation assignment checks cached reachability components before issuing per-unit goals,
avoiding command-time A* probes outside the move coordinator pathing budget. A blocked or unreachable
compact slot is smudged independently to a nearby standable tile; the planner does not translate the
whole formation around an obstacle. If no reachable local alternative exists, it may preserve a free
local goal so normal path processing can report `PathFailed`.

`FormationMove` accepts a bounded, sanitized world-space polyline and assigns deterministic slots
by arc length. A stroke with enough length uses a single rank across the complete stroke; a shorter
stroke grows parallel ranks at body-aware spacing. Stable entity-id and nearest-slot tie-breaking
reduce crossing. Requested slots then pass through the same standability, uniqueness, known-trench,
and cached-reachability goal search as ordinary formations. Immediate commands replace active
orders as a group. Queued commands resolve the slots at issue time and store one ordinary point
move intent per accepted unit, keeping each unit's bounded queue independent of stroke complexity.

The authoritative command model is: clients compose intent; the server validates and plans it.
Keyboard latching, double-tap quick-cast, Shift lifetime, cursor previews, and rejecting
pointer-captured touch releases outside their originating controls are client UX. The simulation
contract begins when a `SimCommand` reaches `services::commands`: the command service
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
Lab scenario export and restore preserve stable active and queued order intent, including artillery
point-fire and blanket-fire commands. Restore also hydrates the runtime state required for active
movement, build, deconstruct, and artillery point-fire orders to resume execution.
When construction completes, every worker targeting that scaffold clears its active Build order.
`services::order_planner` is the pure
reference implementation of this planning policy. The planner has no `EntityStore`, fog, pathing,
economy, or cooldown mutation dependency; it accepts plain facts and emits one of three effects:

- `ReplaceActive` — replace this unit's active order and clear future queued intents.
- `AppendQueued` — append one future intent to this unit's queue.
- `ExecuteAbilityNow { preserve_orders: true }` — execute an immediate ability without replacing
  the active order or queued intents.

`services::order_execution` is the shared narrow mutation helper for order-state transitions that
are needed by both issue-time command application and queued promotion, such as support-weapon
setup, artillery point-fire targeting, and artillery teardown before movement. Queued artillery
Point Fire remains accepted while a deployed gun is tearing down for an active move, preserving
the locked future target for move-then-fire execution. It should not grow
new validation policy or tick orchestration; those responsibilities remain with command admission,
queued promotion, or the owning tick system.

Live pathfinding and dev scenarios share a 32,768-node expansion budget per path miss. Requests
that exhaust the budget return a best-effort path. Cache entries include the effective search
budget, so an identical bounded request can reuse its deterministic best-effort result while a
larger-budget request still searches afresh. The movement coordinator services at most eight
search-backed tile-path requests per tick; cache hits consume the same scheduling allowance as
misses so clearing the rebuildable cache cannot change simulation timing. Same-tile, zero-search,
and proven-clear direct routes remain free. A cached result retains its original search-work
classification; after any result representing at least 4,096 expanded nodes, the coordinator
preserves that result but defers all later tile-path requests to the next tick. Deferred movement
orders remain
`AwaitingPath`; build and deconstruction orders retain their interaction intent for a later routing
pass. Their serialized execution state advances a staging-candidate cursor after bounded failures
and resets it when the static blocker fingerprint or worker start tile changes, so retry progress
does not depend on rebuildable cache residency. This tick-level scheduling does not lower the
per-route search allowance. Ordinary
non-vehicle move formations bypass A* only when the existing body standability check proves the
exact world-space segment clear; interaction routes and blocked direct segments retain the full
tile-guided search. Direct results are not stored in the tile-keyed cache. `PathingService` reuses
cleared A* working containers between sequential room requests; that scratch state remains derived
and is never serialized. Pump Jack standability
permits a Pump Jack to coexist with its oil node, and simulation invariant checks use that same
Pump Jack policy.

Point move and attack-move commands translate selections into one compact destination layout
regardless of command distance. The layout groups units into broad rows from top to bottom, then
keeps left-to-right ordering within each row; small vertical jitter therefore does not swap nearby
units' horizontal destinations. It does not preserve original world-space separation. Infantry-like
selections use adjacent destination tiles. A selection containing an oriented vehicle body uses a
two-tile pitch, leaving one open tile between destination slots so mixed selections also keep clear
of vehicle bodies. Blocked-slot fallback keeps this vehicle spacing strict. Player-drawn formation
lines remain an explicitly authored layout with their separate line-and-rank assignment policy.

Eligible infantry move and attack-move formation slots bias toward nearby known, unoccupied trench
terrain within a two-tile footprint band around the normal formation goal. A trench occupant counts
as occupied only when visible to the issuing player through the fog and smoke projection used for
snapshots; selected units still free their own occupied trenches for group moves. The visibility check uses
contributors derived from `Game::alive_players()`, matching the living-team visibility boundary used
for snapshots; leftover units owned by defeated teammates do not reveal trench candidates. Hidden
trenches, blocked trench points, occupied trenches outside the command, far trenches, and
non-eligible units use ordinary formation spreading. Trench preference cannot place infantry inside
the one-tile clearance reserved around an already assigned vehicle goal.

Tank weapon range is dynamic in the simulation: tanks keep their base 5-tile range while moving,
then linearly ramp to 14 tiles after three stationary seconds. Path-driven translation or hull
rotation resets the ramp to base range; turret aiming and external pushes do not.

Team-current visibility, including an ally's lingering death vision, permits explicit immediate and
queued attack targeting, but death-vision-only targets remain ineligible for general combat
auto-acquisition. A consumed direct Panzerfaust attack spends the unit's launcher immediately
at launch, allowing queued movement to promote while the detached projectile continues travelling.

Combat weapon cooldowns and firing-reveal reaction gates are independently keyed by
`rules::combat::WeaponKind` inside `CombatState`. A reaction gate is additionally keyed by target
and the stable reveal source/episode, so transient target clears and switching back within one
episode do not restart it. Its absolute readiness deadline includes any reload remaining when the
gate begins, preserving the prior additive timing without mixing reaction time into reload state.
Ordinary sight bypasses the gate immediately while leaving the real reload unchanged; explicit
ordered fire and auto-acquisition use the same team-current ordinary-sight scope as their
target-legality checks. Each weapon retains at most 64 active reaction gates and
deterministically evicts the oldest if that defensive bound is reached. The legacy `Entity::attack_cd()`,
`set_attack_cd()`, and `tick_attack_cd()` shims operate only on an entity kind's default weapon;
new multi-weapon code should use `weapon_cooldown`, `set_weapon_cooldown`,
`tick_weapon_cooldowns`, and `weapon_firing_reveal_reaction_ready`. Ability cooldowns,
lockouts, and uses remain separate from weapon cooldown state. Tanks use this keyed state to keep
the `tank_cannon` and `tank_coax` reloads independent.

Auto-acquisition prefers unit targets before building cleanup targets by default. Building fallback targets still use weapon-fit ranking among eligible cleanup targets.

Overpenetration checks use the target's pre-damage entrenchment state, so lethal primary hits keep the same entrenched blocking decision used before damage resolution.

Entrenchment auto-occupation chooses the nearest trench that has a legal occupation slot for the
unit. A closer trench with no legal slot does not block searching for a farther usable trench. For
dig-in progress, explicit attack orders count as holding ground only after combat advances them to
the `Firing` phase; waiting attack orders do not create trench progress. Lab move
operations clear trench occupation and dig-in state when repositioning an entity so snapshots do not
retain stale `occupiedTrenchId` values before the next tick.

Construction build-site checks classify the current site state before deciding whether work can
start or continue. The status distinguishes invalid terrain, existing buildings or scaffolds,
resource nodes, and relevant unit bodies, while preserving Tank Trap placement rules. Pump Jack
placement is the resource-node exception: it is only valid when the Pump Jack footprint center is
associated with non-depleted oil without another extractor on that same patch, and completed Pump
Jacks, not workers, extract oil. When a Pump Jack builder arrives at an otherwise-valid site,
owned and allied unit bodies overlapping the footprint are deterministically moved to the nearest
static-standable, non-overlapping positions before placement is revalidated; enemy bodies remain
ordinary temporary blockers. Pump Jacks are treated like field infrastructure rather than
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
- Non-queued `HoldPosition` clears each selected unit's active order and queued intents, then marks
  the unit as held. Queued `HoldPosition` appends a terminal intent, so the unit finishes earlier
  queued stages before entering that same held stance. Held units keep normal collision behavior
  and may fire at enemies already inside current weapon range.
- Scout Plane entities are non-combat aerial units. Normal selection and command surfaces filter
  them out; direct move, attack, attack-move, hold-position, gather/build/repair/setup, rally,
  train, and research semantics are ignored for planes.
- Direct attack orders against visible enemies keep the explicit target when a friendly or enemy
  hard blocker would absorb the current shot, but remain stationary and wait rather than seeking a
  fireable position. Shared line-of-sight raycasts stop as reached when a grid-corner side
  step enters the target tile, while preserving opaque-target handling and the two-stone corner
  blocker behavior. Tank Traps are not combat shot blockers, so damage continues to the
  target behind them; tanks and normal buildings still block shots.
  Tank Traps keep generic building targeting and cleanup behavior but do not count for elimination
  survival. Infantry Move steering treats Tank Traps as passable but applies a small local avoidance
  bias when open space exists; vehicles remain hard-blocked by Tank Traps. Attack-move target
  acquisition considers only currently fireable targets inside weapon range. Setup weapons that
  stopped to engage during an unfinished attack-move keep their
  emplacement for a one-second no-target grace period; if the attack-move order still exists after
  that grace, they tear down and continue toward the original attack-move destination.
- Active moving-fire `Move` orders preserve the player-issued destination while they are still in
  `MovePhase::AwaitingPath`, `Moving`, or `PathFailed`. Their auto-acquisition is opportunistic:
  it may retain, aim at, expose, and fire on targets that are currently inside weapon range and
  pass hostile, visibility, smoke, terrain line-of-sight, and blocker checks, but it must not
  request enemy-directed paths or replacements for `path_goal`. `AttackMove` follows the same
  in-range acquisition boundary: once a non-moving-fire unit reaches a fireable enemy, it clears
  its movement path and stops to engage; after combat clears before arrival, it resumes the original
  player-issued destination. Moving-fire units keep advancing along that destination while firing.
  Direct `Attack` and idle behavior are stationary as well.
- Normal combat auto-acquisition first filters already-legal hostile candidates in
  `services::combat::acquisition`, then chooses between them through the sim-local
  `services::combat::priority` ranker. Candidate construction stores a `rules::target::TargetFacts`
  snapshot so ranking consumes explicit facts instead of re-classifying kind-specific fields. The
  ranker selects one named `rules::combat::TargetPriorityPolicyId` and applies that policy's
  declarative terms: default-weapon fit, Tank cannon immediate-threat order, vehicle Tank Trap route
  obstruction, shoot-while-moving target retention, target-group preference, and nearest/id
  tie-breaks. Auto-acquisition only ranks candidates already inside current weapon range. Explicit
  `Attack` orders keep their commanded target while it remains hostile and visible without moving
  toward it. The
  `tank_coax_machine_gun` policy is used by the Tank secondary-fire pass; it ranks Rifleman and
  Machine Gunner targets first, Worker and Golem targets second, and all other legal non-resource
  fallbacks last, then uses distance/id ties without Tank cannon threat ordering. The ranker does
  not decide fog, smoke, line-of-sight, blocker, ownership, or acquisition-radius legality.
  Tanks run the coax pass after their normal cannon aim/fire/relax work for the tick. The pass reads
  the current authoritative turret facing, accepts only targets within a 10-degree half arc and the
  6-tile coax range, uses intended-target direct-fire legality so enemy hard blockers can reject a
  behind-them infantry target, and never rotates the turret, changes the cannon target slot, clears
  paths, or requests chase movement. It skips candidate construction while the coax weapon is on
  cooldown. Same-tick Tank cannon/coax events are emitted cannon first, then coax.
  The first successful enemy Tank cannon, Anti-Tank Gun, or Panzerfaust hit on a surviving Tank
  establishes a three-second hull-facing preference toward its source. Later qualifying hits
  refresh that under-fire window without redirecting the preference, preventing rapid threat
  switching. A stationary Tank turns at the normal hull rate toward the preferred source. A moving
  Tank compares the forward and reverse hull orientations for its current route direction and uses
  whichever keeps its hull closer to the preferred source; a retreat route behind the threat
  therefore begins in reverse even when its destination is far away or reached through intermediate
  waypoints. After the preference expires, ordinary distance-based forward/reverse movement resumes.
  Vehicle traffic sensing follows travel direction rather than hull direction, so reversing Tanks
  yield to traffic behind them. Zero oil and static standability still gate movement. Idle, Hold
  Position, in-range Attack, and arrived Attack Move react without changing their order, path,
  target, or independent turret aim. Static standability may block a rotation but never translate
  the Tank to make room. Stationary preference rotation preserves the stationary range ramp; path
  translation and path-driven hull rotation still reset it.
  Direct-fire legality is centralized in `services::combat::acquisition::direct_fire_target_legal`:
  default auto-acquisition/firing uses the current resolved-target mode that rejects friendly hard
  blockers but may resolve to an intervening enemy hard blocker, while ordered/intended-target uses
  a stricter mode that requires the shot to hit the intended target. Secondary-weapon acquisition
  should use `services::combat::activation::secondary_weapon_target_passes_activation` to pre-filter
  candidates by current weapon facing arc, weapon range, and intended-target direct-fire legality
  without rotating, chasing, or mutating movement state. Default-weapon unit attackers rank
  non-economy combat units first, economy workers (`Worker` and `Golem`) second, and buildings or
  other non-unit cleanup targets last unless explicitly ordered or covered by a special obstruction
  policy. Within a target group, small-arms weapons prefer soft targets while keeping armored
  targets as fallbacks. Default anti-armor weapons prefer anti-armor threats and
  armored units, with Tanks treating in-range Anti-Tank Guns as the top immediate threat.
  Vehicle-body units rank enemy Tank Traps as high-priority breach targets only when
  `services::occupancy` reports that the trap is on the current bounded route segment or forms a
  closed-gap pinch across that route; irrelevant nearby traps remain legal fallback targets but lose
  to real combat targets. The obstruction query is read-only, uses the current waypoint, `path_goal`,
  or movement intent, and does not run pathfinding during target ranking.
  Default-weapon acquisition is cycle-based. A full spatial candidate pass runs when a ready weapon
  has no target, immediately after a shot to prepare the next target during reload, or on the ready
  tick when that prepared target fails current hostile, visibility, smoke, range, line-of-sight,
  blocker, or weapon-specific legality. Cooldown, setup, and aiming ticks validate only the
  committed target and never rank alternatives. A targetless travelling Attack Move is the narrow
  exception: non-Mortar units may scan during reload so they stop when an enemy first enters weapon
  range; after committing that target, the normal no-rerank reload policy applies. If the prepared
  target remains fireable when the weapon becomes ready, it receives the shot without a full
  rerank; the post-shot pass can then choose a newly higher-priority threat for the following cycle.
  Explicit Attack orders retain their commanded-target semantics.
- The auto-acquisition ranker chooses only for the current default attack profile. Future grenades,
  satchels, sticky bombs, melee demolition, or other special attacks must be represented as separate
  profiles with explicit activation policy; explicit-only special attacks can be added without
  changing default auto-acquisition, and autocast special attacks need their own conservative plan
  and tests.
- Panzerfausts research unlocks the separate Barracks-trained Panzerfaust unit. Panzerfausts use a
  hidden server-only one-shot state in combat state: `Loaded -> Windup -> Spent`; Riflemen never
  receive that state, and spent Panzerfausts are not rearmed.
  Direct `Attack`, idle, Hold Position, and Attack Move all fire the launcher only when a valid
  target is already inside the current 5-tile launcher range; none creates pursuit movement. The exact
  target whitelist is Scout Car, Tank, and Command Car; buildings, Mortar Teams, Artillery, and
  infantry are excluded. Targeting is intentionally independent per Panzerfaust with no overkill
  coordination. Windup cancels without spending the shot if the order changes or the target stops
  being legal, visible, in range, or fireable. At launch the Panzerfaust becomes `Spent`, resumes
  normal movement and rifle combat immediately, and records a detached `panzerfaust_shots` impact
  that survives the firing entity's death. Impact applies 100 base damage with 50% armor
  penetration only to the locked live vehicle. Snapshots project `panzerfaustLoaded = true` while
  loaded/winding up and `false` after launch; Riflemen omit the field.
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
  scaffold appears. Economy-created scaffolds persist a paid-cost receipt: canceling one removes the
  site, releases every attached builder's active Build order while preserving queued handoffs, and
  refunds the full building cost without counting a building loss. Lab/authored unpaid scaffolds can
  still be removed through cancellation but never create resources or reverse structure score.
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
  Constructed buildings spawn with their full max HP budget but only 10% current HP, then linearly
  gain current HP with construction progress. Damage taken before completion permanently subtracts
  from that max HP budget; later progress scales against the reduced budget, completion preserves
  the missing HP, and exhausting the budget destroys the scaffold. Scaffold survival is based on
  that remaining budget rather than its temporary progress-scaled current HP, so an early scaffold
  can survive a hit larger than the HP currently shown. Prebuilt starting buildings and damage
  taken after completion keep the normal fixed max HP behavior.
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
  queued Smoke clicks cannot exceed the Scout Car's currently stored charges. Spent Smoke charges
  regenerate sequentially at one charge per 15-second interval, up to the two-charge maximum.
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
  target has passed issue-time validation. Immediate orders prefer interruptible idle workers, then
  workers not already assigned to build/deconstruct work, then interruptible workers already assigned
  to build/deconstruct work; distance to the footprint/target center breaks ties within each tier.
  Workers actively constructing a building are not immediate candidates, but may still receive
  queued handoff work. Stop is the explicit exception: it immediately detaches an active builder,
  leaves the unfinished paid scaffold in place, and clears that worker's queued handoff work. The
  released worker can immediately receive ordinary commands, and any eligible owned worker can
  resume the scaffold through the normal build intent without paying again. Queued orders prefer
  the lowest work assignment load, then closest worker.
  Work assignment load is the worker's current queued-order count plus one when its active order is
  already a build or deconstruct intent. Deconstruct targets must be completed Tank
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
- Support-weapon setup is queueable for selected Anti-Tank Guns, Mortar Teams, and Artillery. Anti-
  Tank Guns and Artillery store a point meaning "face toward this world point from wherever the
  weapon is when the setup stage promotes." Mortar Teams instead retain their current facing and
  treat queued setup as terminal: they finish preceding orders, set up in place, and reject later
  queued stages. Mixed selections ignore non-setup-capable units for setup but keep them for later
  compatible orders.
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
auto-acquisition and firing both use the smoke-aware LOS query; explicit attack orders remain
stationary behind terrain, smoke, or entity blockers and cannot fire until the shot is clear. Direct-fire shot
projection also checks hard entity blockers: tanks and non-Tank-Trap building footprints intercept
shots, while Tank Traps do not. Future forest visibility/cover rules should extend the terrain rules
and this service instead of adding ad hoc checks to fog or combat.

`Game` samples raw live fog per player every second simulation tick (15 Hz). Command validation,
combat targeting, event visibility, building-memory refreshes, and snapshots all consume the same
held sample between refreshes. Normal player snapshots build a temporary team-current fog by
unioning the sampled raw grids of living teammates only.
Hostile unit shots from outside a victim player's current live fog add temporary firing-reveal
sources to live fog for players on the victim's team, not for third-party observers who merely see
the combat event. These sources reveal only the firing unit's current tile, are actionable for
command validation and combat targeting, and expire at
`fired_at_tick + firing_cycle_cooldown + TICK_HZ / 2` so the duration tracks the weapon's firing
cycle plus 0.5 seconds. The fog rebuild records each source's stamped tile and whether that tile was
already visible before any firing reveals were stamped, keeping source provenance attached to the
authoritative fog result rather than inferred later from the flattened grid. This tile-level record
also covers colocated entities and does not follow a source that moves to a different tile during
the next tick. Combatants that first engage a target
through reveal-only sight spend a one-second response delay before their first counter-shot, so
firing-reveal counterfire plays out as shot/counter-shot rather than an instant simultaneous chain.
The actionable tile is removed from snapshot `visibleTiles` and from explored-history accumulation.
The firing unit remains in `entities`; the client recognizes a projected enemy unit whose tile is
presentation-dark and renders it on the explicit above-fog reveal layer. Thus firing reveals expose
the unit for counterfire without clearing or exploring the terrain beneath it.
An active reveal does not delay a combatant that has ordinary sight, and gaining ordinary sight
bypasses an in-progress reaction gate without changing the weapon's reload. Explicit attacks and
autonomous target acquisition both count ordinary allied sight because firing legality and reaction
provenance share the same team-current scope.
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
build-site blocking. `services::occupancy` separates terrain, all-ground static blockers, and
physical vehicle-body-only blockers.
Pump Jacks remain buildings for placement, targeting, economy, and fog memory, but they do not
populate static occupancy layers, so units and vehicles can stand on and path through their
footprints.
Tank Trap pairs exactly two tiles apart close the single tile between them for physical vehicle body
legality while remaining infantry-passable and shot-transparent. Vehicle path planning uses that
same blocker layer for every Tank Trap regardless of owner, team, visibility, or prior scouting, so
vehicles route around enemy walls even when their player has never seen the traps. Path-cache
fingerprints include the vehicle-only blocker layer. Movement, collision, and standability use the
same physical legality.
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
