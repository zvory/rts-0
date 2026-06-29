use std::collections::BTreeSet;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_footprint, circle_intersects_rect, CircleBody,
};

pub(crate) type ResourceTile = (u32, u32);

pub(crate) fn nearest_resource_tile_center<F>(
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

pub(crate) fn nearest_oil_patch_tile_center(
    map: &Map,
    x: f32,
    y: f32,
    anchor_x: f32,
    anchor_y: f32,
    occupied_oil_tiles: &BTreeSet<ResourceTile>,
    blocked_pump_jack_tiles: &BTreeSet<ResourceTile>,
) -> (f32, f32, ResourceTile) {
    let accepts = |tile: ResourceTile, center_x: f32, center_y: f32, enforce_cc_distance: bool| {
        if !tile_has_one_tile_resource_gap(tile, occupied_oil_tiles) {
            return false;
        }
        if blocked_pump_jack_tiles.contains(&tile) {
            return false;
        }
        if !enforce_cc_distance {
            return true;
        }

        let ts = config::TILE_SIZE as f32;
        let dist_tiles =
            ((center_x - anchor_x).powi(2) + (center_y - anchor_y).powi(2)).sqrt() / ts;
        (config::CC_RESOURCE_MIN_DIST_TILES..=config::CC_RESOURCE_MAX_DIST_TILES)
            .contains(&dist_tiles)
    };

    nearest_resource_tile_center(map, x, y, |tile, cx, cy| {
        accepts(tile, cx, cy, true)
    })
    .or_else(|| {
        nearest_resource_tile_center(map, x, y, |tile, cx, cy| {
            accepts(tile, cx, cy, false)
        })
    })
    .unwrap_or_else(|| nearest_tile_center(map, x, y))
}

pub(crate) fn occupied_resource_tiles(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
) -> BTreeSet<ResourceTile> {
    entities
        .iter()
        .filter(|entity| entity.kind == kind && entity.is_node())
        .map(|entity| map.tile_of(entity.pos_x, entity.pos_y))
        .collect()
}

pub(crate) fn tile_has_one_tile_resource_gap(
    tile: ResourceTile,
    occupied_tiles: &BTreeSet<ResourceTile>,
) -> bool {
    occupied_tiles
        .iter()
        .all(|&other| resource_tiles_have_one_tile_gap(tile, other))
}

pub(crate) fn resource_tiles_have_one_tile_gap(a: ResourceTile, b: ResourceTile) -> bool {
    a.0.abs_diff(b.0) > 1 || a.1.abs_diff(b.1) > 1
}

pub(crate) fn resource_blocked_building_tiles(
    map: &Map,
    entities: &EntityStore,
    building_kind: EntityKind,
    excluded_resource_kind: Option<EntityKind>,
) -> BTreeSet<ResourceTile> {
    let mut blocked = BTreeSet::new();
    let Some(stats) = config::building_stats(building_kind) else {
        return blocked;
    };
    if stats.foot_w == 0 || stats.foot_h == 0 {
        return blocked;
    }

    let ts = config::TILE_SIZE as f32;
    let max_tile = map.size.saturating_sub(1) as i32;
    let foot_w = stats.foot_w as i32;
    let foot_h = stats.foot_h as i32;

    for entity in entities.iter().filter(|entity| entity.is_node()) {
        if Some(entity.kind) == excluded_resource_kind {
            continue;
        }
        if !entity.pos_x.is_finite() || !entity.pos_y.is_finite() {
            continue;
        }
        let radius = entity.radius();
        let min_tx = (((entity.pos_x - radius) / ts).floor() as i32 - foot_w).clamp(0, max_tile);
        let min_ty = (((entity.pos_y - radius) / ts).floor() as i32 - foot_h).clamp(0, max_tile);
        let max_tx = (((entity.pos_x + radius) / ts).floor() as i32).clamp(0, max_tile);
        let max_ty = (((entity.pos_y + radius) / ts).floor() as i32).clamp(0, max_tile);
        let circle = CircleBody {
            x: entity.pos_x,
            y: entity.pos_y,
            radius,
        };

        for ty in min_ty..=max_ty {
            for tx in min_tx..=max_tx {
                let tile = (tx as u32, ty as u32);
                if building_rect_for_footprint(building_kind, tile.0, tile.1)
                    .is_some_and(|rect| circle_intersects_rect(circle, rect))
                {
                    blocked.insert(tile);
                }
            }
        }
    }

    blocked
}
