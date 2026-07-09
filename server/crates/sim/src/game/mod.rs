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
mod checkpoint;
pub mod command;
mod commands;
mod derived_state;
pub mod entity;
mod entrenchment_combat;
mod firing_reveal;
pub(crate) mod fog;
mod hero_abilities;
mod invariants;
pub mod lab;
pub mod map;
mod mortar;
mod mortar_scatter;
mod panzerfaust_shot;
mod pathfinding;
mod player_state;
pub mod replay;
mod replay_artifact;
mod resource_placement;
#[cfg(test)]
mod resource_placement_tests;
mod scoring;
pub(crate) mod services;
mod setup;
pub(crate) mod smoke;
mod snapshot;
mod state;
mod systems;
pub mod teams;
pub(crate) mod trench;
pub mod upgrade;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::config;
use crate::protocol::{
    Event, MapInfo, PlayerResourceSnapshot, PlayerScore, PlayerStart, RememberedBuildingView,
    ResourceDelta, ResourceNode, Snapshot, StartPayload, DEFAULT_FACTION_ID,
};
use crate::rules::{economy as economy_rules, projection};
use serde::{Deserialize, Serialize};

use building_memory::BuildingMemoryEntry;
use derived_state::DerivedState;
use entity::{BuildPhase, EntityKind, EntityStore};
use fog::Fog;
use map::Map;
pub use map::MapMetadata;
#[cfg(test)]
pub(in crate::game) use mortar::MortarShellStore;
use replay::CommandLogEntry;
pub(crate) use setup::StartingLoadout;
use smoke::SmokeCloudStore;
use state::GameState;

pub use crate::game::command::SimCommand;
pub use teams::TeamId;

const AI_WORKER_RETREAT_TILES: f32 = 5.0;

fn primary_base_kind(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::CityCentre | EntityKind::Zamok)
}

fn primary_base_distance_sq(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SnapshotOptions {
    pub include_movement_paths: bool,
    pub movement_paths_for_all_projected: bool,
}

impl SnapshotOptions {
    fn debug_path_projection(self) -> projection::DebugPathProjection {
        match (
            self.include_movement_paths,
            self.movement_paths_for_all_projected,
        ) {
            (false, _) => projection::DebugPathProjection::None,
            (true, false) => projection::DebugPathProjection::OwnerOnly,
            (true, true) => projection::DebugPathProjection::AllProjected,
        }
    }
}

/// Per-player economy and bookkeeping carried for the whole match. Visible to `systems` (the
/// only other module that mutates economy), but not part of the public API.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScoreState {
    unit_score: u32,
    structure_score: u32,
    units_killed: u32,
    units_lost: u32,
    buildings_killed: u32,
    buildings_lost: u32,
    units_lost_by_kind: BTreeMap<EntityKind, u32>,
    resources_mined: ResourceTotals,
    resource_income_history: Vec<ResourceIncomeRecord>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceTotals {
    steel: u32,
    oil: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ResourceIncomeRecord {
    tick: u32,
    steel: u32,
    oil: u32,
}

/// The authoritative match state.
#[derive(Clone)]
pub struct Game {
    /// Durable authoritative state plus setup/replay compatibility metadata.
    pub(in crate::game) state: GameState,
    /// Rebuildable cache and index state: final post-tick spatial index plus pathing cache.
    pub(in crate::game) derived: DerivedState,
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
    /// Ordered per `docs/design/server-sim.md`: drain+apply commands → movement → queued-order
    /// promotion → combat/economy/production → construction/deconstruction → projectile/death
    /// cleanup → collision/supply → recompute fog. The whole method is panic-free: every entity
    /// lookup is fallible and stale ids are ignored.
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
        self.state.tick = self.state.tick.wrapping_add(1);
        self.derived.advance_pathing_tick(self.state.tick);
        self.state.smokes.retain_active(self.state.tick);
        let player_ids = self.state.player_ids();
        if self.retain_active_visibility_sources() {
            self.recompute_live_fog(&player_ids);
        }

        // Per-player event buckets, accumulated by the systems below.
        let mut events: HashMap<u32, Vec<Event>> = HashMap::new();
        for p in &self.state.players {
            events.entry(p.id).or_default();
        }

        let pending = std::mem::take(&mut self.state.pending);
        crate::perf::timed(perf.as_deref_mut(), "record_commands", || {
            self.record_commands_for_tick(&pending);
        });
        self.state.active_construction_sites.clear();
        if !self.state.lab_god_mode_players.is_empty() {
            self.sync_lab_god_mode_flags();
        }

        // Run every per-tick system in order. `run_tick` takes split borrows of the map,
        // entity store, player economy, and the event buckets, so it can mutate resources and
        // entities together without locks.
        let active_vision_players = self.alive_players().into_iter().collect::<BTreeSet<_>>();
        let final_spatial = systems::run_tick(
            &self.state.map,
            &mut self.state.entities,
            &mut self.state.players,
            &active_vision_players,
            &self.state.fog,
            self.derived.pathing_mut(),
            &mut self.state.rng,
            &mut self.state.lingering_sight,
            &mut self.state.firing_reveals,
            &mut self.state.smokes,
            &mut self.state.trenches,
            &mut self.state.ability_runtime,
            &mut self.state.mortar_shells,
            &mut self.state.artillery_shells,
            &mut self.state.panzerfaust_shots,
            &mut self.state.active_construction_sites,
            pending,
            &mut events,
            self.state.tick,
            perf.as_deref_mut(),
        );
        self.derived.set_final_spatial(final_spatial);

        // Live fog last, from the post-systems world state. Lingering death vision is stamped as
        // ordinary temporary sight so snapshots, commands, and combat all consume one visibility
        // model.
        self.retain_active_visibility_sources();
        crate::perf::timed(perf.as_deref_mut(), "fog_recompute", || {
            self.recompute_live_fog(&player_ids);
        });
        self.refresh_building_memory(&player_ids);
        self.refresh_trench_memory(&player_ids);

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
        self.state.tick
    }

    /// Ordinary retreat commands for AI-owned workers hit on the previous tick.
    ///
    /// This exposes the former live-AI direct-hit reflex without letting the AI crate read private
    /// entity state. Callers still enqueue the returned commands through [`Game::enqueue`], so the
    /// normal command validation and replay logging path applies.
    pub fn worker_retreat_commands_for(&self, player: u32) -> Vec<SimCommand> {
        let last_tick = self.state.tick.checked_sub(1);
        let world_max = self.state.map.world_size_px() - 0.01;
        let retreat_px = AI_WORKER_RETREAT_TILES * config::TILE_SIZE as f32;
        let mut commands = Vec::new();
        for entity in self.state.entities.iter() {
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
        for entity in self.state.entities.iter() {
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
        self.state
            .players
            .iter()
            .filter(|p| {
                let has_building =
                    services::world_query::owned_survival_buildings(&self.state.entities, p.id)
                        .next()
                        .is_some();
                if !has_building {
                    return false;
                }
                if p.is_ai {
                    services::world_query::owned_units(&self.state.entities, p.id)
                        .any(|entity| entity.hp > 0 && entity.is_targetable())
                } else {
                    true
                }
            })
            .map(|p| p.id)
            .collect()
    }

    /// Player ids whose starting main base is still alive.
    ///
    /// This is an objective query for AI-vs-AI matches and diagnostics. It intentionally does
    /// not replace the normal match elimination rule in [`Game::alive_players`].
    pub fn primary_base_alive_players(&self) -> Vec<u32> {
        self.state
            .players
            .iter()
            .filter(|player| self.primary_base_alive_for(player.id, player.start_tile))
            .map(|player| player.id)
            .collect()
    }

    fn primary_base_alive_for(&self, player_id: u32, start_tile: (u32, u32)) -> bool {
        let (start_x, start_y) = self
            .state
            .map
            .tile_center(start_tile.0, start_tile.1);
        let max_dist = config::TILE_SIZE as f32 * 0.5;
        let max_dist_sq = max_dist * max_dist;
        services::world_query::owned_buildings(&self.state.entities, player_id).any(|entity| {
            primary_base_kind(entity.kind)
                && entity.hp > 0
                && !entity.under_construction()
                && primary_base_distance_sq(entity.pos_x, entity.pos_y, start_x, start_y)
                    <= max_dist_sq
        })
    }

    /// Remove every entity owned by `player` (e.g. on disconnect) so the match can resolve.
    pub fn eliminate(&mut self, player: u32) {
        let doomed: Vec<u32> = services::world_query::owned_units(&self.state.entities, player)
            .chain(services::world_query::owned_buildings(
                &self.state.entities,
                player,
            ))
            .map(|e| e.id)
            .collect();
        for id in doomed {
            if let Some(entity) = self.state.entities.remove(id) {
                if let Some(p) = self.state.players.iter_mut().find(|p| p.id == entity.owner) {
                    p.record_entity_lost(entity.kind);
                }
            }
        }
        if let Some(p) = self.state.players.iter_mut().find(|p| p.id == player) {
            p.reset_supply();
        }
        self.state
            .lingering_sight
            .retain(|source| source.owner() != player);
        self.state
            .firing_reveals
            .retain(|source| source.viewer() != player);
        // Recompute fog so the now-entity-less player's visibility goes dark immediately;
        // otherwise a stale grid would keep leaking neutral/enemy entities into their snapshots.
        let ids = self.state.player_ids();
        self.recompute_live_fog(&ids);
        self.refresh_building_memory(&ids);
        self.refresh_trench_memory(&ids);
    }

    pub fn tick_count(&self) -> u32 {
        self.state.tick
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn spawn_smoke_cloud_for_test(&mut self, x: f32, y: f32) -> Option<u32> {
        let (x, y) = SmokeCloudStore::clamp_point_to_map(&self.state.map, x, y)?;
        let id = self.state.smokes.spawn(
            x,
            y,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            self.state.tick,
        )?;
        let ids = self.state.player_ids();
        self.recompute_live_fog(&ids);
        self.refresh_building_memory(&ids);
        self.refresh_trench_memory(&ids);
        Some(id)
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(crate) fn spawn_trench_for_test(&mut self, x: f32, y: f32) -> Option<u32> {
        let id = self.state.trenches.create(&self.state.map, x, y)?;
        let ids = self.state.player_ids();
        self.refresh_trench_memory(&ids);
        Some(id)
    }

    #[allow(dead_code)]
    #[cfg(any(test, debug_assertions))]
    pub(in crate::game) fn spawn_ability_world_object_for_test(
        &mut self,
        spec: ability_runtime::AbilityWorldObjectSpec,
    ) -> Option<u32> {
        self.state.ability_runtime.spawn_world_object(spec)
    }

    /// Authoritative commands applied so far, in exact application order.
    #[allow(dead_code)]
    pub fn command_log(&self) -> &[CommandLogEntry] {
        &self.state.command_log
    }

    /// Reconstruct the `PlayerInit` list this game was created from, so a crash/invariant
    /// failure can persist a replayable artifact.
    pub fn player_inits(&self) -> Vec<PlayerInit> {
        self.state
            .players
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
    #[cfg(test)]
    pub(in crate::game) fn clear_and_rebuild_derived_state_for_test(&mut self) {
        self.derived
            .clear_and_rebuild_from_authoritative(&self.state.map, &self.state.entities);
    }

    pub(in crate::game) fn final_spatial(&self) -> &services::spatial::SpatialIndex {
        self.derived.final_spatial()
    }

    #[cfg(test)]
    pub(in crate::game) fn rebuild_final_spatial(&mut self) {
        self.derived
            .rebuild_final_spatial(&self.state.map, &self.state.entities);
    }

    pub(in crate::game) fn reset_derived_state(&mut self) {
        let (default_pathing_budget, pathing_cache_capacity) = self.derived.pathing_config();
        self.derived = DerivedState::new(
            &self.state.map,
            &self.state.entities,
            default_pathing_budget,
            pathing_cache_capacity,
        );
        self.derived.advance_pathing_tick(self.state.tick);
    }

    #[cfg(test)]
    pub(in crate::game) fn pathing_cache_len_for_test(&self) -> usize {
        self.derived.pathing_cache_len_for_test()
    }

    #[cfg(test)]
    pub(in crate::game) fn pathing_config_for_test(&self) -> (usize, usize) {
        self.derived.pathing_config_for_test()
    }

    fn refresh_building_memory(&mut self, player_ids: &[u32]) {
        let teams = self.team_relations();
        self.state.building_memory.refresh(
            player_ids,
            &self.state.entities,
            &self.state.fog,
            &self.state.map,
            &self.state.smokes,
            &teams,
            self.state.tick,
        );
    }

    pub(in crate::game) fn refresh_trench_memory(&mut self, player_ids: &[u32]) {
        for &player in player_ids {
            let fog = self.team_current_fog_for(player, &self.state.fog);
            self.state.trenches.refresh_memory_for_player(player, &fog);
        }
    }

    fn recompute_live_fog(&mut self, player_ids: &[u32]) {
        self.state.fog.recompute_with_smoke(
            player_ids,
            &self.state.entities,
            &self.state.map,
            &self.state.smokes,
        );
        let teams = self.team_relations();
        self.state
            .fog
            .stamp_scout_plane_sources_for_teams_with_smoke(
                &self.state.map,
                &self.state.entities,
                &self.state.smokes,
                &teams,
            );
        self.state.fog.stamp_lingering_sources_for_teams_with_smoke(
            &self.state.lingering_sight,
            &self.state.map,
            &self.state.entities,
            &self.state.smokes,
            &teams,
        );
        self.state.fog.stamp_firing_reveal_sources_with_smoke(
            &self.state.firing_reveals,
            &self.state.entities,
            &self.state.smokes,
        );
    }

    fn retain_active_visibility_sources(&mut self) -> bool {
        let lingering_before = self.state.lingering_sight.len();
        let firing_before = self.state.firing_reveals.len();
        self.state
            .lingering_sight
            .retain(|source| source.is_active_at(self.state.tick));
        self.state
            .firing_reveals
            .retain(|source| source.is_active_at(self.state.tick));
        lingering_before != self.state.lingering_sight.len()
            || firing_before != self.state.firing_reveals.len()
    }

    pub(crate) fn team_relations(&self) -> teams::TeamRelations {
        teams::TeamRelations::from_player_teams(
            self.state.players.iter().map(|p| (p.id, p.team_id)),
        )
    }

    #[allow(dead_code)]
    pub(crate) fn building_memory_for(
        &self,
        player: u32,
        building: u32,
    ) -> Option<&BuildingMemoryEntry> {
        self.state.building_memory.get(player, building)
    }
}

#[cfg(test)]
mod ability_projection_tests;
#[cfg(test)]
mod lab_god_mode_tests;
#[cfg(test)]
mod phase7_privacy_tests;
#[cfg(test)]
mod snapshot_memory_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod trench_tests;
