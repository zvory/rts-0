use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{LabEntityIdRemap, LabError};
use crate::game::map::{BaseResourceCounts, Map};
use crate::game::Game;
use crate::game::MapMetadata;
use crate::protocol::{terrain, validate_map_doodads, MapDoodad, MapTile};
use rts_protocol::{MAX_OIL_PATCHES_PER_BASE, MAX_STEEL_PATCHES_PER_BASE};

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
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<LabScenarioTile>,
    #[serde(rename = "baseSites", alias = "expansionSites")]
    pub base_sites: Vec<LabScenarioBaseSite>,
    #[serde(default)]
    pub doodads: Vec<MapDoodad>,
    #[serde(default)]
    pub stealth_tiles: Vec<MapTile>,
    #[serde(default)]
    pub no_vehicle_tiles: Vec<MapTile>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioTile {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabScenarioBaseSite {
    pub x: u32,
    pub y: u32,
    pub steel_patches: u32,
    pub oil_patches: u32,
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
                width: map.width,
                height: map.height,
                terrain: map.terrain.clone(),
                starts: map
                    .starts
                    .iter()
                    .map(|&(x, y)| LabScenarioTile { x, y })
                    .collect(),
                base_sites: map
                    .base_sites
                    .iter()
                    .map(|&(x, y)| {
                        let counts = map.resource_counts_at((x, y));
                        LabScenarioBaseSite {
                            x,
                            y,
                            steel_patches: counts.steel_patches,
                            oil_patches: counts.oil_patches,
                        }
                    })
                    .collect(),
                doodads: map.doodads.clone(),
                stealth_tiles: map.protocol_stealth_tiles(),
                no_vehicle_tiles: map.protocol_no_vehicle_tiles(),
            },
        }
    }

    pub(super) fn into_map(self) -> Result<(Map, MapMetadata), LabError> {
        self.validate()?;
        let data = self.data;
        let base_resource_counts = data
            .base_sites
            .iter()
            .map(|site| {
                (
                    (site.x, site.y),
                    BaseResourceCounts {
                        steel_patches: site.steel_patches,
                        oil_patches: site.oil_patches,
                    },
                )
            })
            .collect();
        let map = Map {
            width: data.width,
            height: data.height,
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
            base_resource_counts,
            doodads: data.doodads,
            stealth_tiles: data
                .stealth_tiles
                .into_iter()
                .map(|tile| (tile.x, tile.y))
                .collect(),
            no_vehicle_tiles: data
                .no_vehicle_tiles
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
        let width = self.data.width;
        let height = self.data.height;
        let tile_count = width
            .checked_mul(height)
            .map(|count| count as usize)
            .ok_or_else(|| LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map dimensions overflow".to_string(),
            })?;
        if width == 0
            || height == 0
            || tile_count != self.data.terrain.len()
            || tile_count > MAX_LAB_CHECKPOINT_MAP_TILES
        {
            return Err(LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map terrain length is invalid".to_string(),
            });
        }
        for &tile in &self.data.terrain {
            if !terrain::is_known(tile) {
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
        for tile in &self.data.starts {
            if tile.x >= width || tile.y >= height {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map site is out of bounds".to_string(),
                });
            }
        }
        let mut base_sites = HashSet::with_capacity(self.data.base_sites.len());
        for site in &self.data.base_sites {
            if site.x >= width || site.y >= height {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map site is out of bounds".to_string(),
                });
            }
            if !base_sites.insert((site.x, site.y)) {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map contains duplicate base sites".to_string(),
                });
            }
            if site.steel_patches > MAX_STEEL_PATCHES_PER_BASE
                || site.oil_patches > MAX_OIL_PATCHES_PER_BASE
            {
                return Err(LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map resource count is invalid".to_string(),
                });
            }
        }
        let world_width_px =
            width
                .checked_mul(crate::config::TILE_SIZE)
                .ok_or_else(|| LabError::InvalidMap {
                    name: self.name.clone(),
                    reason: "checkpoint scenario map world-pixel dimensions overflow".to_string(),
                })?;
        let world_height_px = height
            .checked_mul(crate::config::TILE_SIZE)
            .ok_or_else(|| LabError::InvalidMap {
                name: self.name.clone(),
                reason: "checkpoint scenario map world-pixel dimensions overflow".to_string(),
            })?;
        validate_map_doodads(&self.data.doodads, world_width_px, world_height_px).map_err(
            |reason| LabError::InvalidMap {
                name: self.name.clone(),
                reason: format!("checkpoint scenario map {reason}"),
            },
        )?;
        validate_overlay_tiles(
            &self.data.stealth_tiles,
            width,
            height,
            "stealthTiles",
            &self.name,
        )?;
        validate_overlay_tiles(
            &self.data.no_vehicle_tiles,
            width,
            height,
            "noVehicleTiles",
            &self.name,
        )?;
        Ok(())
    }
}

fn validate_overlay_tiles(
    tiles: &[MapTile],
    width: u32,
    height: u32,
    field: &str,
    name: &str,
) -> Result<(), LabError> {
    let mut seen = HashSet::with_capacity(tiles.len());
    for tile in tiles {
        if tile.x >= width || tile.y >= height || !seen.insert((tile.x, tile.y)) {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!("checkpoint scenario map {field} is invalid"),
            });
        }
    }
    Ok(())
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
        if !game
            .state
            .players
            .iter()
            .map(|player| player.start_tile)
            .eq(game.state.map.starts.iter().copied())
        {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint player start tiles do not match scenario map starts"
                    .to_string(),
            });
        }
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
