use serde::{Deserialize, Serialize};

use crate::game::entity::{EntityKind, MAX_QUEUED_ORDERS};
use crate::game::map::Map;
use crate::game::MapMetadata;
use crate::protocol::terrain;

use super::orientation::{
    lab_setup_capable, lab_weapon_facing_capable, validate_optional_lab_angle,
};
use super::{LabEntityIdRemap, LabError};

pub(super) const LAB_SCENARIO_V1_SCHEMA_VERSION: u32 = 1;
pub(super) const LAB_SCENARIO_KIND: &str = "labScenario";
pub(super) const LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION: u32 = 1;
pub(super) const LAB_CHECKPOINT_SCENARIO_KIND: &str = "labCheckpointScenario";
pub(super) const MAX_LAB_SCENARIO_NAME_LEN: usize = 80;
pub(super) const MAX_LAB_SCENARIO_PLAYERS: usize = 8;
pub(super) const MAX_LAB_SCENARIO_ENTITIES: usize = 2000;
pub(super) const MAX_LAB_SCENARIO_UPGRADES_PER_PLAYER: usize = 32;
const MAX_LAB_CHECKPOINT_MAP_TILES: usize = 1_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabScenarioMap,
    pub players: Vec<LabScenarioPlayer>,
    pub entities: Vec<LabScenarioEntity>,
    pub metadata: LabScenarioMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioV1 {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub seed: u32,
    pub map: LabCheckpointScenarioMap,
    pub metadata: LabCheckpointScenarioMetadata,
    pub checkpoint_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
    pub materialized_hash: String,
    pub data: LabCheckpointScenarioMapData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMapData {
    pub size: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabScenarioTile>,
    pub expansion_sites: Vec<LabScenarioTile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioTile {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioMetadata {
    pub exported_tick: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scenario: Option<LabCheckpointScenarioSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_entity_id_map: Vec<LabEntityIdRemap>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabCheckpointScenarioSource {
    pub kind: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMetadata {
    pub exported_tick: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioMap {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPlayer {
    pub id: u32,
    pub team_id: u32,
    pub faction_id: String,
    pub name: String,
    pub color: String,
    pub is_ai: bool,
    pub resources: LabScenarioResources,
    pub research: LabScenarioResearch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioResources {
    pub steel: u32,
    pub oil: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioResearch {
    pub completed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioEntity {
    pub id: u32,
    pub owner: u32,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub hp: u32,
    pub completed: bool,
    pub construction_progress: Option<u32>,
    pub construction_total: Option<u32>,
    pub resource_remaining: Option<u32>,
    #[serde(default)]
    pub facing: Option<f32>,
    #[serde(default)]
    pub weapon_facing: Option<f32>,
    #[serde(default)]
    pub set_up: bool,
    #[serde(default)]
    pub setup_facing: Option<f32>,
    #[serde(default)]
    pub setup_target: Option<LabScenarioPoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<LabScenarioOrder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued_orders: Vec<LabScenarioOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioOrder {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_x: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_y: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staging_y: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LabScenarioPoint {
    pub x: f32,
    pub y: f32,
}

impl LabCheckpointScenarioMap {
    pub(super) fn from_map(map: &Map, metadata: &MapMetadata) -> Self {
        Self {
            name: metadata.name.clone(),
            schema_version: metadata.schema_version,
            content_hash: metadata.content_hash.clone(),
            materialized_hash: map.materialized_hash(),
            data: LabCheckpointScenarioMapData {
                size: map.size,
                terrain: map.terrain.clone(),
                starts: map
                    .starts
                    .iter()
                    .map(|&(x, y)| LabScenarioTile { x, y })
                    .collect(),
                expansion_sites: map
                    .expansion_sites
                    .iter()
                    .map(|&(x, y)| LabScenarioTile { x, y })
                    .collect(),
            },
        }
    }

    pub(super) fn into_map(self) -> Result<(Map, MapMetadata), LabError> {
        self.validate()?;
        let data = self.data;
        let map = Map {
            size: data.size,
            terrain: data.terrain,
            starts: data.starts.into_iter().map(|tile| (tile.x, tile.y)).collect(),
            expansion_sites: data
                .expansion_sites
                .into_iter()
                .map(|tile| (tile.x, tile.y))
                .collect(),
        };
        if map.materialized_hash() != self.materialized_hash {
            return Err(LabError::InvalidMap {
                name: self.name,
                reason: "checkpoint scenario map materialized hash does not match map data"
                    .to_string(),
            });
        }
        Ok((
            map,
            MapMetadata {
                name: self.name,
                schema_version: self.schema_version,
                content_hash: self.content_hash,
            },
        ))
    }

    fn validate(&self) -> Result<(), LabError> {
        if self.name.trim().is_empty() {
            return Err(LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map name must be non-empty".to_string(),
            });
        }
        let size = self.data.size;
        let tile_count = size
            .checked_mul(size)
            .map(|count| count as usize)
            .ok_or_else(|| LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map size overflows".to_string(),
            })?;
        if size == 0
            || tile_count != self.data.terrain.len()
            || tile_count > MAX_LAB_CHECKPOINT_MAP_TILES
        {
            return Err(LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map terrain length is invalid".to_string(),
            });
        }
        for &tile in &self.data.terrain {
            if !matches!(tile, terrain::GRASS | terrain::ROCK | terrain::WATER) {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map contains an unknown terrain code".to_string(),
                });
            }
        }
        for tile in self
            .data
            .starts
            .iter()
            .chain(self.data.expansion_sites.iter())
        {
            if tile.x >= size || tile.y >= size {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map site is out of bounds".to_string(),
                });
            }
        }
        Ok(())
    }
}

pub(super) fn validate_lab_scenario_shape(scenario: &LabScenarioV1) -> Result<(), LabError> {
    if scenario.schema_version != LAB_SCENARIO_V1_SCHEMA_VERSION {
        return Err(LabError::InvalidScenarioVersion {
            version: scenario.schema_version,
        });
    }
    if scenario.kind != LAB_SCENARIO_KIND {
        return Err(LabError::InvalidScenario {
            reason: "scenario kind must be labScenario".to_string(),
        });
    }
    if scenario.name.trim().is_empty() || scenario.name.len() > MAX_LAB_SCENARIO_NAME_LEN {
        return Err(LabError::InvalidScenario {
            reason: "scenario name must be non-empty and at most 80 bytes".to_string(),
        });
    }
    if scenario.players.is_empty() {
        return Err(LabError::InvalidScenario {
            reason: "scenario must contain at least one player".to_string(),
        });
    }
    if scenario.players.len() > MAX_LAB_SCENARIO_PLAYERS {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "scenario has too many players: {} > {}",
                scenario.players.len(),
                MAX_LAB_SCENARIO_PLAYERS
            ),
        });
    }
    if scenario.entities.len() > MAX_LAB_SCENARIO_ENTITIES {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "scenario has too many entities: {} > {}",
                scenario.entities.len(),
                MAX_LAB_SCENARIO_ENTITIES
            ),
        });
    }
    Ok(())
}

pub(super) fn validate_lab_checkpoint_scenario_shape(
    scenario: &LabCheckpointScenarioV1,
) -> Result<(), LabError> {
    if scenario.schema_version != LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION {
        return Err(LabError::InvalidScenarioVersion {
            version: scenario.schema_version,
        });
    }
    if scenario.kind != LAB_CHECKPOINT_SCENARIO_KIND {
        return Err(LabError::InvalidScenario {
            reason: "checkpoint scenario kind must be labCheckpointScenario".to_string(),
        });
    }
    if scenario.name.trim().is_empty() || scenario.name.len() > MAX_LAB_SCENARIO_NAME_LEN {
        return Err(LabError::InvalidScenario {
            reason: "checkpoint scenario name must be non-empty and at most 80 bytes".to_string(),
        });
    }
    if scenario.checkpoint_payload.trim().is_empty() {
        return Err(LabError::InvalidScenario {
            reason: "checkpoint scenario payload must be non-empty".to_string(),
        });
    }
    Ok(())
}

pub(super) fn validate_lab_entity_setup_shape(
    entity: &LabScenarioEntity,
    kind: EntityKind,
) -> Result<(), LabError> {
    validate_optional_lab_angle(entity, "facing", entity.facing)?;
    validate_optional_lab_angle(entity, "weaponFacing", entity.weapon_facing)?;
    validate_optional_lab_angle(entity, "setupFacing", entity.setup_facing)?;

    if entity.facing.is_some() && !kind.is_unit() {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} kind {} cannot have facing",
                entity.id, entity.kind
            ),
        });
    }
    if entity.weapon_facing.is_some() && !lab_weapon_facing_capable(kind) {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} kind {} cannot have weaponFacing",
                entity.id, entity.kind
            ),
        });
    }
    if entity.setup_target.is_some() && !entity.set_up {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has setupTarget without setUp", entity.id),
        });
    }
    if entity.setup_facing.is_some() && !entity.set_up {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has setupFacing without setUp", entity.id),
        });
    }
    if (entity.set_up || entity.setup_target.is_some() || entity.setup_facing.is_some())
        && !lab_setup_capable(kind)
    {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} kind {} cannot be set up", entity.id, entity.kind),
        });
    }
    if entity.set_up && entity.setup_facing.is_none() && entity.setup_target.is_none() {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} has setUp without setupFacing or setupTarget",
                entity.id
            ),
        });
    }
    validate_lab_entity_order_shape(entity)?;
    Ok(())
}

fn validate_lab_entity_order_shape(entity: &LabScenarioEntity) -> Result<(), LabError> {
    if entity.queued_orders.len() > MAX_QUEUED_ORDERS {
        return Err(LabError::InvalidScenario {
            reason: format!(
                "entity {} has too many queuedOrders: {} > {}",
                entity.id,
                entity.queued_orders.len(),
                MAX_QUEUED_ORDERS
            ),
        });
    }
    if let Some(order) = entity.order.as_ref() {
        validate_lab_order_shape(entity, order)?;
    }
    for order in &entity.queued_orders {
        validate_lab_order_shape(entity, order)?;
    }
    Ok(())
}

fn validate_lab_order_shape(
    entity: &LabScenarioEntity,
    order: &LabScenarioOrder,
) -> Result<(), LabError> {
    if order.kind.trim().is_empty() {
        return Err(LabError::InvalidScenario {
            reason: format!("entity {} has an order with empty kind", entity.id),
        });
    }
    for (field, value) in [
        ("x", order.x),
        ("y", order.y),
        ("stagingX", order.staging_x),
        ("stagingY", order.staging_y),
    ] {
        if value.is_some_and(|value| !value.is_finite()) {
            return Err(LabError::InvalidScenario {
                reason: format!("entity {} has non-finite order {field}", entity.id),
            });
        }
    }
    Ok(())
}
