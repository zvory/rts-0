## 3. Rust server — modules & the Game core API

Crate layout (`server/`):
```
Cargo.toml
src/
  main.rs        # tokio runtime, axum router: static files + /ws, room manager task
  protocol.rs    # serde types for §2  (PINNED — provided)
  config.rs      # all balance/sim constants (PINNED — provided)
  lobby/         # Lobby API plus room task, connection writers, snapshots, dev replay, crash replay
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
    entity/      # Entity, EntityKind, Order state machines, grouped state, and EntityStore
    pathfinding.rs # A* over the tile grid, with optional turn-cost route shaping for tanks
    fog.rs       # per-player live visibility grid plus snapshot-only lingering death sight sources
    systems.rs   # orchestrator: runs services in order each tick
    services/    # per-tick services: commands, move_coordinator, movement (incl. unit collision), combat, economy, production, construction, death, occupancy, supply, pathing, geometry, standability, line_of_sight
    ai.rs        # optional computer opponents: one AiController per AI player (see §8)
    ai_core/     # shared AI observation, facts, actions, profiles, and decisions
    ai_shared.rs # shared AI placement/resource helper functions
    replay.rs    # tick-stamped command log replay harness for determinism checks
    selfplay/    # test-only API-driven scripted self-play harness (see §9)
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
    /// each player's starting City Centre + STARTING_WORKERS workers + nearby steel/oil
    /// resource clusters. For one-, three-, and four-player games, each start keeps its authored
    /// paired expansion. For two-player games, the selected starts are kept but the two active
    /// neutral expansions are reselected from the authored expansion pool as the most symmetric
    /// assignment for that start matchup, so adjacent starts both expand in comparable directions.
    /// AI players use the deterministic default profile; tests and self-play harnesses rely on
    /// this stable behavior.
    pub fn new(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a live lobby match where each AI chooses one strategy from the live profile pool.
    pub fn new_with_random_ai_profiles(players: &[PlayerInit], seed: u32) -> Game;

    /// Create a match with explicit starting steel/oil for every player.
    pub fn new_with_starting_resources(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Create a live lobby match with explicit starting steel/oil and random AI strategies.
    pub fn new_with_starting_resources_and_random_ai_profiles(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Static info for the `start` message (terrain + player start tiles). Call once.
    pub fn start_payload(&self) -> StartPayload;

    /// Queue a validated-on-apply domain command from `player`. Cheap; real work happens in tick().
    pub fn enqueue(&mut self, player: u32, cmd: SimCommand);

    /// Advance the simulation by one tick. Returns per-player transient events.
    pub fn tick(&mut self) -> Vec<(u32 /*player*/, Vec<Event>)>;

    /// Build the fog-filtered snapshot for one player at the current tick.
    pub fn snapshot_for(&self, player: u32) -> Snapshot;

    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot;

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
respect (they get a start position, City Centre, workers, economy, and count toward win/elimination); the
only difference is they have no socket. `Game` owns one `AiController` per AI player and drives
them at the top of `tick()` — see §8.

### 3.2 Concurrency model
- One tokio task per **room** owns its `Game` and runs the tick loop (`tokio::time::interval`).
- Each **connection** is a task with an `mpsc::Sender<ServerMessage>` to push to its socket.
- Connection→room communication uses an `mpsc` channel of internal `RoomEvent`
  (`Join`, `Leave`, `Ready`, `StartRequest`, `AddAi`, `RemoveAi`, `SetSpectator`, `Command`, `GiveUp`). The room task is the
  single writer of game state — no locks around `Game`.
- The room task, each tick: drain commands → `game.tick()` → for each connected player
  `game.snapshot_for(pid)` → send. Lobby phase: broadcast `lobby` on changes.
- Normal rooms reject all mid-match joins. Spectators are lobby members only: they receive
  `StartPayload.spectator = true` and live `game.snapshot_for_spectator(active_player_ids)`
  snapshots, but are not included in `PlayerInit`, command routing, elimination, or match-player counts.
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
- `rules::terrain` — `TerrainKind` plus movement, cover, concealment, and static line-of-sight
  opacity modifiers. It is intentionally small today (`Open` returns current defaults; raw stone
  terrain blocks LOS) so the forest/road/hill feature has one rules file to grow in.
- `rules::projection` — fog-gated `EntityView` construction, `visionOnly` marking for lingering
  death sight, and event visibility predicates.

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

`services::line_of_sight` owns static terrain raycasts used by fog and combat. Stone/rock tiles
block vision and ranged attacks. Fog may reveal the blocking stone tile itself, but not tiles
behind it. Combat auto-acquisition only considers enemies with a clear line; explicit attack
orders may chase toward a terrain-blocked target but cannot fire until the shot is clear. Future
forest visibility/cover rules should extend the terrain rules and this service instead of adding
ad hoc checks to fog or combat.

`services::geometry` owns shared body primitives: infantry unit bodies are circles centered on
`(x, y)` with the configured unit radius, tanks use an oriented vehicle hull derived from their
body `facing`, configured length/width, and a small clearance margin, building bodies are
axis-aligned rectangles derived from footprint tiles, and resource node bodies are circles for
build-site blocking. `services::standability` owns reusable legality predicates for unit bodies and
building sites. Production spawn exits, construction/build intent, movement landing, steering
candidates, collision push targets, and formation goal selection all use this shared standability
layer for static/body legality. Swept segment checks sample the same body shape along a straight
segment, and broad-phase queries use each body's conservative bounding radius. Movement separates
oriented vehicle body legality from drive behavior: tanks and AT guns use pivot-drive locomotion
that can rotate in place before advancing, while scout cars use car-drive path following where hull
facing changes through translation/curvature. These helpers are pure and do not change the wire
protocol or client contract.

---
