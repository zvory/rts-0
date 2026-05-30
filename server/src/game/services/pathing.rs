//! PathingService: the single boundary for all pathfinding requests.
//!
//! Encapsulates unit mobility class, terrain mask, dynamic blockers, radius/footprint,
//! per-request path budget, and an LRU cache of verified tile paths so multiple units or
//! ticks can reuse A* results.

use std::collections::HashMap;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::{Map, MobilityClass};
use crate::game::pathfinding::{self, Passability};
use crate::game::services::occupancy::Occupancy;

impl MobilityClass {
    /// Derive the mobility class from an entity kind.
    pub fn from_kind(kind: EntityKind) -> Self {
        match kind {
            EntityKind::Tank => MobilityClass::Vehicle,
            _ if kind.is_unit() => MobilityClass::Infantry,
            _ => MobilityClass::Infantry,
        }
    }
}

/// Parameters for a single path query.
#[derive(Clone)]
pub struct PathRequest {
    /// Start tile (inclusive).
    pub start: (i32, i32),
    /// Goal tile (inclusive).
    pub goal: (i32, i32),
    /// Unit mobility class.
    pub class: MobilityClass,
    /// Unit radius in tiles for clearance. `0` means point-sized (current behavior).
    pub radius_tiles: u32,
    /// Max A* nodes to expand. `None` uses the service default.
    pub budget: Option<usize>,
}

/// Passability oracle that layers class-specific rules over terrain + occupancy.
struct ClassPassability<'a> {
    map: &'a Map,
    occupancy: &'a Occupancy<'a>,
    class: MobilityClass,
    radius_tiles: u32,
}

impl<'a> ClassPassability<'a> {
    fn tile_passable(&self, tx: i32, ty: i32) -> bool {
        if !self.map.in_bounds(tx, ty) {
            return false;
        }
        if !self.map.is_passable_for(self.class, tx, ty) {
            return false;
        }
        if !self.occupancy.passable(tx, ty) {
            return false;
        }
        true
    }
}

impl<'a> Passability for ClassPassability<'a> {
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

type CacheKey = ((i32, i32), (i32, i32), MobilityClass, u32);

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
        let pass = ClassPassability {
            map,
            occupancy,
            class: req.class,
            radius_tiles: req.radius_tiles,
        };

        if let Some(tile_path) = self.cache_lookup(&req, &pass) {
            return pathfinding::to_world_waypoints(&tile_path);
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

        let waypoints = pathfinding::to_world_waypoints(&tile_path);
        self.cache_insert(req.start, req.goal, req.class, req.radius_tiles, tile_path);
        waypoints
    }

    /// Convenience: re-path a single entity toward a world-pixel goal, snapping the final
    /// waypoint to the exact goal point (mirrors the old `repath` helper).
    pub fn repath_entity(
        &mut self,
        map: &Map,
        entities: &mut EntityStore,
        occupancy: &Occupancy,
        id: u32,
        gx: f32,
        gy: f32,
    ) {
        let (sx, sy) = match entities.get(id) {
            Some(e) => map.tile_of(e.pos_x, e.pos_y),
            None => return,
        };
        let (gtx, gty) = map.tile_of(gx, gy);
        let kind = match entities.get(id) {
            Some(e) => e.kind,
            None => return,
        };
        let radius_tiles = config::unit_stats(kind)
            .map(|s| s.radius_tiles())
            .unwrap_or(0);
        let req = PathRequest {
            start: (sx as i32, sy as i32),
            goal: (gtx as i32, gty as i32),
            class: MobilityClass::from_kind(kind),
            radius_tiles,
            budget: None,
        };
        let mut waypoints = self.request(map, occupancy, req);
        if !waypoints.is_empty() {
            waypoints[0] = (gx, gy);
        } else {
            waypoints = vec![(gx, gy)];
        }
        if let Some(e) = entities.get_mut(id) {
            e.path = waypoints;
        }
    }

    fn cache_lookup<P: Passability>(
        &mut self,
        req: &PathRequest,
        pass: &P,
    ) -> Option<Vec<(i32, i32)>> {
        let key: CacheKey = (req.start, req.goal, req.class, req.radius_tiles);
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
        start: (i32, i32),
        goal: (i32, i32),
        class: MobilityClass,
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
            (start, goal, class, radius),
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
        start: (i32, i32),
        goal: (i32, i32),
        class: MobilityClass,
        radius: u32,
    ) -> bool {
        self.cache.contains_key(&(start, goal, class, radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;
    use crate::game::map::Map;
    use crate::game::services::occupancy::Occupancy;

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

        let reqs = [
            ((1, 1), (2, 2)),
            ((1, 1), (3, 3)),
            ((2, 2), (4, 4)),
        ];
        for (start, goal) in &reqs {
            let req = PathRequest {
                start: *start,
                goal: *goal,
                class: MobilityClass::Infantry,
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
            start: (1, 1),
            goal: (5, 5),
            class: MobilityClass::Infantry,
            radius_tiles: 0,
            budget: None,
        };
        a.request(&map, &occ, req4.clone());
        b.request(&map, &occ, req4.clone());

        assert_eq!(a.cache_len(), 3);
        assert_eq!(b.cache_len(), 3);

        // Both instances should have evicted the same key: the one with the
        // smallest (start, goal, class, radius) tuple.
        let evicted = ((1, 1), (2, 2), MobilityClass::Infantry, 0);
        assert!(!a.cache_contains(evicted.0, evicted.1, evicted.2, evicted.3));
        assert!(!b.cache_contains(evicted.0, evicted.1, evicted.2, evicted.3));

        // And both should still contain the other three.
        for (start, goal) in &[((1, 1), (3, 3)), ((2, 2), (4, 4)), ((1, 1), (5, 5))] {
            assert!(a.cache_contains(*start, *goal, MobilityClass::Infantry, 0));
            assert!(b.cache_contains(*start, *goal, MobilityClass::Infantry, 0));
        }
    }
}
