//! PathingService: the single boundary for all pathfinding requests.
//!
//! Encapsulates terrain mask, dynamic blockers, radius/footprint,
//! per-request path budget, and an LRU cache of verified non-empty tile paths so multiple units or
//! ticks can reuse A* results.

use std::collections::HashMap;

use crate::game::entity::EntityKind;
use crate::game::map::Map;
use crate::game::pathfinding::{self, Passability};
use crate::game::services::occupancy::Occupancy;
use crate::rules::terrain::{self, TerrainKind};

/// Parameters for a single path query.
#[derive(Clone)]
pub struct PathRequest {
    /// Entity kind being routed.
    pub kind: EntityKind,
    /// Start tile (inclusive).
    pub start: (i32, i32),
    /// Goal tile (inclusive).
    pub goal: (i32, i32),
    /// Unit radius in tiles for clearance. `0` means point-sized (current behavior).
    pub radius_tiles: u32,
    /// Max A* nodes to expand. `None` uses the service default.
    pub budget: Option<usize>,
}

/// Passability oracle that layers terrain + occupancy.
struct TerrainPassability<'a> {
    map: &'a Map,
    occupancy: &'a Occupancy<'a>,
    kind: EntityKind,
    radius_tiles: u32,
}

impl TerrainPassability<'_> {
    fn tile_passable(&self, tx: i32, ty: i32) -> bool {
        if !self.map.in_bounds(tx, ty) {
            return false;
        }
        let Some(terrain_kind) =
            TerrainKind::from_map_code(self.map.terrain_at(tx as u32, ty as u32))
        else {
            return false;
        };
        if !terrain::movement_allowed(self.kind, terrain_kind) {
            return false;
        }
        if !self.occupancy.passable(tx, ty) {
            return false;
        }
        true
    }
}

impl Passability for TerrainPassability<'_> {
    fn passable(&self, tx: i32, ty: i32) -> bool {
        let r = self.radius_tiles as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if !self.tile_passable(tx + dx, ty + dy) {
                    return false;
                }
            }
        }
        true
    }
}

type CacheKey = (EntityKind, (i32, i32), (i32, i32), u32);

struct CacheEntry {
    tile_path: Vec<(i32, i32)>,
    last_used: u32,
}

/// The authoritative pathfinding boundary.
///
/// Holds an LRU cache so multiple units heading to the same destination, or the same unit
/// repathing across ticks, can reuse prior A* work. Cached paths are verified against the
/// current occupancy before reuse.
pub struct PathingService {
    default_budget: usize,
    cache: HashMap<CacheKey, CacheEntry>,
    cache_cap: usize,
    tick: u32,
}

impl PathingService {
    /// Create a new service with the given default budget and cache capacity.
    pub fn new(default_budget: usize, cache_cap: usize) -> Self {
        PathingService {
            default_budget,
            cache: HashMap::with_capacity(cache_cap),
            cache_cap,
            tick: 0,
        }
    }

    /// Advance the internal tick counter. Call once per simulation tick.
    pub fn advance_tick(&mut self, tick: u32) {
        self.tick = tick;
    }

    /// Request a path. Returns world-pixel waypoints in reverse order (next waypoint = pop).
    pub fn request(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
    ) -> Vec<(f32, f32)> {
        let tile_path = self.request_tile_path(map, occupancy, req);
        pathfinding::to_world_waypoints(&tile_path)
    }

    pub(crate) fn request_tile_path(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
    ) -> Vec<(i32, i32)> {
        let pass = TerrainPassability {
            map,
            occupancy,
            kind: req.kind,
            radius_tiles: req.radius_tiles,
        };

        if let Some(tile_path) = self.cache_lookup(&req, &pass) {
            return tile_path;
        }

        let budget = req.budget.unwrap_or(self.default_budget);
        let tile_path = pathfinding::find_path_with_budget(
            &pass,
            req.start.0,
            req.start.1,
            req.goal.0,
            req.goal.1,
            budget,
        );

        if !tile_path.is_empty() {
            self.cache_insert(
                req.kind,
                req.start,
                req.goal,
                req.radius_tiles,
                tile_path.clone(),
            );
        }
        tile_path
    }

    fn cache_lookup<P: Passability>(
        &mut self,
        req: &PathRequest,
        pass: &P,
    ) -> Option<Vec<(i32, i32)>> {
        let key: CacheKey = (req.kind, req.start, req.goal, req.radius_tiles);
        let entry = self.cache.get_mut(&key)?;
        for &(tx, ty) in &entry.tile_path {
            if !pass.passable(tx, ty) {
                return None;
            }
        }
        entry.last_used = self.tick;
        Some(entry.tile_path.clone())
    }

    fn cache_insert(
        &mut self,
        kind: EntityKind,
        start: (i32, i32),
        goal: (i32, i32),
        radius: u32,
        tile_path: Vec<(i32, i32)>,
    ) {
        if self.cache.len() >= self.cache_cap {
            if let Some(oldest_key) = self
                .cache
                .iter()
                .min_by_key(|(k, e)| (e.last_used, *k))
                .map(|(k, _)| *k)
            {
                self.cache.remove(&oldest_key);
            }
        }
        self.cache.insert(
            (kind, start, goal, radius),
            CacheEntry {
                tile_path,
                last_used: self.tick,
            },
        );
    }
}

#[cfg(test)]
impl PathingService {
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    pub fn cache_contains(
        &self,
        kind: EntityKind,
        start: (i32, i32),
        goal: (i32, i32),
        radius: u32,
    ) -> bool {
        self.cache.contains_key(&(kind, start, goal, radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config;
    use crate::game::entity::EntityStore;
    use crate::game::map::Map;
    use crate::game::services::occupancy::Occupancy;
    use crate::game::services::standability;
    use crate::protocol::terrain;

    fn flat_test_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![terrain::GRASS; (size * size) as usize],
            starts: vec![(1, 1)],
            expansion_sites: Vec::new(),
        }
    }

    fn map_with_rock_rect(size: u32, min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Map {
        let mut map = flat_test_map(size);
        for ty in min_y..=max_y {
            for tx in min_x..=max_x {
                let index = map.index(tx, ty);
                map.terrain[index] = terrain::ROCK;
            }
        }
        map
    }

    fn request_fixture_path(
        map: &Map,
        kind: EntityKind,
        start: (i32, i32),
        goal: (i32, i32),
    ) -> (Vec<(i32, i32)>, Vec<(f32, f32)>) {
        let entities = EntityStore::new();
        let occ = Occupancy::build(map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let radius_tiles = config::unit_stats(kind)
            .map(|stats| stats.radius_tiles())
            .unwrap_or(0);
        let req = PathRequest {
            kind,
            start,
            goal,
            radius_tiles,
            budget: None,
        };
        let tile_path = service.request_tile_path(map, &occ, req.clone());
        let world_path = service.request(map, &occ, req);
        (tile_path, world_path)
    }

    fn heading_changes_above(points: &[(f32, f32)], threshold_rad: f32) -> usize {
        points
            .windows(3)
            .filter(|triple| {
                let a = segment_angle(triple[0], triple[1]);
                let b = segment_angle(triple[1], triple[2]);
                angle_delta(a, b).abs() > threshold_rad
            })
            .count()
    }

    fn segment_angle(from: (f32, f32), to: (f32, f32)) -> f32 {
        (to.1 - from.1).atan2(to.0 - from.0)
    }

    fn angle_delta(from: f32, to: f32) -> f32 {
        (to - from + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
    }

    fn straight_segment_standable_for_test(
        map: &Map,
        occ: &Occupancy,
        kind: EntityKind,
        from: (f32, f32),
        to: (f32, f32),
    ) -> bool {
        let dx = to.0 - from.0;
        let dy = to.1 - from.1;
        let distance = (dx * dx + dy * dy).sqrt();
        let step_px = config::TILE_SIZE as f32 / 4.0;
        let steps = (distance / step_px).ceil().max(1.0) as u32;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let x = from.0 + dx * t;
            let y = from.1 + dy * t;
            if !standability::unit_static_standable(map, occ, kind, x, y) {
                return false;
            }
        }
        true
    }

    #[test]
    fn path_cache_eviction_is_deterministic_across_instances() {
        // Use a small empty map so most path requests are valid and cached.
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        // Two fresh services have independent HashMap seeds, so their internal
        // bucket orders differ. If eviction only compared last_used, ties could
        // resolve differently between the two instances.
        let mut a = PathingService::new(1_000, 3);
        let mut b = PathingService::new(1_000, 3);
        a.advance_tick(1);
        b.advance_tick(1);

        let reqs = [((1, 1), (2, 2)), ((1, 1), (3, 3)), ((2, 2), (4, 4))];
        for (start, goal) in &reqs {
            let req = PathRequest {
                kind: EntityKind::Worker,
                start: *start,
                goal: *goal,
                radius_tiles: 0,
                budget: None,
            };
            a.request(&map, &occ, req.clone());
            b.request(&map, &occ, req.clone());
        }

        assert_eq!(a.cache_len(), 3);
        assert_eq!(b.cache_len(), 3);

        // This 4th insert triggers eviction (capacity is 3). All entries have
        // last_used == 1, so the tie-breaker is the cache key itself.
        let req4 = PathRequest {
            kind: EntityKind::Worker,
            start: (1, 1),
            goal: (5, 5),
            radius_tiles: 0,
            budget: None,
        };
        a.request(&map, &occ, req4.clone());
        b.request(&map, &occ, req4.clone());

        assert_eq!(a.cache_len(), 3);
        assert_eq!(b.cache_len(), 3);

        // Both instances should have evicted the same key: the one with the
        // smallest (start, goal, radius) tuple.
        let evicted = ((1, 1), (2, 2), 0u32);
        assert!(!a.cache_contains(EntityKind::Worker, evicted.0, evicted.1, evicted.2));
        assert!(!b.cache_contains(EntityKind::Worker, evicted.0, evicted.1, evicted.2));

        // And both should still contain the other three.
        for (start, goal) in &[((1, 1), (3, 3)), ((2, 2), (4, 4)), ((1, 1), (5, 5))] {
            assert!(a.cache_contains(EntityKind::Worker, *start, *goal, 0));
            assert!(b.cache_contains(EntityKind::Worker, *start, *goal, 0));
        }
    }

    #[test]
    fn empty_failed_paths_are_not_cached() {
        let map = Map::generate(1, 0x1234_5678);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        let mut service = PathingService::new(1_000, 3);
        let start = (1, 1);
        let goal = (5, 5);

        let failed = service.request(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Worker,
                start,
                goal,
                radius_tiles: 0,
                budget: Some(0),
            },
        );
        assert!(failed.is_empty());
        assert_eq!(service.cache_len(), 0);
        assert!(!service.cache_contains(EntityKind::Worker, start, goal, 0));

        let found = service.request(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Worker,
                start,
                goal,
                radius_tiles: 0,
                budget: None,
            },
        );
        assert!(!found.is_empty());
        assert!(service.cache_contains(EntityKind::Worker, start, goal, 0));
    }

    #[test]
    fn open_tank_route_keeps_every_tile_center_waypoint_before_smoothing() {
        let map = flat_test_map(40);
        let start = (4, 4);
        let goal = (28, 17);
        let (tile_path, world_path) = request_fixture_path(&map, EntityKind::Tank, start, goal);

        assert_eq!(
            tile_path.len(),
            world_path.len(),
            "world waypoint count should mirror original tile path length in phase 0"
        );
        assert!(
            tile_path.len() >= 20,
            "long open tank route should expose many tile-center waypoints before smoothing, got {}",
            tile_path.len()
        );

        let forward_world: Vec<_> =
            std::iter::once(map.tile_center(start.0 as u32, start.1 as u32))
                .chain(world_path.iter().rev().copied())
                .collect();
        let heading_changes = heading_changes_above(&forward_world, 10.0_f32.to_radians());
        assert!(
            heading_changes >= 1,
            "mixed diagonal/cardinal tile route should contain heading changes above 10 degrees"
        );

        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        assert!(
            straight_segment_standable_for_test(
                &map,
                &occ,
                EntityKind::Tank,
                map.tile_center(start.0 as u32, start.1 as u32),
                world_path[0],
            ),
            "fixture should contain a legal straight segment from start to the final waypoint"
        );
    }

    #[test]
    fn obstacle_route_keeps_corner_waypoint_before_smoothing() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start = (4, 7);
        let goal = (13, 7);
        let (tile_path, world_path) = request_fixture_path(&map, EntityKind::Rifleman, start, goal);

        assert!(!tile_path.is_empty(), "fixture route should be reachable");
        assert_eq!(
            tile_path.len(),
            world_path.len(),
            "world waypoint count should mirror original tile path length around blockers"
        );
        assert!(
            tile_path
                .iter()
                .any(|tile| matches!(tile, (6, 5) | (11, 5) | (6, 9) | (11, 9))),
            "route around the rectangular blocker should retain a corner waypoint, got {tile_path:?}"
        );

        let forward_world: Vec<_> =
            std::iter::once(map.tile_center(start.0 as u32, start.1 as u32))
                .chain(world_path.iter().rev().copied())
                .collect();
        let heading_changes = heading_changes_above(&forward_world, 10.0_f32.to_radians());
        assert!(
            heading_changes >= 2,
            "rectangular obstacle route should include at least two heading changes, got {heading_changes}"
        );

        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        assert!(
            !straight_segment_standable_for_test(
                &map,
                &occ,
                EntityKind::Rifleman,
                map.tile_center(start.0 as u32, start.1 as u32),
                map.tile_center(goal.0 as u32, goal.1 as u32),
            ),
            "direct segment across the rock rectangle should be illegal for later smoothing tests"
        );
    }
}
