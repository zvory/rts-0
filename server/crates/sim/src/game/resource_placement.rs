use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;

pub(crate) type ResourceTile = (u32, u32);

pub(crate) fn nearest_oil_tile_center<F>(
    map: &Map,
    x: f32,
    y: f32,
    mut accepts: F,
) -> Option<(f32, f32, ResourceTile)>
where
    F: FnMut(ResourceTile, f32, f32) -> bool,
{
    let mut best: Option<(f32, u32, u32, f32, f32)> = None;
    for ty in 0..map.size {
        for tx in 0..map.size {
            if !map.is_passable(tx as i32, ty as i32) {
                continue;
            }
            let tile = (tx, ty);
            let (cx, cy) = map.tile_center(tx, ty);
            if !accepts(tile, cx, cy) {
                continue;
            }

            let score = (cx - x).powi(2) + (cy - y).powi(2);
            let replace = match best {
                Some((best_score, best_tx, best_ty, _, _)) => {
                    score < best_score - 0.001
                        || ((score - best_score).abs() <= 0.001
                            && (tile.1, tile.0) < (best_ty, best_tx))
                }
                None => true,
            };
            if replace {
                best = Some((score, tile.0, tile.1, cx, cy));
            }
        }
    }

    best.map(|(_, tx, ty, cx, cy)| (cx, cy, (tx, ty)))
}

pub(crate) fn nearest_tile_center(map: &Map, x: f32, y: f32) -> (f32, f32, ResourceTile) {
    let ts = config::TILE_SIZE as f32;
    let max_tile = map.size.saturating_sub(1) as f32;
    let tx = (x / ts - 0.5).round().clamp(0.0, max_tile) as u32;
    let ty = (y / ts - 0.5).round().clamp(0.0, max_tile) as u32;
    let (cx, cy) = map.tile_center(tx, ty);
    (cx, cy, (tx, ty))
}

pub(crate) fn occupied_oil_tiles(map: &Map, entities: &EntityStore) -> BTreeSet<ResourceTile> {
    entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Oil && entity.is_node())
        .map(|entity| map.tile_of(entity.pos_x, entity.pos_y))
        .collect()
}

pub(crate) fn tile_has_one_tile_oil_gap(
    tile: ResourceTile,
    occupied_tiles: &BTreeSet<ResourceTile>,
) -> bool {
    occupied_tiles
        .iter()
        .all(|&other| oil_tiles_have_one_tile_gap(tile, other))
}

pub(crate) fn oil_tiles_have_one_tile_gap(a: ResourceTile, b: ResourceTile) -> bool {
    a.0.abs_diff(b.0) > 1 || a.1.abs_diff(b.1) > 1
}
