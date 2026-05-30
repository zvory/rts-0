//! The authoritative game simulation. See `DESIGN.md` §3.1 for the public API contract.
//!
//! [`Game`] is the single seam between the simulation and the networking/lobby layer. The
//! networking layer calls ONLY the methods in §3.1; everything else here is private detail.
//!
//! The simulation is fixed-rate: each [`Game::tick`] drains queued commands, advances every
//! system in a deterministic order, and recomputes per-player fog. Snapshots are pulled
//! separately via [`Game::snapshot_for`], fog-filtered so a player only ever sees neutral /
//! enemy entities on tiles they currently see.

pub mod ai;
pub mod entity;
pub mod fog;
mod invariants;
pub mod map;
pub mod pathfinding;
pub mod replay;
#[cfg(test)]
mod selfplay;
pub(crate) mod services;
pub mod systems;

use std::collections::HashMap;

use crate::config;
use crate::protocol::{Command, EntityView, Event, MapInfo, PlayerStart, Snapshot, StartPayload};
use serde::{Deserialize, Serialize};

use ai::AiController;
use entity::{Entity, EntityKind, EntityStore, Order};
use fog::Fog;
use map::Map;
use replay::CommandLogEntry;

/// Lobby-supplied identity for a player joining a match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerInit {
    pub id: u32,
    pub name: String,
    pub color: String,
    /// When true this player is a computer opponent: it has no socket and is driven by an
    /// internal [`AiController`] instead of receiving snapshots / sending commands.
    pub is_ai: bool,
}

/// Per-player economy and bookkeeping carried for the whole match. Visible to `systems` (the
/// only other module that mutates economy), but not part of the public API.
pub(crate) struct PlayerState {
    pub(crate) id: u32,
    pub(crate) name: String,
    pub(crate) color: String,
    pub(crate) start_tile: (u32, u32),
    pub(crate) steel: u32,
    pub(crate) oil: u32,
    /// Supply currently consumed by living + in-production units.
    pub(crate) supply_used: u32,
    /// Supply provided by completed Industrial Centers/Depots, capped at `SUPPLY_CAP_MAX`.
    pub(crate) supply_cap: u32,
}

/// The authoritative match state.
pub struct Game {
    map: Map,
    entities: EntityStore,
    fog: Fog,
    players: Vec<PlayerState>,
    /// One controller per AI-owned player. Driven at the top of [`tick`] to enqueue commands;
    /// empty for an all-human match.
    ai: Vec<AiController>,
    /// Commands received this tick window, drained at the start of [`tick`]. Each carries the
    /// issuing player so ownership can be validated on apply.
    pending: Vec<(u32, Command)>,
    /// Authoritative commands stamped with the tick where they were applied. Includes AI commands
    /// because they are emitted into the same pending queue before command application.
    command_log: Vec<CommandLogEntry>,
    tick: u32,
    /// Post-tick spatial index, rebuilt every tick after all systems run so [`snapshot_for`]
    /// can use it for interest filtering without rebuilding.
    spatial: services::spatial::SpatialIndex,
    /// Persistent pathfinding service with an LRU cache for verified paths.
    pathing: services::pathing::PathingService,
}

impl Game {
    /// Create a match for the given players. Generates a symmetric map sized for the player
    /// count and spawns each player's starting Industrial Center, workers, and a nearby resource cluster.
    pub fn new(players: &[PlayerInit]) -> Game {
        Self::new_inner(players, true)
    }

    #[cfg(test)]
    pub(crate) fn new_for_replay(players: &[PlayerInit]) -> Game {
        Self::new_inner(players, false)
    }

    fn new_inner(players: &[PlayerInit], enable_ai: bool) -> Game {
        // Deterministic seed derived from the player set so a given lobby produces a stable
        // map (helps reproducibility / debugging) without any external RNG.
        let mut seed: u32 = 0x1234_5678 ^ (players.len() as u32).wrapping_mul(2_654_435_761);
        for p in players {
            seed ^= p.id.wrapping_mul(0x9E37_79B1).rotate_left(7);
        }

        let map = Map::generate(players.len(), seed);
        let fog = Fog::new(map.size);
        let mut entities = EntityStore::new();

        let mut player_states = Vec::with_capacity(players.len());
        let mut ai = Vec::new();
        for (i, p) in players.iter().enumerate() {
            let start = map.starts.get(i).copied().unwrap_or((0, 0));
            if enable_ai && p.is_ai {
                ai.push(AiController::new(p.id));
            }
            let mut ps = PlayerState {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile: start,
                steel: config::STARTING_STEEL,
                oil: config::STARTING_OIL,
                supply_used: 0,
                supply_cap: 0,
            };
            spawn_player_start(&mut entities, &map, p.id, start);
            // The starting Industrial Center contributes supply immediately.
            ps.supply_cap = config::INDUSTRIAL_CENTER_SUPPLY.min(config::SUPPLY_CAP_MAX);
            player_states.push(ps);
        }

        let spatial = services::spatial::SpatialIndex::build(&entities, map.size);
        let pathing = services::pathing::PathingService::new(8_192, 256);
        let mut game = Game {
            map,
            entities,
            fog,
            players: player_states,
            ai,
            pending: Vec::new(),
            command_log: Vec::new(),
            tick: 0,
            spatial,
            pathing,
        };
        // Initialize supply accounting and fog so the very first snapshot is correct.
        systems::recompute_supply(&mut game.players, &game.entities);
        let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
        game.fog.recompute(&ids, &game.entities);
        game
    }

    /// Static info for the `start` message: terrain grid + each player's start tile. The
    /// `player_id` is left 0; the networking layer overwrites it per recipient.
    pub fn start_payload(&self) -> StartPayload {
        let map = MapInfo {
            width: self.map.size,
            height: self.map.size,
            tile_size: config::TILE_SIZE,
            terrain: self.map.terrain.clone(),
        };
        let players = self
            .players
            .iter()
            .map(|p| PlayerStart {
                id: p.id,
                name: p.name.clone(),
                color: p.color.clone(),
                start_tile_x: p.start_tile.0,
                start_tile_y: p.start_tile.1,
            })
            .collect();
        StartPayload {
            player_id: 0,
            tick: self.tick,
            map,
            players,
        }
    }

    /// Queue a command for application at the next tick. No validation here (it happens on
    /// apply, where the live state is known).
    pub fn enqueue(&mut self, player: u32, cmd: Command) {
        self.pending.push((player, cmd));
    }

    /// Advance the simulation by one tick and return per-player transient events.
    ///
    /// Ordered per `DESIGN.md` §3: drain+apply commands → movement → combat → gather →
    /// production+spawn → construction → deaths → recompute supply → recompute fog. The whole
    /// method is panic-free: every entity lookup is fallible and stale ids are ignored.
    pub fn tick(&mut self) -> Vec<(u32, Vec<Event>)> {
        self.tick = self.tick.wrapping_add(1);
        self.pathing.advance_tick(self.tick);

        // Per-player event buckets, accumulated by the systems below.
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        for p in &self.players {
            events.entry(p.id).or_default();
        }

        // Let each AI player decide its actions first, appending ordinary commands to the same
        // pending queue a human client feeds. They are validated on apply just like any client
        // command — the AI gets no special authority over the simulation. Disjoint field borrows
        // (`self.ai` mutably, the rest shared) keep this lock-free.
        let mut pending = std::mem::take(&mut self.pending);
        for controller in self.ai.iter_mut() {
            controller.think(
                &self.map,
                &self.entities,
                &self.spatial,
                &self.players,
                self.tick,
                &mut pending,
            );
        }
        self.record_commands_for_tick(&pending);

        // Run every per-tick system in order. `run_tick` takes split borrows of the map,
        // entity store, player economy, and the event buckets, so it can mutate resources and
        // entities together without locks.
        self.spatial = systems::run_tick(
            &self.map,
            &mut self.entities,
            &mut self.players,
            &self.fog,
            &mut self.pathing,
            pending,
            &mut events,
            self.tick,
        );

        // Fog last, from the post-systems world state.
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog.recompute(&ids, &self.entities);

        // In debug builds, assert that the world state is internally consistent.
        // Panics here mean a system violated a documented invariant.
        #[cfg(debug_assertions)]
        self.assert_invariants();

        // Return events in a stable order (by player id) for determinism.
        let mut out: Vec<(u32, Vec<Event>)> = events.into_iter().collect();
        out.sort_by_key(|(pid, _)| *pid);
        out
    }

    /// Build the fog-filtered snapshot for one player at the current tick. Includes ALL of the
    /// player's own entities plus neutral/enemy entities whose tile is currently visible.
    pub fn snapshot_for(&self, player: u32) -> Snapshot {
        let ps = self.player(player);
        let (steel, oil, supply_used, supply_cap) = match ps {
            Some(p) => (p.steel, p.oil, p.supply_used, p.supply_cap),
            None => (0, 0, 0, 0),
        };

        let mut entities = Vec::new();
        // Use the spatial index for interest filtering instead of a full entity scan.
        for id in self.spatial.all_ids() {
            let e = match self.entities.get(id) {
                Some(e) => e,
                None => continue,
            };
            let own = e.owner == player;
            if !own {
                // Reveal neutral / enemy entities only when their tile is currently visible.
                if !self.fog.is_visible_world(player, e.pos_x, e.pos_y) {
                    continue;
                }
            }
            entities.push(self.view_of(e, player));
        }
        // Deterministic order (stable for tests / replays).
        entities.sort_by_key(|v| v.id);

        Snapshot {
            tick: self.tick,
            steel,
            oil,
            supply_used,
            supply_cap,
            entities,
            // Events are delivered via the `tick()` return value, not the snapshot.
            events: Vec::new(),
        }
    }

    /// Player ids that still own at least one entity.
    pub fn alive_players(&self) -> Vec<u32> {
        self.players
            .iter()
            .map(|p| p.id)
            .filter(|&id| self.entities.player_alive(id))
            .collect()
    }

    /// Remove every entity owned by `player` (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32) {
        let doomed: Vec<u32> = self
            .entities
            .iter()
            .filter(|e| e.owner == player)
            .map(|e| e.id)
            .collect();
        for id in doomed {
            self.entities.remove(id);
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.supply_used = 0;
            p.supply_cap = 0;
        }
        // Recompute fog so the now-entity-less player's visibility goes dark immediately;
        // otherwise a stale grid would keep leaking neutral/enemy entities into their snapshots.
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog.recompute(&ids, &self.entities);
    }

    #[cfg(test)]
    pub fn tick_count(&self) -> u32 {
        self.tick
    }

    /// Authoritative commands applied so far, in exact application order.
    #[cfg(test)]
    pub fn command_log(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    // --- internal helpers ------------------------------------------------------

    fn record_commands_for_tick(&mut self, pending: &[(u32, Command)]) {
        self.command_log
            .extend(pending.iter().map(|(player_id, command)| CommandLogEntry {
                tick: self.tick,
                player_id: *player_id,
                command: command.clone(),
            }));
    }

    fn player(&self, id: u32) -> Option<&PlayerState> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Project an entity into its wire `EntityView` for `viewer`, filling the optional fields
    /// that apply. `viewer` is needed to fog-gate the combat tracer target id.
    fn view_of(&self, e: &Entity, viewer: u32) -> EntityView {
        let mut v = EntityView::new(
            e.id,
            e.owner,
            e.kind.to_protocol_str(),
            e.pos_x,
            e.pos_y,
            e.hp,
            e.max_hp,
            e.state_str(),
        );

        if e.is_unit() {
            v.facing = Some(e.facing);
        }

        // Production buildings: surface the front item + queue depth.
        if e.is_building() && !e.prod_queue.is_empty() {
            if let Some(front) = e.prod_queue.first() {
                v.prod_kind = Some(front.unit.to_protocol_str().to_string());
                v.prod_progress = Some(if front.total == 0 {
                    0.0
                } else {
                    front.progress as f32 / front.total as f32
                });
            }
            v.prod_queue = Some(e.prod_queue.len() as u32);
        }

        // Buildings under construction: surface progress so the client renders scaffolding.
        if e.under_construction {
            v.build_progress = Some(if e.build_total == 0 {
                1.0
            } else {
                (e.build_progress as f32 / e.build_total as f32).min(1.0)
            });
        }

        // Resource nodes: remaining amount.
        if e.is_node() {
            v.remaining = Some(e.remaining);
        }

        // Combat tracer target (only meaningful for attackers actively engaged).
        if let Some(t) = e.target_id {
            // Only expose a target that points at a real combat target, to keep tracers sane.
            if matches!(e.order, Order::Attack { .. } | Order::AttackMove { .. })
                || (e.is_building() && e.can_attack())
            {
                // Fog-gate the tracer: reveal the target id only when the viewer owns the
                // attacker or can currently see the target's tile. Otherwise withholding it
                // avoids leaking the position/existence of an entity hidden in the viewer's fog.
                if let Some(target) = self.entities.get(t) {
                    let visible = e.owner == viewer
                        || self
                            .fog
                            .is_visible_world(viewer, target.pos_x, target.pos_y);
                    if visible {
                        v.target_id = Some(t);
                    }
                }
            }
        }

        v
    }
}

/// Spawn a player's full starting layout: a free, fully-built Industrial Center on the start tile, a ring of
/// workers around it, and a nearby neutral resource cluster (steel + one oil node).
///
/// Resource placement is precisely controlled so every player receives the same relative layout,
/// rotated to point toward the map center. All starting resources lie within
/// [`IC_RESOURCE_MIN_DIST_TILES` .. `IC_RESOURCE_MAX_DIST_TILES`] from the Industrial Center.
fn spawn_player_start(entities: &mut EntityStore, map: &Map, owner: u32, start: (u32, u32)) {
    let (stx, sty) = start;
    let (hx, hy) = map.tile_center(stx, sty);

    // Industrial Center (free, fully built). Footprint is 3x3 centered on the start tile.
    entities.spawn_building(owner, EntityKind::IndustrialCenter, hx, hy, true);

    // Starting workers arranged in a ring just outside the Industrial Center footprint.
    let ts = config::TILE_SIZE as f32;
    let ring_r = ts * 2.5;
    let count = config::STARTING_WORKERS;
    for i in 0..count {
        let ang = std::f32::consts::TAU * (i as f32) / (count.max(1) as f32);
        let wx = hx + ring_r * ang.cos();
        let wy = hy + ring_r * ang.sin();
        entities.spawn_unit(owner, EntityKind::Worker, wx, wy);
    }

    // Determine the angle toward the map center so the resource cluster points inward.
    let center = map.world_size_px() * 0.5;
    let dx = center - hx;
    let dy = center - hy;
    let base_angle = dy.atan2(dx);

    // Steel cluster: a 4x2 block centered at a fixed distance from the IC, rotated to face
    // the map center. This guarantees identical distances for every player.
    let block_dist = config::STEEL_BLOCK_DIST_TILES * ts;
    let block_cx = hx + block_dist * base_angle.cos();
    let block_cy = hy + block_dist * base_angle.sin();

    // Perpendicular axis (rotated 90 deg from base_angle) for the block width.
    let perp_x = -base_angle.sin();
    let perp_y = base_angle.cos();

    let patches = config::STEEL_PATCHES_PER_BASE;
    let cols = 4;
    let _rows = 2;
    for i in 0..patches {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        // Local offsets within the block, centered on the block center.
        let off_x = (col - 1.5) * ts; // -1.5 .. +1.5 tiles
        let off_y = (row - 0.5) * ts; // -0.5 .. +0.5 tiles
        let px = block_cx + off_x * perp_x + off_y * base_angle.cos();
        let py = block_cy + off_x * perp_y + off_y * base_angle.sin();
        let dist_tiles = ((px - hx).powi(2) + (py - hy).powi(2)).sqrt() / ts;
        debug_assert!(
            (config::IC_RESOURCE_MIN_DIST_TILES..=config::IC_RESOURCE_MAX_DIST_TILES)
                .contains(&dist_tiles),
            "steel patch {i} at {dist_tiles:.2} tiles from IC is out of [{:.1}, {:.1}] bounds",
            config::IC_RESOURCE_MIN_DIST_TILES,
            config::IC_RESOURCE_MAX_DIST_TILES
        );
        entities.spawn_node(EntityKind::Steel, px, py);
    }

    // Oil node sits at a fixed distance further out in the same direction.
    let oil_dist = config::OIL_DIST_TILES * ts;
    let gx = hx + oil_dist * base_angle.cos();
    let gy = hy + oil_dist * base_angle.sin();
    let oil_tiles = oil_dist / ts;
    debug_assert!(
        (config::IC_RESOURCE_MIN_DIST_TILES..=config::IC_RESOURCE_MAX_DIST_TILES)
            .contains(&oil_tiles),
        "oil at {oil_tiles:.2} tiles from IC is out of [{:.1}, {:.1}] bounds",
        config::IC_RESOURCE_MIN_DIST_TILES,
        config::IC_RESOURCE_MAX_DIST_TILES
    );
    entities.spawn_node(EntityKind::Oil, gx, gy);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::kinds;

    /// Drive a passive human vs. one AI and confirm the AI actually plays: it grows its economy,
    /// expands supply, builds a barracks, produces riflemen, and marches them into the human base
    /// to deal damage. This exercises the full command path the AI shares with human clients.
    #[test]
    fn ai_builds_economy_and_attacks() {
        let players = [
            PlayerInit {
                id: 1,
                name: "Human".into(),
                color: "#fff".into(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                name: "Computer".into(),
                color: "#000".into(),
                is_ai: true,
            },
        ];
        let mut game = Game::new(&players);

        let mut max_workers = 0usize;
        let mut max_riflemen = 0usize;
        let mut ever_had_barracks = false;
        let mut ai_supply_cap = 0u32;
        let mut human_damaged = false;
        let mut event_log = Vec::new();

        // ~200s of simulation. The human issues no commands (passive target).
        for tick in 1..=6000 {
            for (player_id, events) in game.tick() {
                for event in events {
                    event_log.push(super::replay::EventLogEntry {
                        tick,
                        player_id,
                        event,
                    });
                }
            }

            let ai = game.snapshot_for(2);
            ai_supply_cap = ai.supply_cap.max(ai_supply_cap);
            let workers = ai
                .entities
                .iter()
                .filter(|e| e.owner == 2 && e.kind == kinds::WORKER)
                .count();
            let riflemen = ai
                .entities
                .iter()
                .filter(|e| e.owner == 2 && e.kind == kinds::RIFLEMAN)
                .count();
            max_workers = max_workers.max(workers);
            max_riflemen = max_riflemen.max(riflemen);
            if ai
                .entities
                .iter()
                .any(|e| e.owner == 2 && e.kind == kinds::BARRACKS)
            {
                ever_had_barracks = true;
            }

            // Any human entity below full hp means an AI attack landed.
            let human = game.snapshot_for(1);
            if human
                .entities
                .iter()
                .any(|e| e.owner == 1 && e.hp < e.max_hp)
            {
                human_damaged = true;
            }
        }

        assert!(
            max_workers > config::STARTING_WORKERS as usize,
            "AI should train workers beyond the {} it starts with (saw {max_workers})",
            config::STARTING_WORKERS
        );
        assert!(
            ai_supply_cap > config::INDUSTRIAL_CENTER_SUPPLY,
            "AI should build a depot to raise supply above the Industrial Center's {} (saw {ai_supply_cap})",
            config::INDUSTRIAL_CENTER_SUPPLY
        );
        assert!(ever_had_barracks, "AI should build a barracks");
        assert!(max_riflemen > 0, "AI should produce riflemen");
        assert!(
            human_damaged,
            "AI riflemen should reach and damage the human base"
        );

        // Replay determinism: the same command log fed into a fresh game must reproduce
        // the exact events and final snapshots.
        selfplay::assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(
            |failure| {
                panic!("AI replay determinism failed: {}", failure.reason());
            },
        );
    }

    /// Adding an AI must not perturb a human-only game's construction: an all-human match has no
    /// controllers and behaves exactly as before.
    #[test]
    fn no_ai_controllers_without_ai_players() {
        let players = [PlayerInit {
            id: 1,
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let game = Game::new(&players);
        assert!(
            game.ai.is_empty(),
            "a human-only match has no AI controllers"
        );
    }

    /// A one-player sandbox with no commands must still be deterministic: fog, supply, and the
    /// spatial index rebuild identically every tick, and replaying the empty command log
    /// reproduces the same final snapshot.
    #[test]
    fn no_commands_one_player_is_deterministic() {
        let players = [PlayerInit {
            id: 1,
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players);

        let mut event_log = Vec::new();
        for tick in 1..=300 {
            for (player_id, events) in game.tick() {
                for event in events {
                    event_log.push(super::replay::EventLogEntry {
                        tick,
                        player_id,
                        event,
                    });
                }
            }
        }

        assert!(
            event_log.is_empty(),
            "a one-player sandbox with no commands should emit no events"
        );

        selfplay::assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(
            |failure| {
                panic!(
                    "one-player no-commands replay determinism failed: {}",
                    failure.reason()
                );
            },
        );
    }

    /// Every player must receive the same relative resource layout, and all starting resources
    /// must fall within the configured min/max distance from the Industrial Center.
    #[test]
    fn spawn_resource_distances_are_fair_and_symmetric() {
        let counts = [1, 2, 3, 4];
        for &pc in &counts {
            let players: Vec<PlayerInit> = (1..=pc)
                .map(|id| PlayerInit {
                    id,
                    name: format!("P{id}"),
                    color: "#fff".into(),
                    is_ai: false,
                })
                .collect();
            let game = Game::new_for_replay(&players);

            let mut all_player_dists: Vec<Vec<(EntityKind, f32)>> = Vec::new();
            for p in &game.players {
                let ic = game
                    .entities
                    .iter()
                    .find(|e| e.owner == p.id && e.kind == EntityKind::IndustrialCenter)
                    .expect("Industrial Center exists for every player");

                let mut dists = Vec::new();
                for e in game.entities.iter() {
                    if e.owner != 0 || (!e.is_node()) {
                        continue;
                    }
                    let d_x = e.pos_x - ic.pos_x;
                    let d_y = e.pos_y - ic.pos_y;
                    let dist_tiles = (d_x * d_x + d_y * d_y).sqrt() / config::TILE_SIZE as f32;

                    // Only consider nodes that belong to this player's start cluster.
                    if dist_tiles <= config::IC_RESOURCE_MAX_DIST_TILES + 1.0 {
                        dists.push((e.kind, dist_tiles));
                        assert!(
                            dist_tiles >= config::IC_RESOURCE_MIN_DIST_TILES,
                            "player {} has a {:?} node too close ({:.2} tiles) to their IC",
                            p.id,
                            e.kind,
                            dist_tiles
                        );
                        assert!(
                            dist_tiles <= config::IC_RESOURCE_MAX_DIST_TILES,
                            "player {} has a {:?} node too far ({:.2} tiles) from their IC",
                            p.id,
                            e.kind,
                            dist_tiles
                        );
                    }
                }
                // Sort for deterministic comparison.
                dists.sort_by(|a, b| {
                    let kind_ord = a.0.to_protocol_str().cmp(b.0.to_protocol_str());
                    if kind_ord != std::cmp::Ordering::Equal {
                        return kind_ord;
                    }
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                });
                all_player_dists.push(dists);
            }

            // Every player in the same match must have identical distance sets.
            if let Some(first) = all_player_dists.first() {
                for (i, other) in all_player_dists.iter().enumerate().skip(1) {
                    assert_eq!(
                        first.len(),
                        other.len(),
                        "player count {}: player {} has a different number of nearby resources",
                        pc,
                        i + 1
                    );
                    for (j, ((ek_a, da), (ek_b, db))) in first.iter().zip(other.iter()).enumerate()
                    {
                        assert_eq!(*ek_a, *ek_b, "mismatched resource kind at index {j}");
                        assert!(
                            (da - db).abs() < 0.01,
                            "player count {pc}: resource {j} distance mismatch — player 1 has {:.3} tiles, player {} has {:.3} tiles",
                            da,
                            i + 1,
                            db
                        );
                    }
                }
            }
        }
    }
}
