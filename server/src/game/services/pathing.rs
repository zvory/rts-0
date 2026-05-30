//! PathingService: the single boundary for all pathfinding requests.
//!
//! Encapsulates unit mobility class, terrain mask, dynamic blockers, radius/footprint,
//! per-request path budget, and an LRU cache of verified tile paths so multiple units or
//! ticks can reuse A* results.

use std::collections::HashMap;

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
    #[allow(dead_code)]
    radius_tiles: u32,
}

impl<'a> Passability for ClassPassability<'a> {
    fn passable(&self, tx: i32, ty: i32) -> bool {
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
        let req = PathRequest {
            start: (sx as i32, sy as i32),
            goal: (gtx as i32, gty as i32),
            class: MobilityClass::from_kind(kind),
            radius_tiles: 0,
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
                .min_by_key(|(_, e)| e.last_used)
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
