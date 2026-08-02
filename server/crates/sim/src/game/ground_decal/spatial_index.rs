use std::collections::{BTreeMap, BTreeSet};

use crate::config;

use super::GroundDecal;

#[derive(Debug, Clone, Default)]
pub(super) struct GroundDecalSpatialIndex {
    tiles: BTreeMap<u32, Vec<u32>>,
    ready: bool,
}

impl GroundDecalSpatialIndex {
    pub(super) fn new() -> Self {
        Self {
            tiles: BTreeMap::new(),
            ready: true,
        }
    }

    pub(super) fn ensure(&mut self, decals: &[GroundDecal]) {
        if self.ready {
            return;
        }
        self.tiles.clear();
        for decal in decals {
            self.add(decal);
        }
        self.ready = true;
    }

    pub(super) fn add(&mut self, decal: &GroundDecal) {
        for key in spatial_tile_keys(decal) {
            self.tiles.entry(key).or_default().push(decal.id);
        }
    }

    pub(super) fn candidates(&self, tx: u32, ty: u32) -> Option<&[u32]> {
        self.tiles.get(&tile_key(tx, ty)).map(Vec::as_slice)
    }
}

fn tile_key(tx: u32, ty: u32) -> u32 {
    ty << 16 | tx
}

fn spatial_tile_keys(decal: &GroundDecal) -> BTreeSet<u32> {
    let tile_size = config::TILE_SIZE as f32;
    let center_tx = (decal.x / tile_size).floor() as i32;
    let center_ty = (decal.y / tile_size).floor() as i32;
    let mut keys = BTreeSet::new();
    if center_tx >= 0 && center_ty >= 0 {
        keys.insert(tile_key(center_tx as u32, center_ty as u32));
    }
    let Some(radius_tiles) = decal.radius_tiles else {
        return keys;
    };
    let radius_px = radius_tiles * tile_size;
    let tile_radius = radius_tiles.ceil() as i32;
    for dy in -tile_radius..=tile_radius {
        for dx in -tile_radius..=tile_radius {
            let tx = center_tx + dx;
            let ty = center_ty + dy;
            if tx < 0 || ty < 0 {
                continue;
            }
            let x = (tx as f32 + 0.5) * tile_size;
            let y = (ty as f32 + 0.5) * tile_size;
            let offset_x = x - decal.x;
            let offset_y = y - decal.y;
            if offset_x * offset_x + offset_y * offset_y <= radius_px * radius_px {
                keys.insert(tile_key(tx as u32, ty as u32));
            }
        }
    }
    keys
}
