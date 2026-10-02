//! Gameplay overlays from the match-start map: forest (concealment) and no-entrenchment ground.

use rts_sim::protocol::{MapInfo, MapTile};

use super::{fnv_u32, tile_index, AiMapAnalysis, FNV_OFFSET_BASIS};

impl AiMapAnalysis {
    /// Whether units on this tile are concealed from the enemy (forest).
    pub(crate) fn tile_is_concealment(&self, x: u32, y: u32) -> bool {
        tile_index(self.width, self.height, x, y)
            .and_then(|idx| self.concealment.get(idx).copied())
            .unwrap_or(false)
    }

    /// Whether infantry standing here can dig in: passable, not a road and not authored
    /// no-entrenchment ground.
    pub(crate) fn tile_allows_entrenchment(&self, x: u32, y: u32) -> bool {
        tile_index(self.width, self.height, x, y).is_some_and(|idx| {
            self.passable.get(idx).copied().unwrap_or(false)
                && !self.road.get(idx).copied().unwrap_or(true)
                && !self.no_entrenchment.get(idx).copied().unwrap_or(true)
        })
    }
}

/// The gameplay overlays the analysis reads, so two maps that differ only there never share it.
pub(super) fn hash_overlays(map: &MapInfo) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for tiles in [&map.concealment_tiles, &map.no_entrenchment_tiles] {
        hash = fnv_u32(hash, tiles.len() as u32);
        for tile in tiles {
            hash = fnv_u32(hash, tile.x);
            hash = fnv_u32(hash, tile.y);
        }
    }
    hash
}

pub(super) fn overlay_grid(width: u32, height: u32, tiles: &[MapTile]) -> Vec<bool> {
    let mut grid = vec![false; width.saturating_mul(height) as usize];
    for tile in tiles {
        if let Some(idx) = tile_index(width, height, tile.x, tile.y) {
            if let Some(cell) = grid.get_mut(idx) {
                *cell = true;
            }
        }
    }
    grid
}
