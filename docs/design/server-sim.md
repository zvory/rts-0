## 3. Rust server — modules & the Game core API

Crate layout (`server/`):
```
Cargo.toml
src/
  main.rs        # tokio runtime, axum router: static files + /ws, room manager task
  protocol.rs    # server-shell protocol adapter shim; serde DTOs live in crates/protocol
  config.rs      # server-shell balance shim; authoritative values live in crates/rules
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
    services/    # per-tick services: commands, order_planner, move_coordinator, movement (incl. unit collision), combat, economy, production, construction/deconstruction, death, occupancy, supply, pathing, geometry, standability, line_of_sight
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
    /// expansion per player.
    /// AI players are spawned as normal match participants; external AI orchestration owns any
    /// controller/profile selection.
    pub fn new(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a live lobby match where each AI chooses one strategy from the live profile pool.
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game;

    /// Compatibility helper for tests and debug starts that still need explicit starting
    /// Steel/Oil. Production replay/lifecycle reconstruction should use per-player
    /// `PlayerStartingLoadout` records instead.
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

    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot;

    /// Same projection as `snapshot_for_spectator`, with explicit room-projection diagnostic options.
    pub fn snapshot_for_spectator_with_options(&self, visible_players: &[u32], options: SnapshotOptions) -> Snapshot;

    /// Build a full-world snapshot for a dev watch client. Normal gameplay must not use this.
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

    /// Authoritative observer analysis state for replay viewers and live spectators.
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
player state, entity records, and small lab metadata such as scenario name and exported tick;
room-owned protocol export adds current lab vision metadata before sending JSON to the browser.
Restore loads the named map, validates faction/research/kind data, recreates entities with fresh
ids, repairs derived state, and returns the id remap for callers that need to reconcile UI
selection. Snapshot-only projections, transient events, projectile runtime state, and command logs
are not part of the scenario format.

`PlayerInit.team_id` is canonical team identity. Phase 1 preserves FFA gameplay by assigning each
seated player a unique nonzero team by default; deserialized or hand-built fixtures with
`team_id == 0` are normalized to `team_id = id` when constructing a `Game`. Relationship helpers
on `Game` are available for future team-aware systems: `team_of_player`, `same_team_player`,
`same_team_owner`, `is_enemy_player`, `is_enemy_owner`, and `allied_player_ids`. Neutral owner `0`
is never allied with a player.

`PlayerInit.faction_id` is canonical faction identity. The default current faction is
`kriegsia`, and the server/lobby layer validates requested or recorded faction ids before match
assembly. That policy is separate from `rules::faction` catalog existence: normal lobby,
quickstart, AI, self-play, and dev starts default missing requests to Kriegsia, explicit
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
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`).
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` to push to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `SetSpectator`, `Command`,
  `GiveUp`, `PauseGame`, `UnpauseGame`, `SetRoomTimeSpeed`, `StepRoomTime`, `SeekRoomTime`,
  `SeekRoomTimeTo`, `SetReplayVision`, `Lab`). The room task is the single writer of game state —
  no locks around `Game`.
- The room task, each tick: enqueue live AI commands for AI players → `game.tick()` → for each
  connected player `game.snapshot_for(pid)` → send. Lobby phase: broadcast `lobby` on changes.
- Live-match pause state belongs to `RoomTask`, not `Game` and not `tick_control.rs`. Normal live
  and branch-live active seats can spend up to three successful pause starts per match; spectators,
  replay viewers, dev-watch viewers, and lab viewers cannot spend pauses. While paused, the room
  event loop continues handling reliable control messages, Give up, disconnects, and unpause, but
  the live scheduled tick returns before constructing `LiveTickDriver`, so AI thinking,
  command-ack consumption, `Game::tick`, snapshot fanout, and defeat checks do not advance.
  `prepare_live_match_launch`, live-match teardown/replay transition, and empty-room reset all
  clear pause counters and paused state.
- Normal rooms reject all mid-match joins. Spectators are lobby members only: they receive
  `StartPayload.spectator = true` and live `game.snapshot_for_spectator(active_player_ids)`
  snapshots, but are not included in `PlayerInit`, command routing, elimination, or match-player counts.
- Lab rooms are hidden `RoomMode::Lab` rooms that start a real `Game` on first join with a
  room-owned operator/read-only viewer session record. They use the shared launch helper with
  `StartPayload.lab` metadata and prediction disabled. Lab setup mutations call `Game::apply_lab_op`;
  issue-as commands call `Game::issue_lab_command_as`, which rejects mixed-owner selections before
  queuing a normal command. Lab state, dirty flags, viewer roles, selected vision, and append-only
  operation log records stay in the room task rather than in `Game`.
- Dev scenario watch rooms are a special-case room mode inside the same task model: they own a
  normal `Game`, drive authored scenario setup and optional scripted movement, and use the shared
  projection and fanout helpers to send watchers full-world snapshots for the configured view
  player. Saved self-play artifacts are normal `ReplayArtifactV1` files and load through
  `Phase::ReplayViewer` via the neutral replay-artifact room path.
- Replay viewer rooms use `Phase::ReplayViewer`, which owns a `ReplaySession`:
  the immutable `ReplayArtifactV1`, rebuilt `Game`, command cursor, shared playback speed, and
  per-viewer fog selection. Replay snapshots use `game.snapshot_for_spectator(selected_player_ids)`
  so viewers see authoritative union-fog or single-player fog, never full-world state.

Lobby-owned runtime boundaries stay in `server/src/lobby/`; none of these helpers move transport,
AI controllers, or Tokio coordination into `rts-sim`:

- `room_task.rs` remains the room lifecycle owner: membership, lobby/ingame/replay/branch phase
  transitions, start/end/reset/drain bookkeeping, match-history dispatch, and the single owned
  `Game`.
- `session_policy.rs` is the explicit internal descriptor for the current room mode and phase. It
  names the state source, join, clock, authority, mutation, visibility, diagnostics,
  persistence/export, start-payload, and UI-affordance choices used by the rest of the lobby
  helpers. Persistence is split into match-history eligibility, transient post-match replay
  capture, match-history replay-artifact attachment, and room-local lab operation logging. Product
  identity still selects real setup paths such as replay-artifact loading, dev scenario
  construction, replay-branch seeding, and lab room initialization; lower-level helpers should
  consume the explicit policy fields when the behavior is shared.
- `participants.rs` is the connected-user and active-seat helper. It owns host fallback, active
  human and AI seat lists, spectator visible-seat lists, branch-live connection-to-original-seat
  aliases, and command issuer resolution.
- `tick_control.rs` maps the session clock policy, replay pause/speed, dev-watch pause state, and
  countdown state to the room ticker interval and scheduled action. `RoomTask` still owns the Tokio
  interval and remains the only task that advances a room.
- `projection.rs` owns snapshot projection and observer-analysis decisions for client fanout. Live
  active players get player fog, live spectators get active-seat union fog, replay viewers get their
  per-viewer replay vision, branch-live active players use original-seat aliases, and dev-watch
  viewers get full-world scenario snapshots.
- `launch.rs` owns common `StartPayload` stamping for live, replay-branch-live, and dev-watch
  starts: player id, spectator flag, prediction build/version, recipient capability metadata,
  pending snapshot clearing, and the send loop. Replay viewer payloads remain in
  `replay_session.rs` because they also carry replay metadata.
- `live_tick.rs` runs one live simulation tick around the existing `Game` seam: AI command enqueue,
  `Game::tick`, snapshot fanout, observer analysis, defeat/game-over checks, and panic replay
  capture.
- `replay_session.rs` owns replay playback state, seek/keyframe policy, per-viewer replay vision,
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
keeps accepted lab mutation and issue-as calls centralized in `room_task.rs`, where operator
authorization, result routing, dirty state, and the append-only operation log live.

### 3.3 Rules layer (`rules/`)

`server/crates/rules/src/` contains classification, formula, terrain, and economy functions with
no simulation state dependency. `server/crates/sim/src/rules/projection.rs` is the explicit
state-reading exception: it reads `Entity`, `Fog`, and smoke state so snapshot and event visibility
policy is centralized instead of scattered through services.

- `rules::defs` — immutable unit/building/node definition tables keyed by `EntityKind`. These
  records are the source of truth for kind-specific stats, armor class, weapon class, target
  priority, production chains, tech requirements, and resource-node amounts.
- `rules::faction` — faction catalogs keyed by stable faction id. Catalogs reference global
  `EntityKind`, upgrade id, ability id, and Steel/Oil/Supply costs; reuse a global id across
  factions only when gameplay semantics are identical for every faction that can use it. Divergent
  behavior, stats, production role, or ability meaning requires a distinct global id gated through
  catalog availability. The default catalog is `kriegsia`; `ekat` exposes the current Ekat hero
  and Zamok slice; `phase2_empty_fixture` exists only as a command-validation test fixture.
  Server-side lifecycle policy lives in `server/src/lobby/faction_validation.rs`.
- `rules::combat` — AP/armor predicates (`is_ap`, `is_armored`, `prefers_armored_targets`),
  `attack_profile(kind) -> AttackProfile`, and
  `effective_damage(attacker_kind, victim_kind, base_dmg, victim_terrain) -> u32`.
- `rules::economy` — tech/production predicates (`trainable_units_for_faction`,
  `build_requirement_met_for_faction`, `train_requirement_met_for_faction`,
  `can_research_for_faction`), resource-node amounts, and cost/supply wrappers (`cost`,
  `supply_cost`, `supply_provided`). Legacy non-faction helpers remain as default-faction
  compatibility surfaces for older call sites and tests.
- `rules::terrain` — `TerrainKind` plus movement, cover, concealment, and static line-of-sight
  opacity modifiers. It is intentionally small today (`Open` returns current defaults; raw stone
  terrain blocks LOS) so the forest/road/hill feature has one rules file to grow in.
- `rules::projection` — fog-gated `EntityView` construction, `visionOnly` marking for lingering
  death sight, and event visibility predicates.

### 3.4 Ability system (`game/ability.rs`, `game/services/ability_orders.rs`)

`rules::faction` owns the faction-aware ability registry. Each `AbilityCatalogEntry` records the
stable id, label/icon/hotkey/title, legal carriers, target mode, optional min/max range, cooldown,
finite charges, Steel/Oil cost, tech requirement, queue behavior, autocast support, command-card
visibility, and compact protocol/order-stage codes. `game/ability.rs` keeps the typed
`AbilityKind` and converts those registry rows into the sim-facing `AbilityDefinition`; it is not a
second source of metadata. Adding a registry-backed ability means adding a global `AbilityKind` and
protocol id, adding the faction catalog entry, updating the client mirror/parity check, and then
adding only the effect-specific code that the registry cannot express.

`AbilityDefinition` also carries a sim-local `AbilityEffectHook` discriminator for the reusable
effect shapes that actually exist today: self status (`charge` legacy compatibility), owned area
status (`breakthrough`), delayed world effects (`smoke`, `mortarFire`), dash return, line
projectile, Magic Anchor placement, and the intentionally one-off artillery point-fire path. The
hook receives the owning player's faction id at execution time through the normal command/order
helpers, so wrong-faction ability use fails before effects, resource spending, cooldowns, or events
are applied. The hook is deliberately not a generic script engine. Phase 11 signature abilities
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
  shell). Guards:
  caster exists + alive + owner + not under construction + correct kind + not on cooldown +
  required tech present + in range + affordable.
  All guards are checked without panicking; missing/stale casters are no-ops.
- `launch_self_ability` — validates the self-targeted registry row and dispatches self-status or
  owned-area-status hooks. Breakthrough remains an owned-unit area buff; legacy Charge remains
  decodable but has no current carriers.
- `caster_can_attempt`, `tech_requirement_met`, `caster_in_range` — pure predicates used by both
  command validation and order-queue promotion.

Active `Order::Ability` movement orders run through `services::order_queue::promote_ready_orders`:
when the caster arrives (phase `Arrived`), `launch_world_ability` is called; when pathing fails
(phase `PathFailed`), the order is cleared silently. Stale queued ability intents (caster dead,
cooldown active, tech gone, target point off-map) are skipped at promotion time via
`ability_intent_valid`.

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

Mortar shells are delayed AOE effects resolved by `game::mortar` after their flight timer expires.
They damage owned, allied, and enemy units/buildings with the same falloff and armor rules; resource
nodes are ignored. Same-team mortar damage is intentionally real friendly fire, but it is
unattributed: it does not update `last_damage_owner`/position/tick, does not trigger AI worker
retreat, does not emit enemy under-attack notices, and does not award kill credit or combat score.
Idle/attack-move autocast is conservative and requires completed `mortar_autocast` research: before
scheduling a shell, combat checks the predicted impact point against owned and allied units/buildings
at their current positions and holds fire if any would be inside the damaging radius. Manual mortar
fire is intentionally allowed onto same-team positions, so players can still take risky shots
deliberately. Mortar autocast is stored on the authoritative combat state, is enabled for current
and future Mortar Teams when research completes, and can be disabled through
`SetAutocast(mortarFire, enabled=false)`; disabled mortars still accept manual `mortarFire` commands.

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
after production/construction/death mutations; and final state for snapshot interest filtering.
Systems should consume the derived-state object for their phase instead of carrying occupancy or
spatial indexes across later mutations.

### 3.5 Command planning and queued order semantics

The authoritative command model is: clients compose intent; the server validates and plans it.
Keyboard latching, double-tap quick-cast, Shift lifetime, and cursor previews are client UX. The
simulation contract begins when a `SimCommand` reaches `services::commands`: the command service
dedupes and caps unit-id lists, rejects over-budget human unit-list commands, builds issue-time
facts for the referenced units/targets, and must produce unit-local actions that match the policy
below. Human command budget is supply-based: 24 base command supply plus 12 per submitted owned
Command Car plus that Command Car's own mirrored supply weight, so Command Cars offset their own
weight before adding bonus capacity. AI-owned players are exempt from this budget because live AI
still issues ordinary `SimCommand`s through
`Game::enqueue`. `services::order_planner` is the pure
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

General rules:

- Commands must be valid at issue time. The server checks ownership, unit capability, target
  validity/visibility, finite points, ability carrier kind, ability readiness/cooldown/uses, and
  other command-specific facts before planning. It does not project future movement, future
  cooldown expiry, future tech, or future affordability.
- Resource costs are paid at execution time, not queue time. A queued ability or build that becomes
  unaffordable later is skipped or rejected by the execution/promotion path.
- Omitted `queued` means immediate. Ordinary immediate unit orders replace active state and clear
  future intents. `stop` always clears both active and queued unit orders.
- Queueable commands append future unit-local intents. Unit queues are capped at 8 intents today;
  a valid append rejected only because the queue is full should emit a player notice.
- Invalid commands are no-ops except where a notice is explicitly useful. Stale queued stages are
  skipped at promotion time rather than retried forever.
- Queue planning is issue-time only. A unit with an ability on cooldown is not eligible for a
  queued ability intent just because the cooldown might expire before the intent promotes.
- Later orders still apply to every compatible selected unit. Earlier specialized stages do not
  remove non-carriers from the plan; for example, a queued smoke applies to one scout car, while the
  following queued attack-move applies to all selected units that can receive attack-move.

Allocation rules:

- Point orders (`move`, `attackMove`) apply to every selected owned unit that can receive orders.
- Target/resource orders apply to every selected compatible owned unit after the target has passed
  issue-time validation. Occupied resource nodes are still valid gather targets; when a worker
  arrives and the patch is already occupied, the economy service redirects it to the nearest
  unoccupied same-resource node within ten tiles, or moves it to nearby open grass if none exists.
  Build and Tank Trap deconstruct orders allocate one compatible selected worker per click after the
  target has passed issue-time validation: immediate orders prefer idle workers and then closest
  worker to the footprint/target center; queued orders prefer the lowest work assignment load, then
  closest worker. Work assignment load is the worker's current queued-order count plus one when its
  active order is already a build or deconstruct intent. Deconstruct targets must be completed Tank
  Traps; friendly/allied traps are always legal targets for their team's workers, while enemy traps
  must be visible when accepted or promoted. Deconstruction takes the Tank Trap's build time, is not
  accelerated by assigning multiple workers to the same trap, and refunds the Tank Trap cost to the
  deconstructing player.
- Legacy Charge has no eligible carriers after the Methamphetamines research conversion. It remains
  decodable for old command logs but does not create queued or immediate ability work.
- World-targeted abilities, such as Smoke, allocate one ready carrier per click. For queued
  commands the planner chooses an eligible selected carrier with the shortest current queue, which
  gives round-robin behavior across repeated clicks. If all eligible carriers are full, emit queue
  full notices; if no carrier is ready at issue time, ignore the click.
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
owns terrain raycasts used by fog and combat and can be constructed with the active smoke store as
a dynamic blocker input. Stone/rock tiles block vision and ranged attacks. Fog may reveal the
blocking stone tile itself and the visible edge of a smoke cloud, but not tiles behind blockers.
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
Snapshot-only lingering death sight is layered after live fog and then unioned for projection, so
lingering views remain non-actionable (`visionOnly`) and cannot validate commands or refresh
remembered buildings. Neutral resource nodes never stamp vision.

`game::building_memory::BuildingMemory` is server-only stale intel owned by `Game`. After live,
smoke-aware fog is recomputed, the store records one latest-seen entry per
`(viewer_player_id, enemy_building_entity_id)` for non-neutral enemy buildings currently
projectable to that viewer through team-current actionable fog. Records copy id, owner, kind,
center position, footprint tiles, hp/max hp, construction progress/completion state, and the tick
observed. Snapshot-only lingering death vision is intentionally not used for refreshes, so it
cannot create actionable intel for future commands. If a remembered building no longer exists, the
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
