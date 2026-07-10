//! Authoritative lab mutation API.
//!
//! Lab callers get typed operations with validation at the `Game` seam. This module owns the repair
//! pass so room/client code never reaches into stores, fog, spatial indexes, or economy state.

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
mod terrain_edit;

pub use checkpoint_scenario::{
    LabCheckpointScenarioMap, LabCheckpointScenarioMapData, LabCheckpointScenarioMetadata,
    LabCheckpointScenarioSource, LabCheckpointScenarioV1, LabScenarioTile,
};

pub const LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION: u32 =
    checkpoint_scenario::LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION;
const LAB_MAP_MAIN_PROTECTION_RADIUS_TILES: i32 = 3;
const LAB_MAP_EXPANSION_PROTECTION_RADIUS_TILES: i32 = 0;

#[derive(Debug, Clone, PartialEq)]
pub enum LabOp {
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

#[derive(Debug, Clone, PartialEq)]
pub enum LabOpOutcome {
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
        if draft
            .terrain
            .iter()
            .any(|tile| !matches!(*tile, terrain::GRASS | terrain::ROCK | terrain::WATER))
        {
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
        if draft.expansion_sites.len() > players.len().saturating_mul(3) {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: "map has more than three natural sites per player".to_string(),
            });
        }

        let starts: Vec<_> = draft.starts.iter().map(|tile| (tile.x, tile.y)).collect();
        let expansion_sites: Vec<_> = draft
            .expansion_sites
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
        for &(x, y) in &expansion_sites {
            validate_lab_map_site(
                name,
                draft.size,
                &draft.terrain,
                x,
                y,
                LAB_MAP_EXPANSION_PROTECTION_RADIUS_TILES,
                &mut occupied_sites,
            )?;
        }

        let map = Map {
            size: draft.size,
            terrain: draft.terrain,
            starts,
            expansion_sites,
        };
        let map_metadata = MapMetadata {
            name: name.to_string(),
            schema_version: CURRENT_MAP_VERSION,
            content_hash: format!("lab-draft-{}", map.materialized_hash()),
        };
        let battle_reset = self.state.map.starts != map.starts
            || self.state.map.expansion_sites != map.expansion_sites;
        if !battle_reset {
            let previous_terrain = std::mem::replace(&mut self.state.map.terrain, map.terrain);
            let previous_metadata = std::mem::replace(&mut self.state.map_metadata, map_metadata);
            if let Err(error) = terrain_edit::relocate_blocked_units(
                &self.state.map,
                &mut self.state.entities,
                &self.state.map_metadata.name,
            ) {
                self.state.map.terrain = previous_terrain;
                self.state.map_metadata = previous_metadata;
                return Err(error);
            }
            self.repair_lab_state();
            return Ok(LabOpOutcome::MapDraftApplied {
                name: name.to_string(),
                size: draft.size,
                battle_reset: false,
            });
        }

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
        self.repair_lab_state();
        Ok(LabOpOutcome::Spawned { entity_id: id })
    }

    fn lab_delete_entity(&mut self, entity_id: u32) -> Result<LabOpOutcome, LabError> {
        self.state
            .entities
            .remove(entity_id)
            .ok_or(LabError::StaleEntity { entity_id })?;
        self.state.entities.release_miner(entity_id);
        self.cleanup_entity_references(entity_id);
        self.repair_lab_state();
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
        self.repair_lab_state();
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
        self.repair_lab_state();
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
        validate_world_position(&self.state.map, x, y)?;
        let occ = Occupancy::build(&self.state.map, entities);
        if !standability::unit_spawn_standable(&self.state.map, &occ, entities, kind, x, y) {
            return Err(LabError::OccupiedPosition { x, y });
        }
        Ok(())
    }

    fn validate_building_position(
        &self,
        entities: &EntityStore,
        kind: EntityKind,
        x: f32,
        y: f32,
    ) -> Result<(u32, u32, f32, f32), LabError> {
        validate_world_position(&self.state.map, x, y)?;
        let (tile_x, tile_y, center_x, center_y) =
            building_top_left_for_center(&self.state.map, kind, x, y)?;
        for (tx, ty) in footprint_tiles(kind, tile_x, tile_y) {
            if !self.state.map.in_bounds(tx as i32, ty as i32)
                || !self.state.map.is_passable(tx as i32, ty as i32)
            {
                return Err(LabError::InvalidPosition {
                    x,
                    y,
                    reason: "building footprint is out of bounds or on blocked terrain",
                });
            }
        }
        if !standability::building_site_clear(&self.state.map, entities, kind, tile_x, tile_y) {
            return Err(LabError::OccupiedPosition { x, y });
        }
        Ok((tile_x, tile_y, center_x, center_y))
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
            if terrain_grid.get(index).copied() != Some(terrain::GRASS) {
                return Err(LabError::InvalidMap {
                    name: name.to_string(),
                    reason: format!(
                        "base site ({x},{y}) needs grass throughout its protected area"
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
        | Command::HoldPosition { units } => units.clone(),
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
mod tests {
    use super::*;
    use crate::game::entity::WeaponSetup;
    use crate::game::services::occupancy::footprint_center;
    use crate::protocol::{terrain, LabMapTile};

    fn lab_players() -> [PlayerInit; 2] {
        [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "Alpha".to_string(),
                color: "#4878c8".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Bravo".to_string(),
                color: "#c84848".to_string(),
                is_ai: false,
            },
        ]
    }

    fn lab_metadata() -> MapMetadata {
        MapMetadata {
            name: "Default".to_string(),
            schema_version: crate::game::map::CURRENT_MAP_VERSION,
            content_hash: "test-map".to_string(),
        }
    }

    fn flat_lab_map() -> Map {
        const SIZE: u32 = 64;
        Map {
            size: SIZE,
            terrain: vec![terrain::GRASS; (SIZE * SIZE) as usize],
            starts: vec![(16, 16), (48, 48)],
            expansion_sites: Vec::new(),
        }
    }

    fn new_game() -> Game {
        Game::new_lab(&lab_players(), 0xABCD, flat_lab_map(), lab_metadata())
    }

    fn map_draft() -> LabMapDraft {
        let mut terrain = vec![terrain::GRASS; 64 * 64];
        terrain[0] = terrain::WATER;
        LabMapDraft {
            name: "Edited Lab Map".to_string(),
            size: 64,
            terrain,
            starts: vec![LabMapTile { x: 12, y: 12 }, LabMapTile { x: 51, y: 51 }],
            expansion_sites: vec![LabMapTile { x: 32, y: 32 }],
        }
    }

    #[test]
    fn lab_map_draft_rebuilds_the_battle_on_authoritative_terrain_and_bases() {
        let mut game = new_game();
        for _ in 0..10 {
            game.tick();
        }

        let outcome = game
            .apply_lab_op(LabOp::ApplyMapDraft(map_draft()))
            .expect("valid lab map draft");

        assert_eq!(
            outcome,
            LabOpOutcome::MapDraftApplied {
                name: "Edited Lab Map".to_string(),
                size: 64,
                battle_reset: true,
            }
        );
        assert_eq!(game.tick_count(), 0);
        assert_eq!(game.state.map.terrain[0], terrain::WATER);
        assert_eq!(game.state.map.starts, vec![(12, 12), (51, 51)]);
        assert_eq!(game.state.map.expansion_sites, vec![(32, 32)]);
        assert_eq!(game.state.map_metadata.name, "Edited Lab Map");
        assert_eq!(
            game.start_payload()
                .players
                .iter()
                .map(|player| (player.start_tile_x, player.start_tile_y))
                .collect::<Vec<_>>(),
            vec![(12, 12), (51, 51)]
        );
    }

    #[test]
    fn lab_map_draft_rejects_blocked_base_protection_area() {
        let mut game = new_game();
        let mut draft = map_draft();
        draft.terrain[12 * 64 + 12] = terrain::ROCK;

        assert!(matches!(
            game.apply_lab_op(LabOp::ApplyMapDraft(draft)),
            Err(LabError::InvalidMap { reason, .. })
                if reason.contains("protected area")
        ));
    }

    #[test]
    fn lab_map_draft_allows_terrain_immediately_beyond_starting_unit_area() {
        let mut game = new_game();
        let mut draft = map_draft();
        draft.terrain[12 * 64 + 16] = terrain::ROCK;

        game.apply_lab_op(LabOp::ApplyMapDraft(draft))
            .expect("terrain beyond the starting unit area should remain editable");
        assert_eq!(game.state.map.terrain[12 * 64 + 16], terrain::ROCK);
    }

    #[test]
    fn terrain_only_lab_map_draft_preserves_tick_and_moved_worker() {
        let mut game = new_game();
        let worker_id = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
            .map(|entity| entity.id)
            .expect("starting worker");
        let (worker_x, worker_y) = game.state.map.tile_center(30, 30);
        game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
            entity_id: worker_id,
            x: worker_x,
            y: worker_y,
        }))
        .expect("move worker to map center");
        for _ in 0..10 {
            game.tick();
        }
        let tick = game.tick_count();
        let mut terrain = game.state.map.terrain.clone();
        for y in 29..=31 {
            for x in 30..=32 {
                terrain[y * 64 + x] = terrain::ROCK;
            }
        }
        let draft = LabMapDraft {
            name: "Terrain-only edit".to_string(),
            size: 64,
            terrain,
            starts: game
                .state
                .map
                .starts
                .iter()
                .map(|&(x, y)| LabMapTile { x, y })
                .collect(),
            expansion_sites: Vec::new(),
        };

        let outcome = game
            .apply_lab_op(LabOp::ApplyMapDraft(draft))
            .expect("terrain-only edit");

        assert_eq!(
            outcome,
            LabOpOutcome::MapDraftApplied {
                name: "Terrain-only edit".to_string(),
                size: 64,
                battle_reset: false,
            }
        );
        assert_eq!(game.tick_count(), tick);
        assert_eq!(game.state.map.terrain[30 * 64 + 31], terrain::ROCK);
        let worker = game
            .state
            .entities
            .get(worker_id)
            .expect("moved worker remains");
        assert_ne!((worker.pos_x, worker.pos_y), (worker_x, worker_y));
        assert!(worker.pos_x > 20.0 * config::TILE_SIZE as f32);
        assert!(worker.pos_y > 20.0 * config::TILE_SIZE as f32);
    }

    fn default_map_game() -> Game {
        let players = lab_players();
        let start_players: Vec<_> = players
            .iter()
            .map(|player| (player.id, player.team_id))
            .collect();
        let map =
            Map::load_for_players("Default", &start_players, 0xABCD).expect("default lab map");
        let metadata = Map::metadata_for_name("Default").expect("default map metadata");
        Game::new_lab(&players, 0xABCD, map, metadata)
    }

    fn tile_center(game: &Game, x: u32, y: u32) -> (f32, f32) {
        game.state.map.tile_center(x, y)
    }

    fn assert_angle_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected angle {expected:.4}, got {actual:.4}"
        );
    }

    fn free_unit_position(game: &Game, kind: EntityKind) -> (f32, f32) {
        for ty in 8..game.state.map.size.saturating_sub(8) {
            for tx in 8..game.state.map.size.saturating_sub(8) {
                let (x, y) = game.state.map.tile_center(tx, ty);
                if game
                    .validate_unit_position(&game.state.entities, kind, x, y)
                    .is_ok()
                {
                    return (x, y);
                }
            }
        }
        panic!("no free position found for {kind:?}");
    }

    #[test]
    fn lab_spawn_unit_repairs_supply_and_snapshot_fog() {
        let mut game = new_game();
        let before_supply = game.snapshot_for(1).supply_used;
        let (enemy_x, enemy_y) = tile_center(&game, 35, 35);
        let enemy = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 2,
                kind: EntityKind::Depot,
                x: enemy_x,
                y: enemy_y,
                completed: true,
            }))
            .expect("enemy building should spawn");
        let LabOpOutcome::Spawned {
            entity_id: enemy_id,
        } = enemy
        else {
            panic!("unexpected outcome");
        };

        assert!(
            !game
                .snapshot_for(1)
                .entities
                .iter()
                .any(|entity| entity.id == enemy_id),
            "enemy building should start outside player 1 fog"
        );

        let (scout_x, scout_y) = tile_center(&game, 30, 35);
        let spawned = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::ScoutCar,
                x: scout_x,
                y: scout_y,
                completed: true,
            }))
            .expect("scout should spawn");
        let LabOpOutcome::Spawned { entity_id } = spawned else {
            panic!("unexpected outcome");
        };
        let snapshot = game.snapshot_for(1);
        assert!(snapshot
            .entities
            .iter()
            .any(|entity| entity.id == entity_id));
        assert!(snapshot.entities.iter().any(|entity| entity.id == enemy_id));
        assert_eq!(
            snapshot.supply_used,
            before_supply + rules::economy::supply_cost(EntityKind::ScoutCar)
        );
    }

    #[test]
    fn lab_spawn_building_repairs_supply_cap() {
        let mut game = new_game();
        let before_cap = game.snapshot_for(1).supply_cap;
        let (x, y) = footprint_center(&game.state.map, EntityKind::Depot, 28, 28);

        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Depot,
            x,
            y,
            completed: true,
        }))
        .expect("depot should spawn");

        assert_eq!(
            game.snapshot_for(1).supply_cap,
            before_cap + rules::economy::supply_provided(EntityKind::Depot)
        );
    }

    #[test]
    fn lab_spawn_rejects_nodes_invalid_owners_bad_positions_and_occupied_sites() {
        let mut game = new_game();
        let (x, y) = tile_center(&game, 30, 30);

        assert!(matches!(
            game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Steel,
                x,
                y,
                completed: true,
            })),
            Err(LabError::InvalidKind { .. })
        ));
        assert!(matches!(
            game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 999,
                kind: EntityKind::Worker,
                x,
                y,
                completed: true,
            })),
            Err(LabError::InvalidOwner { owner: 999 })
        ));
        assert!(matches!(
            game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Worker,
                x: f32::NAN,
                y,
                completed: true,
            })),
            Err(LabError::InvalidPosition { .. })
        ));

        let worker = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Worker)
            .expect("starting worker")
            .clone();
        assert!(matches!(
            game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: worker.pos_x,
                y: worker.pos_y,
                completed: true,
            })),
            Err(LabError::OccupiedPosition { .. })
        ));

        let city_centre = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == EntityKind::CityCentre)
            .expect("starting city centre")
            .clone();
        assert!(matches!(
            game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Depot,
                x: city_centre.pos_x,
                y: city_centre.pos_y,
                completed: true,
            })),
            Err(LabError::OccupiedPosition { .. })
        ));
    }

    #[test]
    fn lab_move_entity_validates_collision_and_repairs_position() {
        let mut game = new_game();
        let (x, y) = tile_center(&game, 30, 30);
        let LabOpOutcome::Spawned { entity_id } = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x,
                y,
                completed: true,
            }))
            .expect("rifleman should spawn")
        else {
            panic!("unexpected outcome");
        };

        let (move_x, move_y) = tile_center(&game, 31, 30);
        game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
            entity_id,
            x: move_x,
            y: move_y,
        }))
        .expect("move should be accepted");
        let moved = game.state.entities.get(entity_id).expect("moved entity");
        assert_eq!((moved.pos_x, moved.pos_y), (move_x, move_y));

        let city_centre = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && entity.kind == EntityKind::CityCentre)
            .expect("starting city centre")
            .clone();
        assert!(matches!(
            game.apply_lab_op(LabOp::MoveEntity(LabMoveEntity {
                entity_id,
                x: city_centre.pos_x,
                y: city_centre.pos_y,
            })),
            Err(LabError::OccupiedPosition { .. })
        ));
    }

    #[test]
    fn lab_set_owner_and_delete_repair_supply_and_references() {
        let mut game = new_game();
        let (x, y) = tile_center(&game, 30, 30);
        let LabOpOutcome::Spawned { entity_id } = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Tank,
                x,
                y,
                completed: true,
            }))
            .expect("tank should spawn")
        else {
            panic!("unexpected outcome");
        };

        game.apply_lab_op(LabOp::SetEntityOwner(LabSetEntityOwner {
            entity_id,
            owner: 2,
        }))
        .expect("owner change should be accepted");
        assert_eq!(game.state.entities.get(entity_id).expect("tank").owner, 2);
        assert_eq!(
            game.snapshot_for(1).supply_used,
            rules::economy::supply_cost(EntityKind::Worker) * config::STARTING_WORKERS
        );
        assert!(game.snapshot_for(2).supply_used >= rules::economy::supply_cost(EntityKind::Tank));

        game.apply_lab_op(LabOp::DeleteEntity { entity_id })
            .expect("delete should be accepted");
        assert!(game.state.entities.get(entity_id).is_none());
        assert!(matches!(
            game.apply_lab_op(LabOp::DeleteEntity { entity_id }),
            Err(LabError::StaleEntity { .. })
        ));
    }

    #[test]
    fn lab_resources_and_research_validate_players_and_factions() {
        let mut game = new_game();
        game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
            player_id: 1,
            steel: 1234,
            oil: 567,
        }))
        .expect("resources should be accepted");
        let snapshot = game.snapshot_for(1);
        assert_eq!((snapshot.steel, snapshot.oil), (1234, 567));

        game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
            player_id: 1,
            upgrade: UpgradeKind::TankUnlock,
            completed: true,
        }))
        .expect("research should be accepted");
        assert!(game
            .snapshot_for(1)
            .upgrades
            .contains(&UpgradeKind::TankUnlock.to_protocol_str().to_string()));
        game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
            player_id: 1,
            upgrade: UpgradeKind::TankUnlock,
            completed: false,
        }))
        .expect("research removal should be accepted");
        assert!(!game
            .snapshot_for(1)
            .upgrades
            .contains(&UpgradeKind::TankUnlock.to_protocol_str().to_string()));

        assert!(matches!(
            game.apply_lab_op(LabOp::SetPlayerResources(LabSetPlayerResources {
                player_id: 999,
                steel: 1,
                oil: 1,
            })),
            Err(LabError::InvalidPlayer { player_id: 999 })
        ));
    }

    #[test]
    fn lab_rejects_research_not_in_player_faction_catalog() {
        let players = [PlayerInit {
            id: 7,
            team_id: 7,
            faction_id: "ekat".to_string(),
            name: "Ekat".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        }];
        let map = Map {
            size: 32,
            terrain: vec![terrain::GRASS; 32 * 32],
            starts: vec![(8, 8)],
            expansion_sites: Vec::new(),
        };
        let mut game = Game::new_lab(&players, 1, map, lab_metadata());

        assert!(matches!(
            game.apply_lab_op(LabOp::SetCompletedResearch(LabSetCompletedResearch {
                player_id: 7,
                upgrade: UpgradeKind::TankUnlock,
                completed: true,
            })),
            Err(LabError::InvalidResearch { player_id: 7, .. })
        ));
    }

    #[test]
    fn lab_checkpoint_setup_round_trips_exact_state_with_id_map() {
        let mut game = default_map_game();
        let tank_facing = -1.25;
        let tank_weapon_facing = 0.75;
        let (x, y) = free_unit_position(&game, EntityKind::Tank);
        let LabOpOutcome::Spawned { entity_id: tank_id } = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Tank,
                x,
                y,
                completed: true,
            }))
            .expect("tank should spawn")
        else {
            panic!("unexpected outcome");
        };
        {
            let tank = game.state.entities.get_mut(tank_id).expect("spawned tank");
            tank.set_facing(tank_facing);
            tank.set_weapon_facing(tank_weapon_facing);
            tank.set_desired_weapon_facing(tank_weapon_facing);
        }
        let (x, y) = free_unit_position(&game, EntityKind::AntiTankGun);
        let LabOpOutcome::Spawned { entity_id: gun_id } = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 1,
                kind: EntityKind::AntiTankGun,
                x,
                y,
                completed: true,
            }))
            .expect("anti-tank gun should spawn")
        else {
            panic!("unexpected outcome");
        };
        let setup_target = tile_center(&game, 40, 32);
        let setup_facing = (setup_target.1 - y).atan2(setup_target.0 - x);
        let gun_weapon_facing = setup_facing + 0.125;
        {
            let gun = game.state.entities.get_mut(gun_id).expect("spawned gun");
            gun.set_weapon_setup(WeaponSetup::Deployed);
            gun.set_emplacement_facing(Some(setup_facing));
            gun.set_weapon_facing(gun_weapon_facing);
            gun.set_desired_weapon_facing(gun_weapon_facing);
        }

        let scenario = game
            .export_lab_checkpoint_scenario("Checkpoint setup".to_string(), "test-build")
            .expect("checkpoint setup should export");
        assert_eq!(scenario.metadata.source_scenario, None);
        assert!(scenario
            .metadata
            .source_entity_id_map
            .iter()
            .any(|entry| entry.old_id == tank_id && entry.new_id == tank_id));
        assert!(scenario
            .metadata
            .source_entity_id_map
            .iter()
            .any(|entry| entry.old_id == gun_id && entry.new_id == gun_id));

        let mut restored = Game::restore_lab_checkpoint_scenario(scenario.clone())
            .expect("checkpoint setup should restore");
        let restored_tank = restored.state.entities.get(tank_id).expect("restored tank");
        assert_eq!(restored_tank.kind, EntityKind::Tank);
        assert_eq!(restored_tank.owner, 1);
        assert!(matches!(restored_tank.weapon_setup(), WeaponSetup::Packed));
        assert_angle_close(restored_tank.facing(), tank_facing);
        assert_angle_close(
            restored_tank.weapon_facing().unwrap_or_default(),
            tank_weapon_facing,
        );
        let restored_gun = restored.state.entities.get(gun_id).expect("restored gun");
        assert_eq!(restored_gun.kind, EntityKind::AntiTankGun);
        assert!(matches!(restored_gun.weapon_setup(), WeaponSetup::Deployed));
        assert_angle_close(
            restored_gun.emplacement_facing().unwrap_or_default(),
            setup_facing,
        );
        assert_angle_close(
            restored_gun.weapon_facing().unwrap_or_default(),
            gun_weapon_facing,
        );
        restored.tick();
    }
}
