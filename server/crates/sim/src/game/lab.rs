//! Authoritative lab mutation API.
//!
//! Lab callers get typed operations with validation at the `Game` seam. This module owns the repair
//! pass so room/client code never reaches into stores, fog, spatial indexes, or economy state.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, Order, OrderIntent, NEUTRAL};
use crate::game::map::{Map, CURRENT_MAP_VERSION};
use crate::game::services::occupancy::{footprint_center, footprint_tiles, Occupancy};
use crate::game::services::{production, standability};
use crate::game::upgrade::UpgradeKind;
use crate::protocol::{terrain, Command, LabMapDraft};
use crate::rules;

use super::{systems, Game, MapMetadata, PlayerInit};

mod checkpoint_scenario;

pub use checkpoint_scenario::{
    LabCheckpointScenarioMap, LabCheckpointScenarioMapData, LabCheckpointScenarioMetadata,
    LabCheckpointScenarioSource, LabCheckpointScenarioV1, LabScenarioTile,
};

pub const LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION: u32 =
    checkpoint_scenario::LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION;
const LAB_MAP_MAIN_PROTECTION_RADIUS_TILES: i32 = 3;
const LAB_MAP_BASE_SITE_PROTECTION_RADIUS_TILES: i32 = 0;
const LAB_MAP_MAX_BASE_SITES: usize = 32;
const LAB_MAX_MUTATION_BATCH: usize = 400;
const LAB_PLACEMENT_SUGGESTION_LIMIT: usize = 8;
const LAB_PLACEMENT_SEARCH_RADIUS_TILES: i32 = 8;
const LAB_PLACEMENT_SEARCH_WORK_LIMIT: usize = 256;

#[derive(Debug, Clone, PartialEq)]
pub enum LabOp {
    SpawnEntities(Vec<LabSpawnEntity>),
    ApplyUpdates(Vec<LabUpdate>),
    DeleteEntities(Vec<u32>),
    SpawnEntity(LabSpawnEntity),
    DeleteEntity { entity_id: u32 },
    MoveEntity(LabMoveEntity),
    SetEntityOwner(LabSetEntityOwner),
    SetPlayerResources(LabSetPlayerResources),
    SetPlayerGodMode { player_id: u32, enabled: bool },
    SetCompletedResearch(LabSetCompletedResearch),
    ApplyMapDraft(LabMapDraft),
    RestoreCheckpointScenario(Box<LabCheckpointScenarioV1>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabSpawnEntity {
    pub owner: u32,
    pub kind: EntityKind,
    pub x: f32,
    pub y: f32,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabMoveEntity {
    pub entity_id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabSetEntityOwner {
    pub entity_id: u32,
    pub owner: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabSetPlayerResources {
    pub player_id: u32,
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LabSetCompletedResearch {
    pub player_id: u32,
    pub upgrade: UpgradeKind,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LabUpdate {
    Move(LabMoveEntity),
    SetEntityOwner(LabSetEntityOwner),
    SetPlayerResources(LabSetPlayerResources),
    SetPlayerGodMode { player_id: u32, enabled: bool },
    SetCompletedResearch(LabSetCompletedResearch),
}

#[derive(Debug, Clone, PartialEq)]
struct LabBatchError {
    failed_index: usize,
    error: LabError,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LabOpOutcome {
    Batch(Vec<LabOpOutcome>),
    Spawned {
        entity_id: u32,
    },
    Deleted {
        entity_id: u32,
    },
    Moved {
        entity_id: u32,
        x: f32,
        y: f32,
    },
    OwnerSet {
        entity_id: u32,
        owner: u32,
    },
    PlayerResourcesSet {
        player_id: u32,
        steel: u32,
        oil: u32,
    },
    PlayerGodModeSet {
        player_id: u32,
        enabled: bool,
    },
    CompletedResearchSet {
        player_id: u32,
        upgrade: UpgradeKind,
        completed: bool,
    },
    MapDraftApplied {
        name: String,
        size: u32,
        battle_reset: bool,
    },
    ScenarioRestored(LabScenarioRestore),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioRestore {
    pub entity_id_map: Vec<LabEntityIdRemap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabEntityIdRemap {
    pub old_id: u32,
    pub new_id: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LabError {
    StaleEntity {
        entity_id: u32,
    },
    InvalidKind {
        kind: String,
        operation: &'static str,
    },
    InvalidPlayer {
        player_id: u32,
    },
    InvalidOwner {
        owner: u32,
    },
    InvalidPosition {
        x: f32,
        y: f32,
        reason: &'static str,
    },
    OccupiedPosition {
        x: f32,
        y: f32,
    },
    Placement {
        x: f32,
        y: f32,
        blockers: Vec<LabPlacementBlocker>,
        suggestions: Vec<(f32, f32)>,
    },
    BatchSize {
        count: usize,
        maximum: usize,
    },
    DuplicateMutation {
        reason: String,
    },
    BatchFailed {
        failed_index: usize,
        error: Box<LabError>,
    },
    InvalidResearch {
        player_id: u32,
        upgrade: String,
    },
    InvalidScenarioVersion {
        version: u32,
    },
    InvalidScenario {
        reason: String,
    },
    InvalidMap {
        name: String,
        reason: String,
    },
    InvalidCommand {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum LabPlacementBlocker {
    Entity {
        entity_id: u32,
        entity_kind: String,
    },
    Terrain {
        tile_x: i32,
        tile_y: i32,
        terrain: String,
    },
    Feature {
        feature: String,
        entity_id: u32,
        entity_kind: String,
    },
    Boundary {
        world_size: u32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LabCommandOptions {
    pub ignore_command_limits: bool,
}

impl Game {
    pub fn new_lab(players: &[PlayerInit], seed: u32, map: Map, map_metadata: MapMetadata) -> Game {
        Self::new_inner_with_map(
            players,
            None,
            seed,
            super::StartingLoadout::Standard,
            None,
            Some(map),
            map_metadata,
        )
    }

    pub fn apply_lab_op(&mut self, op: LabOp) -> Result<LabOpOutcome, LabError> {
        match op {
            LabOp::SpawnEntities(spawns) => self
                .lab_spawn_entities(spawns)
                .map(LabOpOutcome::Batch)
                .map_err(batch_error),
            LabOp::ApplyUpdates(updates) => self
                .lab_apply_updates(updates)
                .map(LabOpOutcome::Batch)
                .map_err(batch_error),
            LabOp::DeleteEntities(entity_ids) => self
                .lab_delete_entities(entity_ids)
                .map(LabOpOutcome::Batch)
                .map_err(batch_error),
            op => {
                let outcome = self.apply_lab_op_without_repair(op)?;
                self.repair_lab_state();
                Ok(outcome)
            }
        }
    }

    /// Export only the authoritative map fields needed by the dedicated Map Editor boundary.
    /// Simulation entities, orders, resources, fog, and timeline state are intentionally absent.
    pub fn export_lab_map(&self) -> LabMapDraft {
        LabMapDraft {
            name: self.map_metadata().name.clone(),
            size: self.state.map.size,
            terrain: self.state.map.terrain.clone(),
            starts: self
                .state
                .map
                .starts
                .iter()
                .map(|&(x, y)| crate::protocol::LabMapTile { x, y })
                .collect(),
            base_sites: self
                .state
                .map
                .base_sites
                .iter()
                .map(|&(x, y)| crate::protocol::LabMapTile { x, y })
                .collect(),
        }
    }

    fn apply_lab_op_without_repair(&mut self, op: LabOp) -> Result<LabOpOutcome, LabError> {
        match op {
            LabOp::SpawnEntities(_) | LabOp::ApplyUpdates(_) | LabOp::DeleteEntities(_) => {
                Err(LabError::InvalidCommand {
                    reason: "nested lab mutation batch is not supported".to_string(),
                })
            }
            LabOp::SpawnEntity(input) => self.lab_spawn_entity(input),
            LabOp::DeleteEntity { entity_id } => self.lab_delete_entity(entity_id),
            LabOp::MoveEntity(input) => self.lab_move_entity(input),
            LabOp::SetEntityOwner(input) => self.lab_set_entity_owner(input),
            LabOp::SetPlayerResources(input) => self.lab_set_player_resources(input),
            LabOp::SetPlayerGodMode { player_id, enabled } => {
                self.lab_set_player_god_mode(player_id, enabled)
            }
            LabOp::SetCompletedResearch(input) => self.lab_set_completed_research(input),
            LabOp::ApplyMapDraft(draft) => self.lab_apply_map_draft(draft),
            LabOp::RestoreCheckpointScenario(scenario) => {
                self.restore_lab_checkpoint_scenario_op(*scenario)
            }
        }
    }

    fn lab_spawn_entities(
        &mut self,
        spawns: Vec<LabSpawnEntity>,
    ) -> Result<Vec<LabOpOutcome>, LabBatchError> {
        validate_batch_size(spawns.len()).map_err(|error| LabBatchError {
            failed_index: spawns.len().saturating_sub(1),
            error,
        })?;
        let mut scratch = self.clone();
        let mut items = Vec::with_capacity(spawns.len());
        for (index, spawn) in spawns.into_iter().enumerate() {
            let outcome = scratch
                .apply_lab_op_without_repair(LabOp::SpawnEntity(spawn))
                .map_err(|error| LabBatchError {
                    failed_index: index,
                    error,
                })?;
            items.push(outcome);
        }
        scratch.repair_lab_state();
        *self = scratch;
        Ok(items)
    }

    fn lab_delete_entities(
        &mut self,
        entity_ids: Vec<u32>,
    ) -> Result<Vec<LabOpOutcome>, LabBatchError> {
        validate_batch_size(entity_ids.len()).map_err(|error| LabBatchError {
            failed_index: entity_ids.len().saturating_sub(1),
            error,
        })?;
        let mut seen = HashSet::new();
        for (index, entity_id) in entity_ids.iter().copied().enumerate() {
            if !seen.insert(entity_id) {
                return Err(LabBatchError {
                    failed_index: index,
                    error: LabError::DuplicateMutation {
                        reason: format!("entity {entity_id} is listed more than once"),
                    },
                });
            }
        }
        let mut scratch = self.clone();
        let mut items = Vec::with_capacity(entity_ids.len());
        for (index, entity_id) in entity_ids.into_iter().enumerate() {
            let outcome = scratch
                .apply_lab_op_without_repair(LabOp::DeleteEntity { entity_id })
                .map_err(|error| LabBatchError {
                    failed_index: index,
                    error,
                })?;
            items.push(outcome);
        }
        scratch.repair_lab_state();
        *self = scratch;
        Ok(items)
    }

    fn lab_apply_updates(
        &mut self,
        updates: Vec<LabUpdate>,
    ) -> Result<Vec<LabOpOutcome>, LabBatchError> {
        validate_batch_size(updates.len()).map_err(|error| LabBatchError {
            failed_index: updates.len().saturating_sub(1),
            error,
        })?;
        validate_update_duplicates(&updates)?;

        let mut scratch = self.clone();
        let mut outcomes = vec![None; updates.len()];
        for (index, update) in updates.iter().copied().enumerate() {
            let op = match update {
                LabUpdate::Move(_) => continue,
                LabUpdate::SetEntityOwner(input) => LabOp::SetEntityOwner(input),
                LabUpdate::SetPlayerResources(input) => LabOp::SetPlayerResources(input),
                LabUpdate::SetPlayerGodMode { player_id, enabled } => {
                    LabOp::SetPlayerGodMode { player_id, enabled }
                }
                LabUpdate::SetCompletedResearch(input) => LabOp::SetCompletedResearch(input),
            };
            outcomes[index] =
                Some(
                    scratch
                        .apply_lab_op_without_repair(op)
                        .map_err(|error| LabBatchError {
                            failed_index: index,
                            error,
                        })?,
                );
        }

        let moved_ids: HashSet<u32> = updates
            .iter()
            .filter_map(|update| match update {
                LabUpdate::Move(input) => Some(input.entity_id),
                _ => None,
            })
            .collect();
        let validation_next_id = scratch.state.entities.checkpoint_next_id();
        let base_entities: Vec<Entity> = scratch
            .state
            .entities
            .iter()
            .filter(|entity| !moved_ids.contains(&entity.id))
            .cloned()
            .collect();
        let mut reserved = Vec::new();
        for (index, update) in updates.iter().copied().enumerate() {
            let LabUpdate::Move(input) = update else {
                continue;
            };
            let entity =
                scratch
                    .state
                    .entities
                    .get(input.entity_id)
                    .cloned()
                    .ok_or(LabBatchError {
                        failed_index: index,
                        error: LabError::StaleEntity {
                            entity_id: input.entity_id,
                        },
                    })?;
            let validation_entities = EntityStore::from_checkpoint_entities(
                validation_next_id,
                base_entities
                    .iter()
                    .chain(reserved.iter())
                    .cloned()
                    .collect(),
            );
            let (x, y) = if entity.is_unit() {
                scratch
                    .validate_unit_position(&validation_entities, entity.kind, input.x, input.y)
                    .map_err(|error| LabBatchError {
                        failed_index: index,
                        error,
                    })?;
                (input.x, input.y)
            } else if entity.is_building() {
                let (_, _, x, y) = scratch
                    .validate_building_position(&validation_entities, entity.kind, input.x, input.y)
                    .map_err(|error| LabBatchError {
                        failed_index: index,
                        error,
                    })?;
                (x, y)
            } else {
                return Err(LabBatchError {
                    failed_index: index,
                    error: invalid_kind(entity.kind, "applyUpdates"),
                });
            };
            let mut reservation = entity;
            reservation.set_position(x, y);
            reserved.push(reservation);
            if let Some(entity) = scratch.state.entities.get_mut(input.entity_id) {
                entity.set_position(x, y);
                entity.clear_orders();
                entity.replace_active_order(Order::Idle);
            }
            scratch.state.entities.release_miner(input.entity_id);
            outcomes[index] = Some(LabOpOutcome::Moved {
                entity_id: input.entity_id,
                x,
                y,
            });
        }

        scratch.repair_lab_state();
        *self = scratch;
        Ok(outcomes.into_iter().flatten().collect())
    }

    fn lab_apply_map_draft(&mut self, draft: LabMapDraft) -> Result<LabOpOutcome, LabError> {
        let name = draft.name.trim();
        if name.is_empty() || name.len() > 80 {
            return Err(LabError::InvalidMap {
                name: draft.name,
                reason: "name must contain 1 to 80 bytes".to_string(),
            });
        }
        if draft.size != self.state.map.size {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!(
                    "proof-of-concept map size must remain {}; got {}",
                    self.state.map.size, draft.size
                ),
            });
        }
        let tile_count = draft
            .size
            .checked_mul(draft.size)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| LabError::InvalidMap {
                name: name.to_string(),
                reason: "terrain dimensions overflow".to_string(),
            })?;
        if draft.terrain.len() != tile_count {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!(
                    "terrain has {} tiles; expected {tile_count}",
                    draft.terrain.len()
                ),
            });
        }
        if draft.terrain.iter().any(|tile| {
            !matches!(
                *tile,
                terrain::GRASS
                    | terrain::ROCK
                    | terrain::WATER
                    | terrain::ROAD_BARE
                    | terrain::ROAD_HORIZONTAL
                    | terrain::ROAD_VERTICAL
                    | terrain::ROAD_DIAGONAL_NW_SE
                    | terrain::ROAD_DIAGONAL_NE_SW
            )
        }) {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: "terrain contains an unknown code".to_string(),
            });
        }
        let players = self.player_inits();
        if draft.starts.len() != players.len() {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!(
                    "map has {} starts; this lab needs {}",
                    draft.starts.len(),
                    players.len()
                ),
            });
        }
        if draft.base_sites.len() > LAB_MAP_MAX_BASE_SITES {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!("map has more than {LAB_MAP_MAX_BASE_SITES} base sites"),
            });
        }

        let starts: Vec<_> = draft.starts.iter().map(|tile| (tile.x, tile.y)).collect();
        let base_sites: Vec<_> = draft
            .base_sites
            .iter()
            .map(|tile| (tile.x, tile.y))
            .collect();
        let mut occupied_sites = std::collections::HashSet::new();
        for &(x, y) in &starts {
            validate_lab_map_site(
                name,
                draft.size,
                &draft.terrain,
                x,
                y,
                LAB_MAP_MAIN_PROTECTION_RADIUS_TILES,
                &mut occupied_sites,
            )?;
        }
        for &(x, y) in &base_sites {
            if starts.contains(&(x, y)) {
                continue;
            }
            validate_lab_map_site(
                name,
                draft.size,
                &draft.terrain,
                x,
                y,
                LAB_MAP_BASE_SITE_PROTECTION_RADIUS_TILES,
                &mut occupied_sites,
            )?;
        }

        let map = Map {
            size: draft.size,
            terrain: draft.terrain,
            starts,
            base_sites,
        };
        let map_metadata = MapMetadata {
            name: name.to_string(),
            schema_version: CURRENT_MAP_VERSION,
            content_hash: format!("lab-draft-{}", map.materialized_hash()),
        };
        let seed = self.seed();
        let god_mode_players = self.lab_god_mode_players();
        let mut replacement = Self::new_lab(&players, seed, map, map_metadata);
        for player_id in god_mode_players {
            replacement.lab_set_player_god_mode(player_id, true)?;
        }
        *self = replacement;
        Ok(LabOpOutcome::MapDraftApplied {
            name: name.to_string(),
            size: draft.size,
            battle_reset: true,
        })
    }

    pub fn issue_lab_command_as(
        &mut self,
        player_id: u32,
        command: Command,
        options: LabCommandOptions,
    ) -> Result<(), LabError> {
        self.validate_owner(player_id)?;
        let authority_entities = command_authority_entities(&command);
        if authority_entities.is_empty() {
            return Err(LabError::InvalidCommand {
                reason: "command must identify at least one owned entity".to_string(),
            });
        }
        for entity_id in authority_entities {
            let Some(entity) = self.state.entities.get(entity_id) else {
                return Err(LabError::StaleEntity { entity_id });
            };
            if entity.owner != player_id {
                return Err(LabError::InvalidCommand {
                    reason: format!(
                        "entity {entity_id} is owned by {}, not {player_id}",
                        entity.owner
                    ),
                });
            }
        }
        let command = super::command::SimCommand::from_protocol(command);
        if options.ignore_command_limits {
            self.enqueue_lab_command_ignoring_limits(player_id, command);
        } else {
            self.enqueue(player_id, command);
        }
        Ok(())
    }

    pub fn lab_god_mode_players(&self) -> Vec<u32> {
        self.state.lab_god_mode_players.iter().copied().collect()
    }

    pub fn lab_owned_unit_ids(&self, player_id: u32) -> Result<Vec<u32>, LabError> {
        self.validate_owner(player_id)?;
        Ok(self
            .state
            .entities
            .iter()
            .filter(|entity| entity.owner == player_id && entity.is_unit() && entity.hp > 0)
            .map(|entity| entity.id)
            .collect())
    }

    pub fn restore_lab_checkpoint_scenario_op(
        &mut self,
        scenario: LabCheckpointScenarioV1,
    ) -> Result<LabOpOutcome, LabError> {
        let entity_id_map = scenario.metadata.source_entity_id_map.clone();
        let restored = Self::restore_lab_checkpoint_scenario(scenario)?;
        *self = restored;
        Ok(LabOpOutcome::ScenarioRestored(LabScenarioRestore {
            entity_id_map,
        }))
    }

    fn lab_spawn_entity(&mut self, input: LabSpawnEntity) -> Result<LabOpOutcome, LabError> {
        self.validate_owner(input.owner)?;
        let id = if input.kind.is_unit() {
            self.validate_unit_position(&self.state.entities, input.kind, input.x, input.y)?;
            self.state
                .entities
                .spawn_unit(input.owner, input.kind, input.x, input.y)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else if input.kind.is_building() {
            let (_, _, x, y) = self.validate_building_position(
                &self.state.entities,
                input.kind,
                input.x,
                input.y,
            )?;
            self.state
                .entities
                .spawn_building(input.owner, input.kind, x, y, input.completed)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else {
            return Err(invalid_kind(input.kind, "spawnEntity"));
        };
        Ok(LabOpOutcome::Spawned { entity_id: id })
    }

    fn lab_delete_entity(&mut self, entity_id: u32) -> Result<LabOpOutcome, LabError> {
        self.state
            .entities
            .remove(entity_id)
            .ok_or(LabError::StaleEntity { entity_id })?;
        self.state.entities.release_miner(entity_id);
        self.cleanup_entity_references(entity_id);
        Ok(LabOpOutcome::Deleted { entity_id })
    }

    fn lab_move_entity(&mut self, input: LabMoveEntity) -> Result<LabOpOutcome, LabError> {
        let (kind, is_unit, is_building) = {
            let entity = self
                .state
                .entities
                .get(input.entity_id)
                .ok_or(LabError::StaleEntity {
                    entity_id: input.entity_id,
                })?;
            (entity.kind, entity.is_unit(), entity.is_building())
        };
        let mut entities_without = self.state.entities.clone();
        entities_without.remove(input.entity_id);
        let (x, y) = if is_unit {
            self.validate_unit_position(&entities_without, kind, input.x, input.y)?;
            (input.x, input.y)
        } else if is_building {
            let (_, _, x, y) =
                self.validate_building_position(&entities_without, kind, input.x, input.y)?;
            (x, y)
        } else {
            return Err(invalid_kind(kind, "moveEntity"));
        };

        if let Some(entity) = self.state.entities.get_mut(input.entity_id) {
            entity.set_position(x, y);
            entity.clear_orders();
            entity.replace_active_order(Order::Idle);
        }
        self.state.entities.release_miner(input.entity_id);
        Ok(LabOpOutcome::Moved {
            entity_id: input.entity_id,
            x,
            y,
        })
    }

    fn lab_set_entity_owner(&mut self, input: LabSetEntityOwner) -> Result<LabOpOutcome, LabError> {
        self.validate_owner(input.owner)?;
        let kind = self
            .state
            .entities
            .get(input.entity_id)
            .ok_or(LabError::StaleEntity {
                entity_id: input.entity_id,
            })?
            .kind;
        if !kind.is_unit() && !kind.is_building() {
            return Err(invalid_kind(kind, "setEntityOwner"));
        }

        if let Some(entity) = self.state.entities.get_mut(input.entity_id) {
            entity.owner = input.owner;
            entity.clear_orders();
            clear_lab_production_state(entity);
        }
        self.state.entities.release_miner(input.entity_id);
        self.cleanup_entity_references(input.entity_id);
        Ok(LabOpOutcome::OwnerSet {
            entity_id: input.entity_id,
            owner: input.owner,
        })
    }

    fn lab_set_player_resources(
        &mut self,
        input: LabSetPlayerResources,
    ) -> Result<LabOpOutcome, LabError> {
        let player = self
            .state
            .players
            .iter_mut()
            .find(|player| player.id == input.player_id)
            .ok_or(LabError::InvalidPlayer {
                player_id: input.player_id,
            })?;
        player.steel = input.steel;
        player.oil = input.oil;
        Ok(LabOpOutcome::PlayerResourcesSet {
            player_id: input.player_id,
            steel: input.steel,
            oil: input.oil,
        })
    }

    fn lab_set_player_god_mode(
        &mut self,
        player_id: u32,
        enabled: bool,
    ) -> Result<LabOpOutcome, LabError> {
        self.validate_player(player_id)?;
        if enabled {
            self.state.lab_god_mode_players.insert(player_id);
        } else {
            self.state.lab_god_mode_players.remove(&player_id);
        }
        self.sync_lab_god_mode_flags();
        Ok(LabOpOutcome::PlayerGodModeSet { player_id, enabled })
    }

    fn lab_set_completed_research(
        &mut self,
        input: LabSetCompletedResearch,
    ) -> Result<LabOpOutcome, LabError> {
        let player = self
            .state
            .players
            .iter_mut()
            .find(|player| player.id == input.player_id)
            .ok_or(LabError::InvalidPlayer {
                player_id: input.player_id,
            })?;
        validate_upgrade_for_player(player, input.upgrade)?;
        if input.completed {
            player.upgrades.insert(input.upgrade);
        } else {
            player.upgrades.remove(&input.upgrade);
        }
        production::sync_owned_autocast_from_upgrades(
            &mut self.state.entities,
            input.player_id,
            &player.upgrades,
        );
        Ok(LabOpOutcome::CompletedResearchSet {
            player_id: input.player_id,
            upgrade: input.upgrade,
            completed: input.completed,
        })
    }

    fn validate_owner(&self, owner: u32) -> Result<(), LabError> {
        if owner == NEUTRAL {
            return Err(LabError::InvalidOwner { owner });
        }
        self.validate_player(owner)
            .map_err(|_| LabError::InvalidOwner { owner })
    }

    fn validate_player(&self, player_id: u32) -> Result<(), LabError> {
        if self
            .state
            .players
            .iter()
            .any(|player| player.id == player_id)
        {
            Ok(())
        } else {
            Err(LabError::InvalidPlayer { player_id })
        }
    }

    fn validate_unit_position(
        &self,
        entities: &EntityStore,
        kind: EntityKind,
        x: f32,
        y: f32,
    ) -> Result<(), LabError> {
        if unit_position_valid(&self.state.map, entities, kind, x, y) {
            return Ok(());
        }
        Err(placement_error(
            &self.state.map,
            entities,
            kind,
            x,
            y,
            false,
        ))
    }

    fn validate_building_position(
        &self,
        entities: &EntityStore,
        kind: EntityKind,
        x: f32,
        y: f32,
    ) -> Result<(u32, u32, f32, f32), LabError> {
        if let Some(result) = building_position_if_valid(&self.state.map, entities, kind, x, y) {
            return Ok(result);
        }
        Err(placement_error(&self.state.map, entities, kind, x, y, true))
    }

    fn cleanup_entity_references(&mut self, entity_id: u32) {
        for id in self.state.entities.ids() {
            let Some(entity) = self.state.entities.get_mut(id) else {
                continue;
            };
            if entity.target_id() == Some(entity_id) {
                entity.set_target_id(None);
            }
            if order_references_entity(&entity.order(), entity_id) {
                entity.clear_orders();
            }
            if let Some(movement) = entity.movement.as_mut() {
                movement
                    .queued_orders
                    .retain(|intent| !order_intent_references_entity(intent, entity_id));
            }
        }
        self.state.entities.clear_stale_miner_slots();
    }

    fn repair_lab_state(&mut self) {
        self.state.entities.clear_stale_miner_slots();
        self.sync_lab_god_mode_flags();
        self.repair_mortar_autocast_state();
        systems::recompute_supply(&mut self.state.players, &self.state.entities);
        self.reset_derived_state();
        let ids = self.state.player_ids();
        self.state.fog.recompute_with_smoke(
            &ids,
            &self.state.entities,
            &self.state.map,
            &self.state.smokes,
        );
        self.refresh_building_memory(&ids);
        self.refresh_trench_memory(&ids);
        #[cfg(debug_assertions)]
        self.assert_invariants();
    }

    fn repair_mortar_autocast_state(&mut self) {
        for player in &self.state.players {
            production::sync_owned_autocast_from_upgrades(
                &mut self.state.entities,
                player.id,
                &player.upgrades,
            );
        }
    }

    pub(crate) fn sync_lab_god_mode_flags(&mut self) {
        let enabled_players = self.state.lab_god_mode_players.clone();
        for entity_id in self.state.entities.ids() {
            if let Some(entity) = self.state.entities.get_mut(entity_id) {
                let is_player_asset = entity.is_unit() || entity.is_building();
                entity.set_invulnerable(is_player_asset && enabled_players.contains(&entity.owner));
            }
        }
    }
}

fn validate_batch_size(count: usize) -> Result<(), LabError> {
    if (1..=LAB_MAX_MUTATION_BATCH).contains(&count) {
        Ok(())
    } else {
        Err(LabError::BatchSize {
            count,
            maximum: LAB_MAX_MUTATION_BATCH,
        })
    }
}

fn batch_error(error: LabBatchError) -> LabError {
    LabError::BatchFailed {
        failed_index: error.failed_index,
        error: Box::new(error.error),
    }
}

fn validate_update_duplicates(updates: &[LabUpdate]) -> Result<(), LabBatchError> {
    let mut entities = HashSet::new();
    let mut player_fields = HashSet::new();
    for (index, update) in updates.iter().enumerate() {
        let duplicate = match update {
            LabUpdate::Move(input) => !entities.insert(input.entity_id),
            LabUpdate::SetEntityOwner(input) => !entities.insert(input.entity_id),
            LabUpdate::SetPlayerResources(input) => {
                !player_fields.insert((input.player_id, "resources".to_string()))
            }
            LabUpdate::SetPlayerGodMode { player_id, .. } => {
                !player_fields.insert((*player_id, "godMode".to_string()))
            }
            LabUpdate::SetCompletedResearch(input) => !player_fields.insert((
                input.player_id,
                format!("research:{}", input.upgrade.to_protocol_str()),
            )),
        };
        if duplicate {
            return Err(LabBatchError {
                failed_index: index,
                error: LabError::DuplicateMutation {
                    reason:
                        "batch contains more than one update for the same entity or player field"
                            .to_string(),
                },
            });
        }
    }
    Ok(())
}

fn unit_position_valid(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> bool {
    if validate_world_position(map, x, y).is_err() {
        return false;
    }
    let occ = Occupancy::build(map, entities);
    standability::unit_spawn_standable(map, &occ, entities, kind, x, y)
}

fn building_position_if_valid(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> Option<(u32, u32, f32, f32)> {
    validate_world_position(map, x, y).ok()?;
    let (tile_x, tile_y, center_x, center_y) =
        building_top_left_for_center(map, kind, x, y).ok()?;
    if footprint_tiles(kind, tile_x, tile_y)
        .into_iter()
        .any(|(tx, ty)| {
            !map.in_bounds(tx as i32, ty as i32) || !map.is_passable(tx as i32, ty as i32)
        })
    {
        return None;
    }
    standability::building_site_clear(map, entities, kind, tile_x, tile_y)
        .then_some((tile_x, tile_y, center_x, center_y))
}

fn placement_error(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
    building: bool,
) -> LabError {
    let blockers = placement_blockers(map, entities, kind, x, y, building);
    let suggestions = placement_suggestions(map, entities, kind, x, y, building);
    LabError::Placement {
        x,
        y,
        blockers,
        suggestions,
    }
}

fn placement_blockers(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
    building: bool,
) -> Vec<LabPlacementBlocker> {
    let mut blockers = Vec::new();
    let world_size = map.size.saturating_mul(config::TILE_SIZE);
    if !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || y < 0.0
        || x >= world_size as f32
        || y >= world_size as f32
    {
        blockers.push(LabPlacementBlocker::Boundary { world_size });
    }

    let tile_size = config::TILE_SIZE as f32;
    let (min_tx, max_tx, min_ty, max_ty) = if building {
        match building_top_left_for_center(map, kind, x, y) {
            Ok((tile_x, tile_y, _, _)) => {
                let tiles = footprint_tiles(kind, tile_x, tile_y);
                let max_x = tiles
                    .iter()
                    .map(|(tx, _)| *tx as i32)
                    .max()
                    .unwrap_or(tile_x as i32);
                let max_y = tiles
                    .iter()
                    .map(|(_, ty)| *ty as i32)
                    .max()
                    .unwrap_or(tile_y as i32);
                (tile_x as i32, max_x, tile_y as i32, max_y)
            }
            Err(_) => {
                let tx = (x / tile_size).floor() as i32;
                let ty = (y / tile_size).floor() as i32;
                (tx, tx, ty, ty)
            }
        }
    } else {
        let radius = config::unit_stats(kind)
            .map(|stats| stats.radius)
            .unwrap_or(tile_size / 2.0);
        (
            ((x - radius) / tile_size).floor() as i32,
            ((x + radius) / tile_size).floor() as i32,
            ((y - radius) / tile_size).floor() as i32,
            ((y + radius) / tile_size).floor() as i32,
        )
    };
    for ty in min_ty..=max_ty {
        for tx in min_tx..=max_tx {
            if !map.in_bounds(tx, ty) {
                if !blockers
                    .iter()
                    .any(|blocker| matches!(blocker, LabPlacementBlocker::Boundary { .. }))
                {
                    blockers.push(LabPlacementBlocker::Boundary { world_size });
                }
            } else if !map.is_passable(tx, ty) {
                let terrain = map
                    .terrain
                    .get(ty as usize * map.size as usize + tx as usize)
                    .copied()
                    .map(terrain_name)
                    .unwrap_or("unknown")
                    .to_string();
                blockers.push(LabPlacementBlocker::Terrain {
                    tile_x: tx,
                    tile_y: ty,
                    terrain,
                });
            }
        }
    }

    let candidate_radius = if building {
        config::building_stats(kind)
            .map(|stats| stats.foot_w.max(stats.foot_h) as f32 * tile_size * 0.5)
            .unwrap_or(tile_size)
    } else {
        config::unit_stats(kind)
            .map(|stats| stats.radius)
            .unwrap_or(tile_size / 2.0)
    };
    for entity in entities.iter().filter(|entity| entity.hp > 0) {
        let dx = entity.pos_x - x;
        let dy = entity.pos_y - y;
        if dx * dx + dy * dy >= (candidate_radius + entity.radius()).powi(2) {
            continue;
        }
        let blocker = if entity.is_building() || entity.is_node() {
            LabPlacementBlocker::Feature {
                feature: if entity.is_node() {
                    "resource"
                } else {
                    "building"
                }
                .to_string(),
                entity_id: entity.id,
                entity_kind: entity.kind.to_string(),
            }
        } else {
            LabPlacementBlocker::Entity {
                entity_id: entity.id,
                entity_kind: entity.kind.to_string(),
            }
        };
        blockers.push(blocker);
    }
    blockers.truncate(32);
    blockers
}

fn placement_suggestions(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
    building: bool,
) -> Vec<(f32, f32)> {
    let tile_size = config::TILE_SIZE as f32;
    let mut suggestions = Vec::new();
    let mut work = 0usize;
    for radius in 1..=LAB_PLACEMENT_SEARCH_RADIUS_TILES {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue;
                }
                work += 1;
                if work > LAB_PLACEMENT_SEARCH_WORK_LIMIT {
                    return suggestions;
                }
                let candidate_x = x + dx as f32 * tile_size;
                let candidate_y = y + dy as f32 * tile_size;
                let point = if building {
                    let Some((_, _, snapped_x, snapped_y)) =
                        building_position_if_valid(map, entities, kind, candidate_x, candidate_y)
                    else {
                        continue;
                    };
                    (snapped_x, snapped_y)
                } else {
                    if !unit_position_valid(map, entities, kind, candidate_x, candidate_y) {
                        continue;
                    }
                    (candidate_x, candidate_y)
                };
                if !suggestions.contains(&point) {
                    suggestions.push(point);
                }
                if suggestions.len() == LAB_PLACEMENT_SUGGESTION_LIMIT {
                    return suggestions;
                }
            }
        }
    }
    suggestions
}

fn terrain_name(tile: u8) -> &'static str {
    match tile {
        terrain::GRASS => "grass",
        terrain::ROCK => "rock",
        terrain::WATER => "water",
        terrain::ROAD_BARE => "road-bare",
        terrain::ROAD_HORIZONTAL => "road-horizontal",
        terrain::ROAD_VERTICAL => "road-vertical",
        terrain::ROAD_DIAGONAL_NW_SE => "road-diagonal-nw-se",
        terrain::ROAD_DIAGONAL_NE_SW => "road-diagonal-ne-sw",
        _ => "unknown",
    }
}

fn validate_lab_map_site(
    name: &str,
    size: u32,
    terrain_grid: &[u8],
    x: u32,
    y: u32,
    radius: i32,
    occupied_sites: &mut std::collections::HashSet<(u32, u32)>,
) -> Result<(), LabError> {
    if x >= size || y >= size {
        return Err(LabError::InvalidMap {
            name: name.to_string(),
            reason: format!("base site ({x},{y}) is outside the map"),
        });
    }
    if !occupied_sites.insert((x, y)) {
        return Err(LabError::InvalidMap {
            name: name.to_string(),
            reason: format!("more than one base site uses ({x},{y})"),
        });
    }
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let Some(tx) = i32::try_from(x)
                .ok()
                .and_then(|value| value.checked_add(dx))
            else {
                return Err(LabError::InvalidMap {
                    name: name.to_string(),
                    reason: format!("base site ({x},{y}) is too close to the map edge"),
                });
            };
            let Some(ty) = i32::try_from(y)
                .ok()
                .and_then(|value| value.checked_add(dy))
            else {
                return Err(LabError::InvalidMap {
                    name: name.to_string(),
                    reason: format!("base site ({x},{y}) is too close to the map edge"),
                });
            };
            if tx < 0 || ty < 0 || tx >= size as i32 || ty >= size as i32 {
                return Err(LabError::InvalidMap {
                    name: name.to_string(),
                    reason: format!("base site ({x},{y}) is too close to the map edge"),
                });
            }
            let index = ty as usize * size as usize + tx as usize;
            if !terrain_grid
                .get(index)
                .copied()
                .is_some_and(crate::rules::terrain::is_passable_map_code)
            {
                return Err(LabError::InvalidMap {
                    name: name.to_string(),
                    reason: format!(
                        "base site ({x},{y}) needs passable terrain throughout its protected area"
                    ),
                });
            }
        }
    }
    Ok(())
}

fn command_authority_entities(command: &Command) -> Vec<u32> {
    match command {
        Command::Move { units, .. }
        | Command::AttackMove { units, .. }
        | Command::Attack { units, .. }
        | Command::SetupAntiTankGuns { units, .. }
        | Command::TearDownAntiTankGuns { units }
        | Command::Charge { units }
        | Command::UseAbility { units, .. }
        | Command::RecastAbility { units, .. }
        | Command::SetAutocast { units, .. }
        | Command::Gather { units, .. }
        | Command::Deconstruct { units, .. }
        | Command::Build { units, .. }
        | Command::Stop { units }
        | Command::HoldPosition { units, .. } => units.clone(),
        Command::AdjustProductionRepeat { buildings, .. } => buildings.clone(),
        Command::Train { building, .. }
        | Command::Research { building, .. }
        | Command::Cancel { building }
        | Command::SetRally { building, .. } => vec![*building],
    }
}

fn invalid_kind(kind: EntityKind, operation: &'static str) -> LabError {
    LabError::InvalidKind {
        kind: kind.to_string(),
        operation,
    }
}

fn validate_world_position(map: &Map, x: f32, y: f32) -> Result<(), LabError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "position must be finite",
        });
    }
    let world = map.world_size_px();
    if x < 0.0 || y < 0.0 || x >= world || y >= world {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "position is outside the map",
        });
    }
    Ok(())
}

fn building_top_left_for_center(
    map: &Map,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> Result<(u32, u32, f32, f32), LabError> {
    let stats =
        config::building_stats(kind).ok_or_else(|| invalid_kind(kind, "buildingPosition"))?;
    let (center_tile_x, center_tile_y) = map.tile_of(x, y);
    let offset_x = stats.foot_w / 2;
    let offset_y = stats.foot_h / 2;
    let Some(tile_x) = center_tile_x.checked_sub(offset_x) else {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "building footprint crosses the west map edge",
        });
    };
    let Some(tile_y) = center_tile_y.checked_sub(offset_y) else {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "building footprint crosses the north map edge",
        });
    };
    let (center_x, center_y) = footprint_center(map, kind, tile_x, tile_y);
    Ok((tile_x, tile_y, center_x, center_y))
}

fn clear_lab_production_state(entity: &mut Entity) {
    if let Some(production) = entity.production.as_mut() {
        production.queue.clear();
        production.research_queue.clear();
        production.rally_point = None;
        production.rally_queue.clear();
    }
}

fn order_references_entity(order: &Order, entity_id: u32) -> bool {
    order.attack_target() == Some(entity_id)
        || order.gather_node() == Some(entity_id)
        || order.build_site() == Some(entity_id)
        || order.deconstruct_target() == Some(entity_id)
}

fn order_intent_references_entity(intent: &OrderIntent, entity_id: u32) -> bool {
    match intent {
        OrderIntent::Attack(target) => target.target == entity_id,
        OrderIntent::Gather(gather) => gather.node == entity_id,
        OrderIntent::Deconstruct(target) => target.target == entity_id,
        _ => false,
    }
}

fn validate_upgrade_for_player(
    player: &super::PlayerState,
    upgrade: UpgradeKind,
) -> Result<(), LabError> {
    let upgrade_id = upgrade.to_protocol_str();
    let allowed = rules::faction::catalog_for(&player.faction_id)
        .is_some_and(|catalog| catalog.upgrades.iter().any(|entry| entry.id == upgrade_id));
    if allowed {
        Ok(())
    } else {
        Err(LabError::InvalidResearch {
            player_id: player.id,
            upgrade: upgrade_id.to_string(),
        })
    }
}

#[cfg(test)]
mod tests;
