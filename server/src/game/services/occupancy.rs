use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;
use crate::game::services::spatial::SpatialIndex;

/// A snapshot of which tiles are blocked by buildings this tick, layered over terrain. Units
/// never block (soft overlap is allowed), so only static structures appear here.
pub(crate) struct Occupancy<'a> {
    map: &'a Map,
    blocked: Vec<bool>,
}

impl<'a> Occupancy<'a> {
    pub(crate) fn build(map: &'a Map, entities: &EntityStore) -> Self {
        let size = map.size;
        let mut blocked = vec![false; (size * size) as usize];
        for e in entities.iter() {
            if !e.is_building() {
                continue;
            }
            for (tx, ty) in building_footprint(map, e) {
                if tx < size && ty < size {
                    blocked[(ty * size + tx) as usize] = true;
                }
            }
        }
        Occupancy { map, blocked }
    }
}

impl Passability for Occupancy<'_> {
    /// Building footprints only — terrain passability is checked separately by callers
    /// so that movement classes (infantry vs vehicle) can be applied.
    fn passable(&self, tx: i32, ty: i32) -> bool {
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        !self.blocked[(ty * self.map.size as i32 + tx) as usize]
    }
}

/// The set of tiles a building's footprint covers, centered on its position. Footprints are
/// `foot_w × foot_h`; we center them on the tile under the building center.
pub(crate) fn building_footprint(map: &Map, e: &Entity) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(e.kind) else {
        return Vec::new();
    };
    let (cx, cy) = map.tile_of(e.pos_x, e.pos_y);
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    // Offsets so the footprint is centered on the building's tile.
    let ox = s.foot_w as i32 / 2;
    let oy = s.foot_h as i32 / 2;
    for dy in 0..s.foot_h as i32 {
        for dx in 0..s.foot_w as i32 {
            let tx = cx as i32 + dx - ox;
            let ty = cy as i32 + dy - oy;
            if tx >= 0 && ty >= 0 {
                out.push((tx as u32, ty as u32));
            }
        }
    }
    out
}

/// The tiles a footprint of `building` would cover if its top-left tile were `(tile_x,
/// tile_y)`. The command specifies the top-left tile of the footprint.
pub(crate) fn footprint_tiles(building: EntityKind, tile_x: u32, tile_y: u32) -> Vec<(u32, u32)> {
    let Some(s) = config::building_stats(building) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity((s.foot_w * s.foot_h) as usize);
    for dy in 0..s.foot_h {
        for dx in 0..s.foot_w {
            // Guard against coordinate overflow on huge tile_x/tile_y. An empty footprint is
            // treated as not-placeable by `footprint_placeable`, so the build is cleanly rejected.
            let (Some(tx), Some(ty)) = (tile_x.checked_add(dx), tile_y.checked_add(dy)) else {
                return Vec::new();
            };
            out.push((tx, ty));
        }
    }
    out
}

/// World-pixel center of a footprint placed at top-left tile `(tile_x, tile_y)`.
pub(crate) fn footprint_center(
    map: &Map,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> (f32, f32) {
    let Some(s) = config::building_stats(building) else {
        return (0.0, 0.0);
    };
    let ts = config::TILE_SIZE as f32;
    let x = tile_x as f32 * ts + (s.foot_w as f32 * ts) * 0.5;
    let y = tile_y as f32 * ts + (s.foot_h as f32 * ts) * 0.5;
    // map is unused beyond stats here, kept for signature symmetry / future clamping.
    let _ = map;
    (x, y)
}

/// Whether `building`'s footprint at `(tile_x, tile_y)` is fully in bounds, on passable
/// terrain, and clear of existing building footprints and resource nodes. `(tile_x, tile_y)` is
/// the footprint's top-left tile. Shared with the AI (`ai.rs`) for picking valid build sites.
pub(crate) fn footprint_placeable(
    map: &Map,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> bool {
    let tiles = footprint_tiles(building, tile_x, tile_y);
    if tiles.is_empty() {
        return false;
    }
    // In bounds + passable terrain.
    for &(tx, ty) in &tiles {
        if !map.in_bounds(tx as i32, ty as i32) {
            return false;
        }
        if !map.is_passable(tx as i32, ty as i32) {
            return false;
        }
    }

    // Not overlapping another building's footprint or a resource node tile.
    // Use the spatial index to only check entities near the footprint.
    let stats = match config::building_stats(building) {
        Some(s) => s,
        None => return false,
    };
    let max_dim = stats.foot_w.max(stats.foot_h) as i32;
    let min_tx = tile_x as i32;
    let min_ty = tile_y as i32;
    let max_tx = tile_x as i32 + max_dim - 1;
    let max_ty = tile_y as i32 + max_dim - 1;

    for id in spatial.ids_in_rect(min_tx, min_ty, max_tx, max_ty) {
        if let Some(e) = entities.get(id) {
            if e.is_building() {
                let occupied = building_footprint(map, e);
                for t in &tiles {
                    if occupied.contains(t) {
                        return false;
                    }
                }
            } else if e.is_node() {
                let node_tile = map.tile_of(e.pos_x, e.pos_y);
                if tiles.contains(&node_tile) {
                    return false;
                }
            }
        }
    }
    true
}
