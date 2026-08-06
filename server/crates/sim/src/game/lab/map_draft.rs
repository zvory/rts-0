use std::collections::HashMap;

use crate::game::map::{doodads, BaseResourceCounts, Map};
use crate::game::MapOverlayTiles;
use crate::protocol::{LabBaseSite, LabMapDraft, LabMapTile, MapDoodad, MapTile};
use rts_protocol::{MAX_OIL_PATCHES_PER_BASE, MAX_STEEL_PATCHES_PER_BASE};

use super::LabError;

pub(super) fn export(map: &Map, name: &str) -> LabMapDraft {
    let overlays = map.protocol_overlay_tiles();
    LabMapDraft {
        name: name.to_string(),
        width: map.width,
        height: map.height,
        terrain: map.terrain.clone(),
        starts: map
            .starts
            .iter()
            .map(|&(x, y)| LabMapTile { x, y })
            .collect(),
        base_sites: map
            .base_sites
            .iter()
            .map(|&(x, y)| {
                let counts = map.resource_counts_at((x, y));
                LabBaseSite {
                    x,
                    y,
                    steel_patches: counts.steel_patches,
                    oil_patches: counts.oil_patches,
                }
            })
            .collect(),
        doodads: map.doodads.clone(),
        concealment_tiles: overlays.concealment,
        no_vehicle_tiles: overlays.no_vehicle,
        no_building_tiles: overlays.no_building,
        no_entrenchment_tiles: overlays.no_entrenchment,
        damage_reduction_tiles: overlays.damage_reduction,
        slow_movement_tiles: overlays.slow_movement,
    }
}

pub(super) fn resource_counts(
    draft: &LabMapDraft,
    name: &str,
) -> Result<HashMap<(u32, u32), BaseResourceCounts>, LabError> {
    let mut counts = HashMap::with_capacity(draft.base_sites.len());
    for site in &draft.base_sites {
        if site.steel_patches > MAX_STEEL_PATCHES_PER_BASE
            || site.oil_patches > MAX_OIL_PATCHES_PER_BASE
        {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: "map base resource count is out of range".to_string(),
            });
        }
        if counts
            .insert(
                (site.x, site.y),
                BaseResourceCounts {
                    steel_patches: site.steel_patches,
                    oil_patches: site.oil_patches,
                },
            )
            .is_some()
        {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: "map contains duplicate base sites".to_string(),
            });
        }
    }
    Ok(counts)
}

pub(super) fn canonical_doodads(
    width: u32,
    height: u32,
    records: Vec<MapDoodad>,
    name: &str,
) -> Result<Vec<MapDoodad>, LabError> {
    doodads::canonicalize(width, height, records).map_err(|reason| LabError::InvalidMap {
        name: name.to_string(),
        reason,
    })
}

pub(super) fn canonical_overlays(
    draft: &LabMapDraft,
    name: &str,
) -> Result<MapOverlayTiles<(u32, u32)>, LabError> {
    Ok(MapOverlayTiles {
        concealment: canonical_tiles(
            &draft.concealment_tiles,
            draft.width,
            draft.height,
            "concealmentTiles",
            name,
        )?,
        no_vehicle: canonical_tiles(
            &draft.no_vehicle_tiles,
            draft.width,
            draft.height,
            "noVehicleTiles",
            name,
        )?,
        no_building: canonical_tiles(
            &draft.no_building_tiles,
            draft.width,
            draft.height,
            "noBuildingTiles",
            name,
        )?,
        no_entrenchment: canonical_tiles(
            &draft.no_entrenchment_tiles,
            draft.width,
            draft.height,
            "noEntrenchmentTiles",
            name,
        )?,
        damage_reduction: canonical_tiles(
            &draft.damage_reduction_tiles,
            draft.width,
            draft.height,
            "damageReductionTiles",
            name,
        )?,
        slow_movement: canonical_tiles(
            &draft.slow_movement_tiles,
            draft.width,
            draft.height,
            "slowMovementTiles",
            name,
        )?,
    })
}

fn canonical_tiles(
    tiles: &[MapTile],
    width: u32,
    height: u32,
    field: &str,
    name: &str,
) -> Result<Vec<(u32, u32)>, LabError> {
    let mut out = Vec::with_capacity(tiles.len());
    for (index, tile) in tiles.iter().enumerate() {
        if tile.x >= width || tile.y >= height {
            return Err(LabError::InvalidMap {
                name: name.to_string(),
                reason: format!("{field}[{index}] is outside the map"),
            });
        }
        out.push((tile.x, tile.y));
    }
    out.sort_unstable();
    if out.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(LabError::InvalidMap {
            name: name.to_string(),
            reason: format!("{field} contains duplicate tiles"),
        });
    }
    Ok(out)
}
