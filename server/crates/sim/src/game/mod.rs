//! The authoritative game simulation. See `docs/design/server-sim.md` for the public API contract.
//!
//! [`Game`] is the single seam between the simulation and the networking/lobby layer. The
//! networking layer calls ONLY the methods in §3.1; everything else here is private detail.
//!
//! The simulation is fixed-rate: each [`Game::tick`] drains queued commands, advances every
//! system in a deterministic order, and recomputes per-player fog. Snapshots are pulled
//! separately via [`Game::snapshot_for`], fog-filtered so a player only ever sees neutral /
//! enemy entities on tiles they currently see.

pub(crate) mod ability;
pub(crate) mod ability_projectile;
mod ability_projection;
pub(crate) mod ability_runtime;
mod analysis;
mod artillery;
mod building_memory;
pub mod command;
mod commands;
pub mod entity;
pub(crate) mod fog;
mod hero_abilities;
mod invariants;
pub mod map;
mod mortar;
mod pathfinding;
mod player_state;
pub mod replay;
mod scoring;
pub(crate) mod services;
mod setup;
pub(crate) mod smoke;
mod snapshot;
mod systems;
pub mod teams;
pub mod upgrade;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::config;
use crate::protocol::{
    Event, MapInfo, PlayerResourceSnapshot, PlayerScore, PlayerStart, RememberedBuildingView,
    ResourceDelta, ResourceNode, Snapshot, StartPayload, DEFAULT_FACTION_ID,
};
use crate::rules::{economy as economy_rules, projection};
use ability_runtime::AbilityRuntime;
use serde::{Deserialize, Serialize};

use artillery::ArtilleryShellStore;
use building_memory::{BuildingMemory, BuildingMemoryEntry};
use entity::{BuildPhase, EntityKind, EntityStore};
use fog::{Fog, LingeringSightSource};
use map::Map;
pub use map::MapMetadata;
use mortar::MortarShellStore;
use rand::rngs::SmallRng;
use rand::SeedableRng;
use replay::CommandLogEntry;
pub(crate) use setup::StartingLoadout;
use smoke::SmokeCloudStore;

pub use crate::game::command::SimCommand;
pub use teams::TeamId;

const AI_WORKER_RETREAT_TILES: f32 = 5.0;

/// Lobby-supplied identity for a player joining a match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerInit {
    pub id: u32,
    #[serde(default)]
    pub team_id: TeamId,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    /// When true this player is a computer opponent: it has no socket and is driven by the
    /// caller's AI orchestration instead of receiving snapshots / sending commands.
    pub is_ai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlayerStartingLoadout {
    pub player_id: u32,
    pub faction_id: String,
    pub loadout_id: String,
    pub starting_steel: u32,
    pub starting_oil: u32,
}

/// Per-player economy and bookkeeping carried for the whole match. Visible to `systems` (the
/// only other module that mutates economy), but not part of the public API.
#[derive(Clone)]
pub(crate) struct PlayerState {
    pub(crate) id: u32,
    pub(crate) team_id: TeamId,
    pub(crate) faction_id: String,
    pub(crate) name: String,
    pub(crate) color: String,
    pub(crate) start_tile: (u32, u32),
    pub(crate) steel: u32,
    pub(crate) oil: u32,
    /// Supply currently consumed by living + in-production units.
    pub(crate) supply_used: u32,
    /// Supply provided by completed City Centres and Depots, capped at `SUPPLY_CAP_MAX`.
    pub(crate) supply_cap: u32,
    pub(crate) is_ai: bool,
    pub(crate) score: ScoreState,
    pub(crate) upgrades: BTreeSet<upgrade::UpgradeKind>,
}

/// Per-player score-screen counters. Values are accumulated from authoritative entity lifecycle
/// events, not inferred from fog-filtered snapshots.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScoreState {
    unit_score: u32,
    structure_score: u32,
    units_killed: u32,
    units_lost: u32,
    buildings_killed: u32,
    buildings_lost: u32,
    units_lost_by_kind: BTreeMap<EntityKind, u32>,
}

/// The authoritative match state.
#[derive(Clone)]
pub struct Game {
    map: Map,
    entities: EntityStore,
    fog: Fog,
    building_memory: BuildingMemory,
    players: Vec<PlayerState>,
    /// Commands received this tick window, drained at the start of [`tick`]. Each carries the
    /// issuing player so ownership can be validated on apply.
    pending: Vec<(u32, SimCommand)>,
    /// Authoritative commands stamped with the tick where they were applied. Includes AI commands
    /// because they are emitted into the same pending queue before command application.
    command_log: Vec<CommandLogEntry>,
    tick: u32,
    /// Post-tick spatial index, rebuilt every tick after all systems run so [`snapshot_for`]
    /// can use it for interest filtering without rebuilding.
    spatial: services::spatial::SpatialIndex,
    /// Persistent pathfinding service with an LRU cache for verified paths.
    pathing: services::pathing::PathingService,
    /// Five-second death-vision sources used only when building fog-filtered snapshots.
    lingering_sight: Vec<LingeringSightSource>,
    /// Neutral smoke clouds that block authoritative fog and combat LOS without being entities.
    smokes: SmokeCloudStore,
    /// Persistent ability runtime state that is not a normal entity or one-off projectile event.
    ability_runtime: AbilityRuntime,
    /// Delayed mortar shell impacts waiting to resolve area damage.
    mortar_shells: MortarShellStore,
    /// Delayed artillery shell impacts waiting to resolve area damage.
    artillery_shells: ArtilleryShellStore,
    /// Match seed retained for replay metadata/API compatibility. The current hardcoded map
    /// ignores it until lobby map selection or randomized maps are reintroduced.
    seed: u32,
    /// Per-player faction loadouts used to build the initial match state. Replays persist this
    /// alongside player faction ids so mixed starts do not collapse into one global resource pair.
    starting_loadouts: Vec<PlayerStartingLoadout>,
    /// Stable authored map identity used by replay artifacts.
    map_metadata: MapMetadata,
    /// True for lobby "Debug mode" matches; enables owner-only movement path diagnostics in
    /// snapshots even when the server binary is built in release mode.
    debug_path_overlays: bool,
    /// Under-construction building ids that received authoritative build progress this tick.
    active_construction_sites: BTreeSet<u32>,
    starting_loadout: StartingLoadout,
    pub(crate) rng: SmallRng,
}

impl Game {
    /// Clone the complete authoritative simulation state for replay seek keyframes.
    ///
    /// This is intentionally narrower than exposing checkpoint serialization: replay playback
    /// runs in-process on the room task, so an owned clone keeps all internal service state
    /// deterministic without making snapshots part of the restore contract.
    pub fn clone_for_replay_keyframe(&self) -> Self {
        self.clone()
    }

    /// Advance the simulation by one tick and return per-player transient events.
    ///
    /// Ordered per `docs/design/server-sim.md`: drain+apply commands → movement → combat → gather →
    /// production+spawn → construction → deaths → recompute supply → recompute fog. The whole
    /// method is panic-free: every entity lookup is fallible and stale ids are ignored.
    pub fn tick(&mut self) -> Vec<(u32, Vec<Event>)> {
        self.tick_inner(None)
    }

    pub fn tick_with_perf(
        &mut self,
        perf: Option<&mut crate::perf::TickPerf>,
    ) -> Vec<(u32, Vec<Event>)> {
        self.tick_inner(perf)
    }

    fn tick_inner(
        &mut self,
        mut perf: Option<&mut crate::perf::TickPerf>,
    ) -> Vec<(u32, Vec<Event>)> {
        self.tick = self.tick.wrapping_add(1);
        self.pathing.advance_tick(self.tick);
        self.smokes.retain_active(self.tick);

        // Per-player event buckets, accumulated by the systems below.
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        for p in &self.players {
            events.entry(p.id).or_default();
        }

        let pending = std::mem::take(&mut self.pending);
        crate::perf::timed(perf.as_deref_mut(), "record_commands", || {
            self.record_commands_for_tick(&pending);
        });
        self.active_construction_sites.clear();

        // Run every per-tick system in order. `run_tick` takes split borrows of the map,
        // entity store, player economy, and the event buckets, so it can mutate resources and
        // entities together without locks.
        self.spatial = systems::run_tick(
            &self.map,
            &mut self.entities,
            &mut self.players,
            &self.fog,
            &mut self.pathing,
            &mut self.rng,
            &mut self.lingering_sight,
            &mut self.smokes,
            &mut self.ability_runtime,
            &mut self.mortar_shells,
            &mut self.artillery_shells,
            &mut self.active_construction_sites,
            pending,
            &mut events,
            self.tick,
            perf.as_deref_mut(),
        );

        // Live fog last, from the post-systems world state. Lingering death vision is layered
        // only when snapshots are projected so it cannot validate commands or combat targeting.
        self.lingering_sight
            .retain(|source| source.is_active_at(self.tick));
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        crate::perf::timed(perf.as_deref_mut(), "fog_recompute", || {
            self.fog
                .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        });
        self.refresh_building_memory(&ids);

        // In debug builds, assert that the world state is internally consistent.
        // Panics here mean a system violated a documented invariant.
        #[cfg(debug_assertions)]
        crate::perf::timed(perf, "debug_invariants", || {
            self.assert_invariants();
        });

        // Return events in a stable order (by player id) for determinism.
        let mut out: Vec<(u32, Vec<Event>)> = events.into_iter().collect();
        out.sort_by_key(|(pid, _)| *pid);
        out
    }

    pub fn current_tick(&self) -> u32 {
        self.tick
    }

    /// Ordinary retreat commands for AI-owned workers hit on the previous tick.
    ///
    /// This exposes the former live-AI direct-hit reflex without letting the AI crate read private
    /// entity state. Callers still enqueue the returned commands through [`Game::enqueue`], so the
    /// normal command validation and replay logging path applies.
    pub fn worker_retreat_commands_for(&self, player: u32) -> Vec<SimCommand> {
        let last_tick = self.tick.checked_sub(1);
        let world_max = self.map.world_size_px() - 0.01;
        let retreat_px = AI_WORKER_RETREAT_TILES * config::TILE_SIZE as f32;
        let mut commands = Vec::new();
        for entity in self.entities.iter() {
            if entity.owner != player || entity.kind != EntityKind::Worker || entity.hp == 0 {
                continue;
            }
            if matches!(entity.build_phase(), Some(BuildPhase::Constructing { .. })) {
                continue;
            }
            if entity.last_damage_tick() != last_tick {
                continue;
            }
            let Some((ax, ay)) = entity.last_damage_pos() else {
                continue;
            };
            let (vx, vy) = (entity.pos_x, entity.pos_y);
            let dx = vx - ax;
            let dy = vy - ay;
            let dist = (dx * dx + dy * dy).sqrt();
            let (ux, uy) = if dist > f32::EPSILON && dist.is_finite() {
                (dx / dist, dy / dist)
            } else {
                (1.0, 0.0)
            };
            commands.push(SimCommand::Move {
                units: vec![entity.id],
                x: (vx + ux * retreat_px).clamp(0.0, world_max),
                y: (vy + uy * retreat_px).clamp(0.0, world_max),
                queued: false,
            });
        }
        commands
    }

    pub fn perf_entity_counts(&self) -> crate::perf::EntityCounts {
        let mut counts = crate::perf::EntityCounts::default();
        for entity in self.entities.iter() {
            counts.entities += 1;
            if entity.is_unit() {
                counts.units += 1;
            } else if entity.is_building() {
                counts.buildings += 1;
            } else if entity.is_node() {
                counts.resources += 1;
            }
        }
        counts
    }

    /// Player ids that are not yet defeated. Human players are defeated when they lose all
    /// buildings; AI players are also defeated when they have no units left.
    pub fn alive_players(&self) -> Vec<u32> {
        self.players
            .iter()
            .filter(|p| {
                let has_building =
                    services::world_query::owned_survival_buildings(&self.entities, p.id)
                        .next()
                        .is_some();
                if !has_building {
                    return false;
                }
                if p.is_ai {
                    services::world_query::owned_units(&self.entities, p.id)
                        .next()
                        .is_some()
                } else {
                    true
                }
            })
            .map(|p| p.id)
            .collect()
    }

    /// Remove every entity owned by `player` (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32) {
        let doomed: Vec<u32> = services::world_query::owned_units(&self.entities, player)
            .chain(services::world_query::owned_buildings(
                &self.entities,
                player,
            ))
            .map(|e| e.id)
            .collect();
        for id in doomed {
            if let Some(entity) = self.entities.remove(id) {
                if let Some(p) = self.players.iter_mut().find(|p| p.id == entity.owner) {
                    p.record_entity_lost(entity.kind);
                }
            }
        }
        if let Some(p) = self.players.iter_mut().find(|p| p.id == player) {
            p.reset_supply();
        }
        self.lingering_sight
            .retain(|source| source.owner() != player);
        // Recompute fog so the now-entity-less player's visibility goes dark immediately;
        // otherwise a stale grid would keep leaking neutral/enemy entities into their snapshots.
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog
            .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        self.refresh_building_memory(&ids);
    }

    pub fn tick_count(&self) -> u32 {
        self.tick
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn spawn_smoke_cloud_for_test(&mut self, x: f32, y: f32) -> Option<u32> {
        let (x, y) = SmokeCloudStore::clamp_point_to_map(&self.map, x, y)?;
        let id = self.smokes.spawn(
            x,
            y,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            self.tick,
        )?;
        let ids: Vec<u32> = self.players.iter().map(|p| p.id).collect();
        self.fog
            .recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        self.refresh_building_memory(&ids);
        Some(id)
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(in crate::game) fn spawn_ability_world_object_for_test(
        &mut self,
        spec: ability_runtime::AbilityWorldObjectSpec,
    ) -> Option<u32> {
        self.ability_runtime.spawn_world_object(spec)
    }

    /// Authoritative commands applied so far, in exact application order.
    #[allow(dead_code)]
    pub fn command_log(&self) -> &[CommandLogEntry] {
        &self.command_log
    }

    /// Reconstruct the `PlayerInit` list this game was created from, so a crash/invariant
    /// failure can persist a replayable artifact.
    pub fn player_inits(&self) -> Vec<PlayerInit> {
        self.players
            .iter()
            .map(|p| PlayerInit {
                id: p.id,
                team_id: p.team_id,
                faction_id: p.faction_id.clone(),
                name: p.name.clone(),
                color: p.color.clone(),
                is_ai: p.is_ai,
            })
            .collect()
    }

    // --- internal helpers ------------------------------------------------------
    fn refresh_building_memory(&mut self, player_ids: &[u32]) {
        let teams = self.team_relations();
        self.building_memory.refresh(
            player_ids,
            &self.entities,
            &self.fog,
            &self.map,
            &self.smokes,
            &teams,
            self.tick,
        );
    }

    pub(crate) fn team_relations(&self) -> teams::TeamRelations {
        teams::TeamRelations::from_player_teams(self.players.iter().map(|p| (p.id, p.team_id)))
    }

    #[allow(dead_code)]
    pub(crate) fn building_memory_for(
        &self,
        player: u32,
        building: u32,
    ) -> Option<&BuildingMemoryEntry> {
        self.building_memory.get(player, building)
    }
}

#[cfg(test)]
mod ability_projection_tests;
#[cfg(test)]
mod phase7_privacy_tests;
#[cfg(test)]
mod snapshot_memory_tests;
#[cfg(test)]
mod tests;
