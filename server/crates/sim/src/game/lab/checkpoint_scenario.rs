use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{LabEntityIdRemap, LabError};
use crate::game::map::Map;
use crate::game::Game;
use crate::game::MapMetadata;
use crate::protocol::terrain;

pub(super) const LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION: u32 = 1;
pub(super) const LAB_CHECKPOINT_SCENARIO_KIND: &str = "labCheckpointScenario";
const MAX_LAB_CHECKPOINT_SCENARIO_NAME_LEN: usize = 80;
const MAX_LAB_CHECKPOINT_PLAYERS: usize = 8;
const MAX_LAB_CHECKPOINT_MAP_TILES: usize = 1_000_000;
const MAX_LAB_CHECKPOINT_MAP_STARTS: usize = MAX_LAB_CHECKPOINT_PLAYERS;
const MAX_LAB_CHECKPOINT_MAP_BASE_SITES: usize = MAX_LAB_CHECKPOINT_PLAYERS * 8;

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
    #[serde(rename = "baseSites", alias = "expansionSites")]
    pub base_sites: Vec<LabScenarioTile>,
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
                base_sites: map
                    .base_sites
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
            starts: data
                .starts
                .into_iter()
                .map(|tile| (tile.x, tile.y))
                .collect(),
            base_sites: data
                .base_sites
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
        if self.data.starts.is_empty() || self.data.starts.len() > MAX_LAB_CHECKPOINT_MAP_STARTS {
            return Err(LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map start site count is invalid".to_string(),
            });
        }
        if self.data.base_sites.len() > MAX_LAB_CHECKPOINT_MAP_BASE_SITES {
            return Err(LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map base site count is invalid".to_string(),
            });
        }
        for tile in self
            .data
            .starts
            .iter()
            .chain(self.data.base_sites.iter())
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

fn validate_lab_checkpoint_scenario_shape(
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
    if scenario.name.trim().is_empty() || scenario.name.len() > MAX_LAB_CHECKPOINT_SCENARIO_NAME_LEN
    {
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

fn validate_lab_checkpoint_source_entity_id_map(
    id_map: &[LabEntityIdRemap],
    game: &Game,
) -> Result<(), LabError> {
    let restored_ids: HashSet<_> = game.state.entities.iter().map(|entity| entity.id).collect();
    if id_map.len() > restored_ids.len() {
        return Err(LabError::InvalidScenario {
            reason: "checkpoint scenario sourceEntityIdMap has too many entries".to_string(),
        });
    }

    let mut old_ids = HashSet::new();
    let mut new_ids = HashSet::new();
    for remap in id_map {
        if !old_ids.insert(remap.old_id) {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint scenario sourceEntityIdMap contains duplicate oldId"
                    .to_string(),
            });
        }
        if !new_ids.insert(remap.new_id) {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint scenario sourceEntityIdMap contains duplicate newId"
                    .to_string(),
            });
        }
        if !restored_ids.contains(&remap.new_id) {
            return Err(LabError::InvalidScenario {
                reason:
                    "checkpoint scenario sourceEntityIdMap newId must reference a restored entity"
                        .to_string(),
            });
        }
    }
    Ok(())
}

impl Game {
    pub fn export_lab_checkpoint_scenario(
        &self,
        name: String,
        server_build_sha: &str,
    ) -> Result<LabCheckpointScenarioV1, LabError> {
        let source_entity_id_map = self
            .state
            .entities
            .iter()
            .map(|entity| LabEntityIdRemap {
                old_id: entity.id,
                new_id: entity.id,
            })
            .collect();
        self.export_lab_checkpoint_scenario_with_metadata(
            name,
            self.tick_count(),
            None,
            source_entity_id_map,
            server_build_sha,
        )
    }

    pub fn restore_lab_checkpoint_scenario(
        scenario: LabCheckpointScenarioV1,
    ) -> Result<Game, LabError> {
        validate_lab_checkpoint_scenario_shape(&scenario)?;
        let seed = scenario.seed;
        let (map, map_metadata) = scenario.map.into_map()?;
        let game =
            Game::restore_checkpoint_payload_text(&scenario.checkpoint_payload, map, map_metadata)
                .map_err(|err| LabError::InvalidScenario {
                    reason: format!("checkpoint scenario payload is invalid: {err}"),
                })?;
        if game.seed() != seed {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint scenario seed does not match payload seed".to_string(),
            });
        }
        if scenario.metadata.exported_tick != game.tick_count() {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint scenario exportedTick does not match payload tick".to_string(),
            });
        }
        validate_lab_checkpoint_source_entity_id_map(
            &scenario.metadata.source_entity_id_map,
            &game,
        )?;
        Ok(game)
    }

    fn export_lab_checkpoint_scenario_with_metadata(
        &self,
        name: String,
        exported_tick: u32,
        source_scenario: Option<LabCheckpointScenarioSource>,
        source_entity_id_map: Vec<LabEntityIdRemap>,
        server_build_sha: &str,
    ) -> Result<LabCheckpointScenarioV1, LabError> {
        let checkpoint_payload = self
            .checkpoint_payload_text_for_container("lab", server_build_sha)
            .map_err(|err| LabError::InvalidScenario {
                reason: format!("checkpoint scenario payload export failed: {err}"),
            })?;
        Ok(LabCheckpointScenarioV1 {
            schema_version: LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION,
            kind: LAB_CHECKPOINT_SCENARIO_KIND.to_string(),
            name,
            seed: self.state.seed,
            map: LabCheckpointScenarioMap::from_map(&self.state.map, &self.state.map_metadata),
            metadata: LabCheckpointScenarioMetadata {
                exported_tick,
                source_scenario,
                source_entity_id_map,
            },
            checkpoint_payload,
        })
    }
}
