use std::collections::HashMap;

use crate::game::map::{BaseResourceCounts, Map};
use crate::protocol::{LabBaseSite, LabMapDraft, LabMapTile, MapTile};
use rts_protocol::{MAX_OIL_PATCHES_PER_BASE, MAX_STEEL_PATCHES_PER_BASE};

use super::LabError;

pub(super) fn export(map: &Map, name: &str) -> LabMapDraft {
    let (stealth_tiles, no_vehicle_tiles) = map.protocol_overlay_tiles();
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
        stealth_tiles,
        no_vehicle_tiles,
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

pub(super) fn canonical_overlays(
    draft: &LabMapDraft,
    name: &str,
) -> Result<(Vec<(u32, u32)>, Vec<(u32, u32)>), LabError> {
    Ok((
        canonical_tiles(
            &draft.stealth_tiles,
            draft.width,
            draft.height,
            "stealthTiles",
            name,
        )?,
        canonical_tiles(
            &draft.no_vehicle_tiles,
            draft.width,
            draft.height,
            "noVehicleTiles",
            name,
        )?,
    ))
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
