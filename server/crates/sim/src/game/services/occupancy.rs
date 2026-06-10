use std::collections::VecDeque;

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// A snapshot of which tiles are blocked by buildings this tick, layered over terrain. Units
/// never block (soft overlap is allowed), so only static structures appear here.
pub(crate) struct Occupancy<'a> {
    map: &'a Map,
    blocked: Vec<bool>,
    clearance_tiles: Vec<u16>,
    static_fingerprint: u64,
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
        let mut static_blocked = vec![false; (size * size) as usize];
        for ty in 0..size {
            for tx in 0..size {
                let idx = (ty * size + tx) as usize;
                static_blocked[idx] = blocked[idx] || !map.is_passable(tx as i32, ty as i32);
            }
        }
        let clearance_tiles = build_clearance_field(map, &static_blocked);
        let static_fingerprint = static_blocked_fingerprint(size, &static_blocked);

        Occupancy {
            map,
            blocked,
            clearance_tiles,
            static_fingerprint,
        }
    }

    /// Tile clearance from the nearest static blocker, in whole tiles. Blocked and out-of-bounds
    /// tiles report zero. Map edges count as static bounds, so edge-adjacent tiles have low
    /// clearance even on otherwise empty maps.
    pub(crate) fn clearance_at_tile(&self, tx: i32, ty: i32) -> u16 {
        if !self.map.in_bounds(tx, ty) {
            return 0;
        }
        self.clearance_tiles[(ty as u32 * self.map.size + tx as u32) as usize]
    }

    /// Clearance at the tile containing a world-pixel point.
    #[allow(dead_code)]
    pub(crate) fn clearance_near_world_point(&self, x: f32, y: f32) -> u16 {
        if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
            return 0;
        }
        let world_size = self.map.world_size_px();
        if x >= world_size || y >= world_size {
            return 0;
        }
        let ts = config::TILE_SIZE as f32;
        self.clearance_at_tile((x / ts).floor() as i32, (y / ts).floor() as i32)
    }

    /// Minimum static clearance sampled along a world-pixel segment.
    #[allow(dead_code)]
    pub(crate) fn min_clearance_along_segment(&self, from: (f32, f32), to: (f32, f32)) -> u16 {
        if !from.0.is_finite() || !from.1.is_finite() || !to.0.is_finite() || !to.1.is_finite() {
            return 0;
        }

        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let distance = (dx * dx + dy * dy).sqrt();
        if !distance.is_finite() {
            return 0;
        }
        let step_px = config::TILE_SIZE as f32 / 4.0;
        let steps = (distance / step_px).ceil().max(1.0) as u32;
        let mut min_clearance = u16::MAX;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = from.0 + dx * t;
            let y = from.1 + dy * t;
            min_clearance = min_clearance.min(self.clearance_near_world_point(x, y));
            if min_clearance == 0 {
                break;
            }
        }

        min_clearance
    }

    /// Fingerprint of the static blocker layer used to keep path-cache entries scoped to the
    /// terrain/building clearance field that produced them.
    pub(crate) fn static_fingerprint(&self) -> u64 {
        self.static_fingerprint
    }

    pub(crate) fn building_blocked_at_tile(&self, tx: i32, ty: i32) -> bool {
        let size = self.map.size as i32;
        if tx < 0 || ty < 0 || tx >= size || ty >= size {
            return false;
        }
        self.blocked[(ty * self.map.size as i32 + tx) as usize]
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

fn build_clearance_field(map: &Map, static_blocked: &[bool]) -> Vec<u16> {
    let size = map.size as i32;
    let len = (map.size * map.size) as usize;
    let mut clearance = vec![u16::MAX; len];
    let mut queue = VecDeque::new();

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            if static_blocked[idx] {
                clearance[idx] = 0;
                queue.push_back((tx, ty));
            }
        }
    }

    while let Some((tx, ty)) = queue.pop_front() {
        let idx = (ty as u32 * map.size + tx as u32) as usize;
        let next_clearance = clearance[idx].saturating_add(1);
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = tx + dx;
                let ny = ty + dy;
                if !map.in_bounds(nx, ny) {
                    continue;
                }
                let nidx = (ny as u32 * map.size + nx as u32) as usize;
                if next_clearance < clearance[nidx] {
                    clearance[nidx] = next_clearance;
                    queue.push_back((nx, ny));
                }
            }
        }
    }

    for ty in 0..size {
        for tx in 0..size {
            let idx = (ty as u32 * map.size + tx as u32) as usize;
            let edge_clearance = (tx + 1).min(ty + 1).min(size - tx).min(size - ty) as u16;
            clearance[idx] = clearance[idx].min(edge_clearance);
        }
    }

    clearance
}

fn static_blocked_fingerprint(size: u32, static_blocked: &[bool]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    hash = fnv_mix(hash, size as u64);
    for (idx, blocked) in static_blocked.iter().enumerate() {
        if *blocked {
            hash = fnv_mix(hash, idx as u64 + 1);
        }
    }
    hash
}

fn fnv_mix(hash: u64, value: u64) -> u64 {
    (hash ^ value).wrapping_mul(FNV_PRIME)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::terrain;

    fn flat_test_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![],
            expansion_sites: vec![],
        }
    }

    #[test]
    fn clearance_is_zero_on_static_blocked_tiles() {
        let mut map = flat_test_map(10);
        let rock = map.index(4, 4);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert_eq!(occ.clearance_at_tile(4, 4), 0);
        assert_eq!(occ.clearance_at_tile(-1, 4), 0);
        assert_eq!(occ.clearance_at_tile(10, 4), 0);
    }

    #[test]
    fn clearance_increases_away_from_terrain_blockers() {
        let mut map = flat_test_map(12);
        let rock = map.index(4, 4);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        assert_eq!(occ.clearance_at_tile(5, 4), 1);
        assert_eq!(occ.clearance_at_tile(6, 4), 2);
        assert_eq!(occ.clearance_at_tile(7, 4), 3);
    }

    #[test]
    fn building_occupancy_updates_clearance_and_fingerprint() {
        let map = flat_test_map(12);
        let empty = EntityStore::new();
        let before = Occupancy::build(&map, &empty);
        let clear_before = before.clearance_at_tile(6, 4);
        let fingerprint_before = before.static_fingerprint();

        let mut entities = EntityStore::new();
        let (bx, by) = footprint_center(&map, EntityKind::Depot, 4, 4);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let after = Occupancy::build(&map, &entities);

        assert_eq!(after.clearance_at_tile(4, 4), 0);
        assert_eq!(after.clearance_at_tile(5, 5), 0);
        assert!(
            after.clearance_at_tile(6, 4) < clear_before,
            "adjacent clearance should shrink after building placement"
        );
        assert_ne!(after.static_fingerprint(), fingerprint_before);
    }

    #[test]
    fn world_point_and_segment_clearance_sample_static_field() {
        let mut map = flat_test_map(12);
        let rock = map.index(5, 5);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let blocked_center = map.tile_center(5, 5);
        let open_center = map.tile_center(8, 5);

        assert_eq!(
            occ.clearance_near_world_point(blocked_center.0, blocked_center.1),
            0
        );
        assert!(occ.clearance_near_world_point(open_center.0, open_center.1) > 0);
        assert_eq!(
            occ.min_clearance_along_segment(map.tile_center(3, 5), map.tile_center(7, 5)),
            0
        );
        assert!(occ.min_clearance_along_segment(map.tile_center(8, 5), map.tile_center(9, 5)) > 0);
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
