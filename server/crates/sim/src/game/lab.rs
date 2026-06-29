//! Authoritative lab mutation API.
//!
//! Lab callers get typed operations with validation at the `Game` seam. This module owns the repair
//! pass so room/client code never reaches into stores, fog, spatial indexes, or economy state.

use std::collections::HashSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::config;
use crate::game::entity::{EntityKind, EntityStore, Order, OrderIntent, NEUTRAL};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, circle_intersects_rect, CircleBody,
};
use crate::game::services::occupancy::{footprint_center, footprint_tiles, Occupancy};
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::{production, standability};
use crate::game::upgrade::UpgradeKind;
use crate::protocol::Command;
use crate::rules;

use super::{systems, Game, MapMetadata, PlayerInit};

mod orientation;
mod resource_nodes;
mod scenario;

use orientation::{
    lab_entity_facing, lab_entity_is_set_up, lab_entity_setup_facing, lab_entity_setup_target,
    lab_entity_weapon_facing, restore_lab_entity_orientation, restore_lab_entity_setup,
};
use scenario::{
    validate_lab_entity_setup_shape, validate_lab_scenario_shape, LAB_SCENARIO_KIND,
    MAX_LAB_SCENARIO_UPGRADES_PER_PLAYER,
};
pub use scenario::{
    LabScenarioEntity, LabScenarioMap, LabScenarioMetadata, LabScenarioPlayer, LabScenarioPoint,
    LabScenarioResearch, LabScenarioResources, LabScenarioV1,
};

pub const LAB_SCENARIO_V1_SCHEMA_VERSION: u32 = scenario::LAB_SCENARIO_V1_SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq)]
pub enum LabOp {
    SpawnEntity(LabSpawnEntity),
    DeleteEntity { entity_id: u32 },
    MoveEntity(LabMoveEntity),
    SetEntityOwner(LabSetEntityOwner),
    SetPlayerResources(LabSetPlayerResources),
    SetPlayerGodMode { player_id: u32, enabled: bool },
    SetCompletedResearch(LabSetCompletedResearch),
    RestoreScenario(Box<LabScenarioV1>),
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
            LabOp::RestoreScenario(scenario) => self.restore_lab_scenario(*scenario),
        }
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
            let Some(entity) = self.entities.get(entity_id) else {
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

    pub fn export_lab_scenario(&self) -> LabScenarioV1 {
        let players = self
            .players
            .iter()
            .map(|player| LabScenarioPlayer {
                id: player.id,
                team_id: player.team_id,
                faction_id: player.faction_id.clone(),
                name: player.name.clone(),
                color: player.color.clone(),
                is_ai: player.is_ai,
                resources: LabScenarioResources {
                    steel: player.steel,
                    oil: player.oil,
                },
                research: LabScenarioResearch {
                    completed: player
                        .upgrades
                        .iter()
                        .map(|upgrade| upgrade.to_protocol_str().to_string())
                        .collect(),
                },
            })
            .collect();

        let entities = self
            .entities
            .iter()
            .map(|entity| LabScenarioEntity {
                id: entity.id,
                owner: entity.owner,
                kind: entity.kind.to_string(),
                x: entity.pos_x,
                y: entity.pos_y,
                hp: entity.hp,
                completed: !entity.under_construction(),
                construction_progress: entity.construction.as_ref().map(|state| state.progress),
                construction_total: entity.construction.as_ref().map(|state| state.total),
                resource_remaining: entity.remaining(),
                facing: lab_entity_facing(entity),
                weapon_facing: lab_entity_weapon_facing(entity),
                set_up: lab_entity_is_set_up(entity),
                setup_facing: lab_entity_setup_facing(entity),
                setup_target: lab_entity_setup_target(&self.map, entity),
            })
            .collect();

        LabScenarioV1 {
            schema_version: LAB_SCENARIO_V1_SCHEMA_VERSION,
            kind: LAB_SCENARIO_KIND.to_string(),
            name: "Untitled lab scenario".to_string(),
            seed: self.seed,
            map: LabScenarioMap {
                name: self.map_metadata.name.clone(),
                schema_version: self.map_metadata.schema_version,
                content_hash: self.map_metadata.content_hash.clone(),
            },
            players,
            entities,
            metadata: LabScenarioMetadata {
                exported_tick: self.tick_count(),
            },
        }
    }

    pub fn lab_god_mode_players(&self) -> Vec<u32> {
        self.lab_god_mode_players.iter().copied().collect()
    }

    pub fn restore_lab_scenario(
        &mut self,
        scenario: LabScenarioV1,
    ) -> Result<LabOpOutcome, LabError> {
        validate_lab_scenario_shape(&scenario)?;

        let mut seen_players = HashSet::new();
        let mut inits = Vec::with_capacity(scenario.players.len());
        for player in &scenario.players {
            if player.id == NEUTRAL || !seen_players.insert(player.id) {
                return Err(LabError::InvalidPlayer {
                    player_id: player.id,
                });
            }
            if rules::faction::catalog_for_or_default_empty(&player.faction_id).is_none() {
                return Err(LabError::InvalidScenario {
                    reason: format!("unknown faction {:?}", player.faction_id),
                });
            }
            if player.name.len() > 48 || player.color.len() > 32 {
                return Err(LabError::InvalidScenario {
                    reason: format!("player {} has invalid display metadata", player.id),
                });
            }
            if player.research.completed.len() > MAX_LAB_SCENARIO_UPGRADES_PER_PLAYER {
                return Err(LabError::InvalidScenario {
                    reason: format!("player {} has too many upgrades", player.id),
                });
            }
            inits.push(PlayerInit {
                id: player.id,
                team_id: player.team_id,
                faction_id: player.faction_id.clone(),
                name: player.name.clone(),
                color: player.color.clone(),
                is_ai: player.is_ai,
            });
        }

        let start_players: Vec<_> = inits
            .iter()
            .map(|player| {
                (
                    player.id,
                    super::teams::normalize_team_id(player.id, player.team_id),
                )
            })
            .collect();
        let map = Map::load_for_players(&scenario.map.name, &start_players, scenario.seed)
            .map_err(|reason| LabError::InvalidMap {
                name: scenario.map.name.clone(),
                reason,
            })?;
        let map_metadata =
            Map::metadata_for_name(&scenario.map.name).map_err(|reason| LabError::InvalidMap {
                name: scenario.map.name.clone(),
                reason,
            })?;
        if map_metadata.schema_version != scenario.map.schema_version
            || map_metadata.content_hash != scenario.map.content_hash
        {
            return Err(LabError::InvalidMap {
                name: scenario.map.name,
                reason: "scenario map metadata does not match the bundled map".to_string(),
            });
        }

        let mut restored = Game::new_lab(&inits, scenario.seed, map, map_metadata);
        for player in &scenario.players {
            let Some(state) = restored
                .players
                .iter_mut()
                .find(|state| state.id == player.id)
            else {
                return Err(LabError::InvalidPlayer {
                    player_id: player.id,
                });
            };
            state.set_resources(player.resources.steel, player.resources.oil);
            state.upgrades.clear();
            for upgrade_id in &player.research.completed {
                let upgrade =
                    UpgradeKind::from_str(upgrade_id).map_err(|_| LabError::InvalidResearch {
                        player_id: player.id,
                        upgrade: upgrade_id.clone(),
                    })?;
                validate_upgrade_for_player(state, upgrade)?;
                state.upgrades.insert(upgrade);
            }
        }

        restored.entities = EntityStore::new();
        let mut entity_id_map = Vec::with_capacity(scenario.entities.len());
        let mut seen_entities = HashSet::new();
        for entity in &scenario.entities {
            if entity.id == 0 || !seen_entities.insert(entity.id) {
                return Err(LabError::InvalidScenario {
                    reason: format!("duplicate or zero entity id {}", entity.id),
                });
            }
            let kind = EntityKind::from_str(&entity.kind).map_err(|_| LabError::InvalidKind {
                kind: entity.kind.clone(),
                operation: "restoreScenario",
            })?;
            validate_lab_entity_setup_shape(entity, kind)?;
            let new_id = restored.restore_lab_entity(entity, kind)?;
            entity_id_map.push(LabEntityIdRemap {
                old_id: entity.id,
                new_id,
            });
        }
        restored.repair_lab_state();

        *self = restored;
        Ok(LabOpOutcome::ScenarioRestored(LabScenarioRestore {
            entity_id_map,
        }))
    }

    fn lab_spawn_entity(&mut self, input: LabSpawnEntity) -> Result<LabOpOutcome, LabError> {
        self.validate_owner(input.owner)?;
        let id = if input.kind.is_unit() {
            self.validate_unit_position(&self.entities, input.kind, input.x, input.y)?;
            self.entities
                .spawn_unit(input.owner, input.kind, input.x, input.y)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else if input.kind.is_building() {
            let (_, _, x, y) =
                self.validate_building_position(&self.entities, input.kind, input.x, input.y)?;
            self.entities
                .spawn_building(input.owner, input.kind, x, y, input.completed)
                .ok_or_else(|| invalid_kind(input.kind, "spawnEntity"))?
        } else {
            return Err(invalid_kind(input.kind, "spawnEntity"));
        };
        self.repair_lab_state();
        Ok(LabOpOutcome::Spawned { entity_id: id })
    }

    fn lab_delete_entity(&mut self, entity_id: u32) -> Result<LabOpOutcome, LabError> {
        self.entities
            .remove(entity_id)
            .ok_or(LabError::StaleEntity { entity_id })?;
        self.entities.release_miner(entity_id);
        self.cleanup_entity_references(entity_id);
        self.repair_lab_state();
        Ok(LabOpOutcome::Deleted { entity_id })
    }

    fn lab_move_entity(&mut self, input: LabMoveEntity) -> Result<LabOpOutcome, LabError> {
        let (kind, is_unit, is_building) = {
            let entity = self
                .entities
                .get(input.entity_id)
                .ok_or(LabError::StaleEntity {
                    entity_id: input.entity_id,
                })?;
            (entity.kind, entity.is_unit(), entity.is_building())
        };

        let mut entities_without = self.entities.clone();
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

        if let Some(entity) = self.entities.get_mut(input.entity_id) {
            entity.set_position(x, y);
            entity.clear_orders();
        }
        self.entities.release_miner(input.entity_id);
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
            .entities
            .get(input.entity_id)
            .ok_or(LabError::StaleEntity {
                entity_id: input.entity_id,
            })?
            .kind;
        if !kind.is_unit() && !kind.is_building() {
            return Err(invalid_kind(kind, "setEntityOwner"));
        }

        if let Some(entity) = self.entities.get_mut(input.entity_id) {
            entity.owner = input.owner;
            entity.clear_orders();
            clear_lab_production_state(entity);
        }
        self.entities.release_miner(input.entity_id);
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
            self.lab_god_mode_players.insert(player_id);
        } else {
            self.lab_god_mode_players.remove(&player_id);
        }
        self.sync_lab_god_mode_flags();
        Ok(LabOpOutcome::PlayerGodModeSet { player_id, enabled })
    }

    fn lab_set_completed_research(
        &mut self,
        input: LabSetCompletedResearch,
    ) -> Result<LabOpOutcome, LabError> {
        let player = self
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
            &mut self.entities,
            input.player_id,
            &player.upgrades,
        );
        Ok(LabOpOutcome::CompletedResearchSet {
            player_id: input.player_id,
            upgrade: input.upgrade,
            completed: input.completed,
        })
    }

    fn restore_lab_entity(
        &mut self,
        entity: &LabScenarioEntity,
        kind: EntityKind,
    ) -> Result<u32, LabError> {
        validate_world_position(&self.map, entity.x, entity.y)?;
        if entity.hp == 0 {
            return Err(LabError::InvalidScenario {
                reason: format!("entity {} has zero hp", entity.id),
            });
        }

        let new_id = if kind.is_unit() {
            if !entity.completed || entity.construction_progress.is_some() {
                return Err(LabError::InvalidScenario {
                    reason: format!("unit {} cannot have construction state", entity.id),
                });
            }
            self.validate_owner(entity.owner)?;
            self.validate_unit_position(&self.entities, kind, entity.x, entity.y)?;
            let id = self
                .entities
                .spawn_unit(entity.owner, kind, entity.x, entity.y)
                .ok_or_else(|| invalid_kind(kind, "restoreScenario"))?;
            if let Some(restored) = self.entities.get_mut(id) {
                restored.hp = entity.hp.min(restored.max_hp).max(1);
                restore_lab_entity_orientation(entity, restored)?;
                restore_lab_entity_setup(&self.map, entity, restored)?;
            }
            id
        } else if kind.is_building() {
            self.validate_owner(entity.owner)?;
            let (_, _, x, y) =
                self.validate_building_position(&self.entities, kind, entity.x, entity.y)?;
            let completed = entity.completed && entity.construction_progress.is_none();
            let id = self
                .entities
                .spawn_building(entity.owner, kind, x, y, completed)
                .ok_or_else(|| invalid_kind(kind, "restoreScenario"))?;
            if let Some(restored) = self.entities.get_mut(id) {
                if let Some(progress) = entity.construction_progress {
                    if entity.completed {
                        return Err(LabError::InvalidScenario {
                            reason: format!(
                                "building {} cannot be both completed and under construction",
                                entity.id
                            ),
                        });
                    }
                    restored.set_construction_progress(progress);
                }
                if entity.construction_total.is_some_and(|total| {
                    restored
                        .construction
                        .as_ref()
                        .is_some_and(|state| state.total != total)
                }) {
                    return Err(LabError::InvalidScenario {
                        reason: format!("building {} construction total mismatch", entity.id),
                    });
                }
                restored.hp = entity.hp.min(restored.max_hp).max(1);
                restore_lab_entity_orientation(entity, restored)?;
            }
            id
        } else if kind.is_node() {
            if entity.owner != NEUTRAL {
                return Err(LabError::InvalidOwner {
                    owner: entity.owner,
                });
            }
            let (x, y) = resource_nodes::restore_resource_node_position(
                &self.map,
                &self.entities,
                kind,
                entity.x,
                entity.y,
                kind == EntityKind::Oil,
            )?;
            let id = self
                .entities
                .spawn_node(kind, x, y)
                .ok_or_else(|| invalid_kind(kind, "restoreScenario"))?;
            if let Some(restored) = self.entities.get_mut(id) {
                if let Some(remaining) = entity.resource_remaining {
                    if let Some(node) = restored.resource_node.as_mut() {
                        node.remaining = remaining;
                    }
                }
            }
            id
        } else {
            return Err(invalid_kind(kind, "restoreScenario"));
        };
        Ok(new_id)
    }

    fn validate_owner(&self, owner: u32) -> Result<(), LabError> {
        if owner == NEUTRAL {
            return Err(LabError::InvalidOwner { owner });
        }
        self.validate_player(owner)
            .map_err(|_| LabError::InvalidOwner { owner })
    }

    fn validate_player(&self, player_id: u32) -> Result<(), LabError> {
        if self.players.iter().any(|player| player.id == player_id) {
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
        validate_world_position(&self.map, x, y)?;
        let occ = Occupancy::build(&self.map, entities);
        if !standability::unit_spawn_standable(&self.map, &occ, entities, kind, x, y) {
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
        validate_world_position(&self.map, x, y)?;
        let (tile_x, tile_y, center_x, center_y) =
            building_top_left_for_center(&self.map, kind, x, y)?;
        for (tx, ty) in footprint_tiles(kind, tile_x, tile_y) {
            if !self.map.in_bounds(tx as i32, ty as i32)
                || !self.map.is_passable(tx as i32, ty as i32)
            {
                return Err(LabError::InvalidPosition {
                    x,
                    y,
                    reason: "building footprint is out of bounds or on blocked terrain",
                });
            }
        }
        if !standability::building_site_clear(&self.map, entities, kind, tile_x, tile_y) {
            return Err(LabError::OccupiedPosition { x, y });
        }
        Ok((tile_x, tile_y, center_x, center_y))
    }

    fn cleanup_entity_references(&mut self, entity_id: u32) {
        for id in self.entities.ids() {
            let Some(entity) = self.entities.get_mut(id) else {
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
        self.entities.clear_stale_miner_slots();
    }

    fn repair_lab_state(&mut self) {
        self.entities.clear_stale_miner_slots();
        self.sync_lab_god_mode_flags();
        self.repair_mortar_autocast_state();
        systems::recompute_supply(&mut self.players, &self.entities);
        self.spatial = SpatialIndex::build(&self.entities, self.map.size);
        let ids: Vec<u32> = self.players.iter().map(|player| player.id).collect();
        self.fog.recompute_with_smoke(&ids, &self.entities, &self.map, &self.smokes);
        self.refresh_building_memory(&ids);
        self.refresh_trench_memory(&ids);
        #[cfg(debug_assertions)]
        self.assert_invariants();
    }

    fn repair_mortar_autocast_state(&mut self) {
        for player in &self.players {
            production::sync_owned_autocast_from_upgrades(
                &mut self.entities,
                player.id,
                &player.upgrades,
            );
        }
    }

    pub(crate) fn sync_lab_god_mode_flags(&mut self) {
        let enabled_players = self.lab_god_mode_players.clone();
        for entity_id in self.entities.ids() {
            if let Some(entity) = self.entities.get_mut(entity_id) {
                let is_player_asset = entity.is_unit() || entity.is_building();
                entity.set_invulnerable(is_player_asset && enabled_players.contains(&entity.owner));
            }
        }
    }
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

fn validate_resource_node_position(
    map: &Map,
    entities: &EntityStore,
    x: f32,
    y: f32,
) -> Result<(), LabError> {
    validate_world_position(map, x, y)?;
    let (tx, ty) = map.tile_of(x, y);
    if !map.is_passable(tx as i32, ty as i32) {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "resource node must be on passable terrain",
        });
    }
    let body = CircleBody {
        x,
        y,
        radius: config::TILE_SIZE as f32 * 0.5,
    };
    for entity in entities.iter() {
        if !entity.is_building() {
            continue;
        }
        if building_rect_for_entity(map, entity)
            .is_some_and(|rect| circle_intersects_rect(body, rect))
        {
            return Err(LabError::OccupiedPosition { x, y });
        }
    }
    Ok(())
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

fn clear_lab_production_state(entity: &mut crate::game::entity::Entity) {
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
}

fn order_intent_references_entity(intent: &OrderIntent, entity_id: u32) -> bool {
    match intent {
        OrderIntent::Attack(target) => target.target == entity_id,
        OrderIntent::Gather(gather) => gather.node == entity_id,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::WeaponSetup;
    use crate::game::services::occupancy::footprint_center;
    use crate::protocol::terrain;

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
        game.map.tile_center(x, y)
    }

    fn assert_angle_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.001,
            "expected angle {expected:.4}, got {actual:.4}"
        );
    }

    fn free_unit_position(game: &Game, kind: EntityKind) -> (f32, f32) {
        for ty in 8..game.map.size.saturating_sub(8) {
            for tx in 8..game.map.size.saturating_sub(8) {
                let (x, y) = game.map.tile_center(tx, ty);
                if game
                    .validate_unit_position(&game.entities, kind, x, y)
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
        let (x, y) = footprint_center(&game.map, EntityKind::Depot, 28, 28);

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
        let moved = game.entities.get(entity_id).expect("moved entity");
        assert_eq!((moved.pos_x, moved.pos_y), (move_x, move_y));

        let city_centre = game
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
        assert_eq!(game.entities.get(entity_id).expect("tank").owner, 2);
        assert_eq!(
            game.snapshot_for(1).supply_used,
            rules::economy::supply_cost(EntityKind::Worker) * config::STARTING_WORKERS
        );
        assert!(game.snapshot_for(2).supply_used >= rules::economy::supply_cost(EntityKind::Tank));

        game.apply_lab_op(LabOp::DeleteEntity { entity_id })
            .expect("delete should be accepted");
        assert!(game.entities.get(entity_id).is_none());
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
    fn lab_scenario_export_restore_round_trips_setup_with_id_remap() {
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
            let tank = game.entities.get_mut(tank_id).expect("spawned tank");
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
            let gun = game.entities.get_mut(gun_id).expect("spawned gun");
            gun.set_weapon_setup(WeaponSetup::Deployed);
            gun.set_emplacement_facing(Some(setup_facing));
            gun.set_weapon_facing(gun_weapon_facing);
            gun.set_desired_weapon_facing(gun_weapon_facing);
        }

        let scenario = game.export_lab_scenario();
        let exported_tank = scenario
            .entities
            .iter()
            .find(|entity| entity.id == tank_id)
            .expect("exported tank");
        assert!(!exported_tank.set_up);
        assert_eq!(exported_tank.facing, Some(tank_facing));
        assert_eq!(exported_tank.weapon_facing, Some(tank_weapon_facing));
        assert_eq!(exported_tank.setup_facing, None);
        assert_eq!(exported_tank.setup_target, None);
        let exported_gun = scenario
            .entities
            .iter()
            .find(|entity| entity.id == gun_id)
            .expect("exported gun");
        assert!(exported_gun.set_up);
        assert_angle_close(exported_gun.setup_facing.unwrap_or_default(), setup_facing);
        assert_angle_close(
            exported_gun.weapon_facing.unwrap_or_default(),
            gun_weapon_facing,
        );
        assert!(exported_gun.setup_target.is_some());

        let mut restored = default_map_game();
        let LabOpOutcome::ScenarioRestored(result) = restored
            .apply_lab_op(LabOp::RestoreScenario(Box::new(scenario)))
            .expect("scenario restore should succeed")
        else {
            panic!("unexpected outcome");
        };
        let remap = result
            .entity_id_map
            .iter()
            .find(|entry| entry.old_id == tank_id)
            .expect("spawned entity should be remapped");
        let restored_tank = restored.entities.get(remap.new_id).expect("restored tank");
        assert_eq!(restored_tank.kind, EntityKind::Tank);
        assert_eq!(restored_tank.owner, 1);
        assert!(matches!(restored_tank.weapon_setup(), WeaponSetup::Packed));
        assert_angle_close(restored_tank.facing(), tank_facing);
        assert_angle_close(
            restored_tank.weapon_facing().unwrap_or_default(),
            tank_weapon_facing,
        );
        let gun_remap = result
            .entity_id_map
            .iter()
            .find(|entry| entry.old_id == gun_id)
            .expect("gun should be remapped");
        let restored_gun = restored
            .entities
            .get(gun_remap.new_id)
            .expect("restored gun");
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
    }

    #[test]
    fn lab_scenario_restore_rejects_invalid_upgrade_string() {
        let mut game = default_map_game();
        let mut scenario = game.export_lab_scenario();
        scenario.players[0]
            .research
            .completed
            .push("not_real".to_string());

        assert!(matches!(
            game.apply_lab_op(LabOp::RestoreScenario(Box::new(scenario))),
            Err(LabError::InvalidResearch { player_id: 1, .. })
        ));
    }

    #[test]
    fn lab_scenario_restore_rejects_invalid_schema_fields() {
        let mut game = default_map_game();
        let mut scenario = game.export_lab_scenario();
        scenario.kind = "snapshot".to_string();
        assert!(matches!(
            game.apply_lab_op(LabOp::RestoreScenario(Box::new(scenario))),
            Err(LabError::InvalidScenario { .. })
        ));

        let mut scenario = game.export_lab_scenario();
        scenario.name.clear();
        assert!(matches!(
            game.apply_lab_op(LabOp::RestoreScenario(Box::new(scenario))),
            Err(LabError::InvalidScenario { .. })
        ));

        let mut scenario = game.export_lab_scenario();
        scenario.entities[0].set_up = true;
        scenario.entities[0].setup_facing = Some(0.0);
        assert!(matches!(
            game.apply_lab_op(LabOp::RestoreScenario(Box::new(scenario))),
            Err(LabError::InvalidScenario { .. })
        ));
    }

}
