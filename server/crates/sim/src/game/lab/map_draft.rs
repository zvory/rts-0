use std::collections::HashMap;

use crate::game::map::{
    BaseResourceCounts, Map, MAX_OIL_PATCHES_PER_BASE, MAX_STEEL_PATCHES_PER_BASE,
};
use crate::protocol::{LabBaseSite, LabMapDraft, LabMapTile};

use super::LabError;

pub(super) fn export(map: &Map, name: &str) -> LabMapDraft {
    LabMapDraft {
        name: name.to_string(),
        size: map.size,
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
    }
}

pub(super) fn resource_counts(
    draft: &LabMapDraft,
    name: &str,
) -> Result<HashMap<(u32, u32), BaseResourceCounts>, LabError> {
    if draft.base_sites.iter().any(|site| {
        site.steel_patches > MAX_STEEL_PATCHES_PER_BASE
            || site.oil_patches > MAX_OIL_PATCHES_PER_BASE
    }) {
        return Err(LabError::InvalidMap {
            name: name.to_string(),
            reason: "map base resource count is out of range".to_string(),
        });
    }
    Ok(draft
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
        .collect())
}
