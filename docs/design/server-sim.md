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
    services/    # per-tick services: commands, order_planner, move_coordinator, movement (incl. unit collision), combat, economy, production, construction, death, occupancy, supply, pathing, geometry, standability, line_of_sight
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

    /// Create a match with explicit starting steel/oil for every player.
    pub fn new_with_starting_resources(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Create a live lobby match with explicit starting steel/oil and random AI strategies.
    pub fn new_with_starting_resources_and_random_ai_profiles(players: &[PlayerInit], steel: u32, oil: u32, seed: u32) -> Game;

    /// Rebuild replay playback with explicit resources, loadout, map, and map metadata.
    /// Commands are injected by the replay runtime rather than live AI controllers.
    pub fn new_for_replay_with_map_metadata(
        players: &[PlayerInit],
        steel: u32,
        oil: u32,
        seed: u32,
        starting_loadout: ReplayStartingLoadoutMode,
        map: Map,
        map_metadata: MapMetadata,
    ) -> Game;

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

    /// Build a spectator snapshot from the union of all active players' current fog.
    pub fn snapshot_for_spectator(&self, visible_players: &[u32]) -> Snapshot;

    /// Build a full-world snapshot for a dev watch client. Normal gameplay must not use this.
    pub fn snapshot_full_for(&self, player: u32) -> Snapshot;

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

    /// Authoritative replay-only analysis state for spectator overlays.
    pub fn replay_analysis(&self) -> ReplayAnalysisPayload;

    /// Remove all of a player's entities (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32);

    pub fn tick_count(&self) -> u32;

    /// Authoritative commands applied so far, stamped with the tick that applied them.
    pub fn command_log(&self) -> &[CommandLogEntry];

    /// Reconstruct the player specs used to create this match for replay/crash artifacts.
    pub fn player_inits(&self) -> Vec<PlayerInit>;
}

pub type TeamId = u32;
pub struct PlayerInit { pub id: u32, pub team_id: TeamId, pub name: String, pub color: String, pub is_ai: bool }
pub struct CommandLogEntry { pub tick: u32, pub player_id: u32, pub command: Command }
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

`PlayerInit.team_id` is canonical team identity. Phase 1 preserves FFA gameplay by assigning each
seated player a unique nonzero team by default; deserialized or hand-built fixtures with
`team_id == 0` are normalized to `team_id = id` when constructing a `Game`. Relationship helpers
on `Game` are available for future team-aware systems: `team_of_player`, `same_team_player`,
`same_team_owner`, `is_enemy_player`, `is_enemy_owner`, and `allied_player_ids`. Neutral owner `0`
is never allied with a player.

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
  `GiveUp`, `SetReplaySpeed`, `SeekReplay`, `SetReplayVision`). The room task is the single writer
  of game state — no locks around `Game`.
- The room task, each tick: enqueue live AI commands for AI players → `game.tick()` → for each
  connected player `game.snapshot_for(pid)` → send. Lobby phase: broadcast `lobby` on changes.
- Normal rooms reject all mid-match joins. Spectators are lobby members only: they receive
  `StartPayload.spectator = true` and live `game.snapshot_for_spectator(active_player_ids)`
  snapshots, but are not included in `PlayerInit`, command routing, elimination, or match-player counts.
- Live dev self-play watch rooms are a special-case room mode inside the same task model: they own
  a normal `Game`, feed it scripted commands from `rts_ai::selfplay`, and send watchers
  `game.snapshot_full_for(view_pid)` instead of fog-filtered snapshots. Saved self-play artifacts
  are normal `ReplayArtifactV1` files and load through `Phase::ReplayViewer`.
- Replay viewer rooms use `Phase::ReplayViewer`, which owns a `ReplaySession`:
  the immutable `ReplayArtifactV1`, rebuilt `Game`, command cursor, shared playback speed, and
  per-viewer fog selection. Replay snapshots use `game.snapshot_for_spectator(selected_player_ids)`
  so viewers see authoritative union-fog or single-player fog, never full-world state.

### 3.3 Rules layer (`rules/`)

`server/crates/rules/src/` contains classification, formula, terrain, and economy functions with
no simulation state dependency. `server/crates/sim/src/rules/projection.rs` is the explicit
state-reading exception: it reads `Entity`, `Fog`, and smoke state so snapshot and event visibility
policy is centralized instead of scattered through services.

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

### 3.4 Ability system (`game/ability.rs`, `game/services/ability_orders.rs`)

`game/ability.rs` defines `AbilityKind` (legacy `Charge` plus active `Smoke`), `AbilityDefinition`,
`AbilityTargetMode` (`SelfTarget` or `WorldPoint`), `ResourceCost`, and the compile-time
definition table accessed via `ability::definition(kind)`. Ability definitions include the carrier
entity kinds, target mode, optional tile range, cooldown in ticks, resource cost, tech requirement,
and whether the ability may be queued. Adding a new ability means adding an `AbilityKind` variant
and a `definition` match arm; no other files need to change for the definition itself.

`services::ability_orders` owns the tick-path execution helpers:
- `order_or_launch_world_ability` — for `WorldPoint` abilities: if the caster is in range, launch
  immediately; otherwise compute a staging point inside range and issue an `Order::Ability`
  movement order via `MoveCoordinator`.
- `launch_world_ability` — deducts resources, sets the caster's cooldown, clears the active order,
  and executes the effect (currently: schedules a smoke cloud or delayed mortar shell). Guards:
  caster exists + alive + owner + not under construction + correct kind + not on cooldown +
  required tech present + in range + affordable.
  All guards are checked without panicking; missing/stale casters are no-ops.
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
service-to-service imports. The roles are intentionally coarse:

- tick systems are the phases called by `systems.rs`;
- command adapters translate validated command facts into mutations;
- pure policy modules accept facts and return decisions without mutable world state;
- query/index services read derived or immutable world state;
- mutation helpers perform narrow execution work below a caller-owned phase.

Every service import must be present in the exact import allowlist and must also be legal under the
role matrix. A role failure explains why the edge is forbidden, usually because orchestration should
stay in `systems.rs` or because command-family growth should use command input -> issue-time facts
-> pure plan -> narrow executor. `services::commands` and `services::order_queue` are
grandfathered broad command adapters: their current allowed imports pass so cleanup can proceed
gradually, but new service modules must be classified and new edges still need explicit review.

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
dedupes and caps unit-id lists, builds issue-time facts for the referenced units/targets, and must
produce unit-local actions that match the policy below. `services::order_planner` is the pure
reference implementation of this planning policy. The planner has no `EntityStore`, fog, pathing,
economy, or cooldown mutation dependency; it accepts plain facts and emits one of three effects:

- `ReplaceActive` — replace this unit's active order and clear future queued intents.
- `AppendQueued` — append one future intent to this unit's queue.
- `ExecuteAbilityNow { preserve_orders: true }` — execute an immediate ability without replacing
  the active order or queued intents.

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
  issue-time validation. Build orders allocate one compatible selected worker per click after the
  placement has passed issue-time validation: immediate builds prefer idle workers and then closest
  worker to the footprint center; queued builds prefer the lowest build assignment load, then closest
  worker. Build assignment load is the worker's current queued-order count plus one when its active
  order is already a build intent.
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
toward terrain- or smoke-blocked targets but cannot fire until the shot is clear. Future forest
visibility/cover rules should extend the terrain rules and this service instead of adding ad hoc
checks to fog or combat.

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
build-site blocking. `services::standability` owns reusable legality predicates for unit bodies and
building sites. Production spawn exits, construction/build intent, movement landing, steering
candidates, collision push targets, and formation goal selection all use this shared standability
layer for static/body legality. Swept segment checks sample the same body shape along a straight
segment, and broad-phase queries use each body's conservative bounding radius. Movement separates
oriented vehicle body legality from drive behavior: tanks and anti-tank guns use pivot-drive locomotion
that can rotate in place before advancing, while scout cars use car-drive path following where hull
facing changes through translation/curvature. These helpers are pure and do not change the wire
protocol or client contract.

---
