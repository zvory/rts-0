use std::collections::HashMap;

use super::Map;
use crate::config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaseResourceCounts {
    pub steel_patches: u32,
    pub oil_patches: u32,
}

impl Default for BaseResourceCounts {
    fn default() -> Self {
        Self {
            steel_patches: config::STEEL_PATCHES_PER_BASE,
            oil_patches: config::OIL_PATCHES_PER_BASE,
        }
    }
}

/// Empty scaffold used by focused tests and dev fixtures through struct-update syntax.
impl Default for Map {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            terrain: Vec::new(),
            elevation: Vec::new(),
            starts: Vec::new(),
            base_sites: Vec::new(),
            base_resource_counts: HashMap::new(),
            doodads: Vec::new(),
            concealment_tiles: Vec::new(),
            no_vehicle_tiles: Vec::new(),
            no_building_tiles: Vec::new(),
            no_entrenchment_tiles: Vec::new(),
            damage_reduction_tiles: Vec::new(),
            slow_movement_tiles: Vec::new(),
        }
    }
}

impl Map {
    pub(crate) fn resource_counts_at(&self, tile: (u32, u32)) -> BaseResourceCounts {
        self.base_resource_counts
            .get(&tile)
            .copied()
            .unwrap_or_default()
    }
}
