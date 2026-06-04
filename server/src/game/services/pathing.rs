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
use crate::game::services::standability;
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
    /// Optional route-shaping cost model. Keep normal for interaction paths where exact tile
    /// progression matters more than visual smoothness.
    pub route_shape: RouteShape,
    /// Max A* nodes to expand. `None` uses the service default.
    pub budget: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteShape {
    Normal,
    PreferFewerTurns,
}

impl RouteShape {
    fn turn_penalty(self) -> u32 {
        match self {
            RouteShape::Normal => 0,
            RouteShape::PreferFewerTurns => 3,
        }
    }
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

type CacheKey = (EntityKind, (i32, i32), (i32, i32), u32, RouteShape);

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
        let tile_path = pathfinding::find_path_with_budget_and_turn_cost(
            &pass,
            req.start.0,
            req.start.1,
            req.goal.0,
            req.goal.1,
            budget,
            req.route_shape.turn_penalty(),
        );

        if !tile_path.is_empty() {
            self.cache_insert(
                req.kind,
                req.start,
                req.goal,
                req.radius_tiles,
                req.route_shape,
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
        let key: CacheKey = (
            req.kind,
            req.start,
            req.goal,
            req.radius_tiles,
            req.route_shape,
        );
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
        route_shape: RouteShape,
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
            (kind, start, goal, radius, route_shape),
            CacheEntry {
                tile_path,
                last_used: self.tick,
            },
        );
    }
}

/// Simplify reverse-ordered world waypoints by dropping intermediate tile centers when the unit
/// body can travel straight to a later waypoint without clipping static terrain or buildings.
pub(crate) fn simplify_reverse_waypoints(
    map: &Map,
    occupancy: &Occupancy,
    kind: EntityKind,
    start: (f32, f32),
    waypoints: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    if waypoints.len() <= 1 {
        return waypoints;
    }

    let forward: Vec<(f32, f32)> = waypoints.iter().rev().copied().collect();
    let mut simplified = Vec::with_capacity(forward.len());
    let mut from = start;
    let mut next_index = 0;

    while next_index < forward.len() {
        let mut farthest_legal = None;
        for candidate_index in (next_index..forward.len()).rev() {
            if standability::unit_static_segment_standable(
                map,
                occupancy,
                kind,
                from,
                forward[candidate_index],
            ) {
                farthest_legal = Some(candidate_index);
                break;
            }
        }

        let keep_index = farthest_legal.unwrap_or(next_index);
        let keep = forward[keep_index];
        simplified.push(keep);
        from = keep;
        next_index = keep_index + 1;
    }

    simplified.reverse();
    simplified
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
        route_shape: RouteShape,
    ) -> bool {
        self.cache
            .contains_key(&(kind, start, goal, radius, route_shape))
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

    fn two_tile_wide_horizontal_corridor() -> Map {
        let size = 8;
        let mut terrain = vec![terrain::ROCK; size * size];
        for y in 3..=4 {
            for x in 1..=6 {
                terrain[y * size + x] = terrain::GRASS;
            }
        }
        Map {
            size: size as u32,
            terrain,
            starts: vec![],
            expansion_sites: vec![],
        }
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
            route_shape: RouteShape::Normal,
            budget: None,
        };
        let tile_path = service.request_tile_path(map, &occ, req.clone());
        let world_path = service.request(map, &occ, req);
        (tile_path, world_path)
    }

    fn request_route_shape_tile_path(
        map: &Map,
        kind: EntityKind,
        start: (i32, i32),
        goal: (i32, i32),
        route_shape: RouteShape,
    ) -> Vec<(i32, i32)> {
        let entities = EntityStore::new();
        let occ = Occupancy::build(map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let radius_tiles = config::unit_stats(kind)
            .map(|stats| stats.radius_tiles())
            .unwrap_or(0);
        service.request_tile_path(
            map,
            &occ,
            PathRequest {
                kind,
                start,
                goal,
                radius_tiles,
                route_shape,
                budget: None,
            },
        )
    }

    fn raw_world_path(
        map: &Map,
        kind: EntityKind,
        start: (i32, i32),
        goal: (i32, i32),
    ) -> Vec<(f32, f32)> {
        request_fixture_path(map, kind, start, goal).1
    }

    fn assert_reverse_segments_standable(
        map: &Map,
        occ: &Occupancy,
        kind: EntityKind,
        start: (f32, f32),
        reverse_waypoints: &[(f32, f32)],
    ) {
        let mut from = start;
        for to in reverse_waypoints.iter().rev().copied() {
            assert!(
                standability::unit_static_segment_standable(map, occ, kind, from, to),
                "segment from {from:?} to {to:?} should be static-standable for {kind:?}"
            );
            from = to;
        }
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

    fn tile_turn_count(start: (i32, i32), path: &[(i32, i32)]) -> usize {
        let mut points = Vec::with_capacity(path.len() + 1);
        points.push(start);
        points.extend_from_slice(path);
        points
            .windows(3)
            .filter(|triple| {
                let a = (
                    (triple[1].0 - triple[0].0).signum(),
                    (triple[1].1 - triple[0].1).signum(),
                );
                let b = (
                    (triple[2].0 - triple[1].0).signum(),
                    (triple[2].1 - triple[1].1).signum(),
                );
                a != b
            })
            .count()
    }

    fn tile_move_cost(start: (i32, i32), path: &[(i32, i32)]) -> u32 {
        let mut cost = 0;
        let mut prev = start;
        for &next in path {
            let dx = (next.0 - prev.0).abs();
            let dy = (next.1 - prev.1).abs();
            cost += if dx != 0 && dy != 0 { 14 } else { 10 };
            prev = next;
        }
        cost
    }

    fn assert_tile_path_passable(map: &Map, path: &[(i32, i32)]) {
        for &(tx, ty) in path {
            assert!(
                map.is_passable(tx, ty),
                "tile path should not cross impassable terrain, got blocked tile ({tx}, {ty}) in {path:?}"
            );
        }
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
                route_shape: RouteShape::Normal,
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
            route_shape: RouteShape::Normal,
            budget: None,
        };
        a.request(&map, &occ, req4.clone());
        b.request(&map, &occ, req4.clone());

        assert_eq!(a.cache_len(), 3);
        assert_eq!(b.cache_len(), 3);

        // Both instances should have evicted the same key: the one with the
        // smallest (start, goal, radius) tuple.
        let evicted = ((1, 1), (2, 2), 0u32);
        assert!(!a.cache_contains(
            EntityKind::Worker,
            evicted.0,
            evicted.1,
            evicted.2,
            RouteShape::Normal
        ));
        assert!(!b.cache_contains(
            EntityKind::Worker,
            evicted.0,
            evicted.1,
            evicted.2,
            RouteShape::Normal
        ));

        // And both should still contain the other three.
        for (start, goal) in &[((1, 1), (3, 3)), ((2, 2), (4, 4)), ((1, 1), (5, 5))] {
            assert!(a.cache_contains(EntityKind::Worker, *start, *goal, 0, RouteShape::Normal));
            assert!(b.cache_contains(EntityKind::Worker, *start, *goal, 0, RouteShape::Normal));
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
                route_shape: RouteShape::Normal,
                budget: Some(0),
            },
        );
        assert!(failed.is_empty());
        assert_eq!(service.cache_len(), 0);
        assert!(!service.cache_contains(EntityKind::Worker, start, goal, 0, RouteShape::Normal));

        let found = service.request(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Worker,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert!(!found.is_empty());
        assert!(service.cache_contains(EntityKind::Worker, start, goal, 0, RouteShape::Normal));
    }

    #[test]
    fn tank_turn_cost_prefers_fewer_semi_open_route_turns_than_normal_pathing() {
        let mut map = flat_test_map(24);
        for (tx, ty) in [
            (6, 6),
            (6, 11),
            (6, 15),
            (6, 19),
            (7, 4),
            (7, 6),
            (7, 17),
            (8, 5),
            (8, 14),
            (8, 15),
            (8, 16),
            (9, 4),
            (9, 8),
            (9, 12),
            (9, 16),
            (10, 11),
            (10, 12),
            (10, 14),
            (11, 14),
            (11, 15),
            (12, 4),
            (12, 8),
            (12, 10),
            (13, 13),
            (13, 14),
            (13, 16),
            (14, 4),
            (14, 8),
            (14, 10),
            (14, 16),
            (14, 17),
            (15, 5),
            (15, 6),
            (15, 10),
            (15, 14),
            (15, 15),
            (16, 4),
            (16, 6),
            (16, 9),
            (16, 10),
            (16, 12),
            (16, 14),
            (17, 4),
            (17, 14),
            (17, 16),
            (17, 18),
        ] {
            let index = map.index(tx, ty);
            map.terrain[index] = terrain::ROCK;
        }
        let start = (3, 12);
        let goal = (20, 12);

        let normal =
            request_route_shape_tile_path(&map, EntityKind::Tank, start, goal, RouteShape::Normal);
        let shaped = request_route_shape_tile_path(
            &map,
            EntityKind::Tank,
            start,
            goal,
            RouteShape::PreferFewerTurns,
        );

        assert_eq!(normal.last().copied(), Some(goal));
        assert_eq!(shaped.last().copied(), Some(goal));
        assert_eq!(
            tile_move_cost(start, &shaped),
            tile_move_cost(start, &normal),
            "turn cost should prefer an equally short semi-open route, not a longer detour"
        );
        assert!(
            tile_turn_count(start, &shaped) < tile_turn_count(start, &normal),
            "turn-shaped tank route should reduce heading changes, normal={normal:?} shaped={shaped:?}"
        );
    }

    #[test]
    fn tank_turn_cost_still_finds_route_around_obstacle() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start = (4, 7);
        let goal = (13, 7);

        let shaped = request_route_shape_tile_path(
            &map,
            EntityKind::Tank,
            start,
            goal,
            RouteShape::PreferFewerTurns,
        );

        assert_eq!(shaped.last().copied(), Some(goal));
        assert_tile_path_passable(&map, &shaped);
    }

    #[test]
    fn tank_turn_cost_keeps_required_bend() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start = (4, 7);
        let goal = (13, 7);

        let shaped = request_route_shape_tile_path(
            &map,
            EntityKind::Tank,
            start,
            goal,
            RouteShape::PreferFewerTurns,
        );

        assert!(
            tile_turn_count(start, &shaped) >= 2,
            "route around a rectangular blocker should keep the necessary bends, got {shaped:?}"
        );
        assert!(
            shaped
                .iter()
                .any(|&(_, y)| y < 6 || y > 8),
            "route must leave the blocked row band instead of pretending the direct path is legal, got {shaped:?}"
        );
    }

    #[test]
    fn tank_turn_cost_requests_are_deterministic() {
        let map = map_with_rock_rect(32, 10, 8, 14, 12);
        let start = (5, 10);
        let goal = (22, 14);
        let first = request_route_shape_tile_path(
            &map,
            EntityKind::Tank,
            start,
            goal,
            RouteShape::PreferFewerTurns,
        );

        for _ in 0..5 {
            let next = request_route_shape_tile_path(
                &map,
                EntityKind::Tank,
                start,
                goal,
                RouteShape::PreferFewerTurns,
            );
            assert_eq!(next, first);
        }
    }

    #[test]
    fn route_shape_is_part_of_path_cache_key() {
        let map = flat_test_map(40);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let start = (4, 4);
        let goal = (28, 17);
        let mut service = PathingService::new(8_192, 256);

        for route_shape in [RouteShape::Normal, RouteShape::PreferFewerTurns] {
            let path = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    kind: EntityKind::Tank,
                    start,
                    goal,
                    radius_tiles: 0,
                    route_shape,
                    budget: None,
                },
            );
            assert!(!path.is_empty());
        }

        assert_eq!(service.cache_len(), 2);
        assert!(service.cache_contains(EntityKind::Tank, start, goal, 0, RouteShape::Normal));
        assert!(service.cache_contains(
            EntityKind::Tank,
            start,
            goal,
            0,
            RouteShape::PreferFewerTurns
        ));
    }

    #[test]
    fn tank_turn_cost_respects_expansion_budget() {
        let map = flat_test_map(40);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let start = (4, 4);
        let goal = (28, 17);
        let mut service = PathingService::new(8_192, 256);

        let bounded = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::PreferFewerTurns,
                budget: Some(0),
            },
        );
        assert!(bounded.is_empty());
        assert_eq!(service.cache_len(), 0);

        let unbounded = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::PreferFewerTurns,
                budget: None,
            },
        );
        assert_eq!(unbounded.last().copied(), Some(goal));
        assert!(service.cache_contains(
            EntityKind::Tank,
            start,
            goal,
            0,
            RouteShape::PreferFewerTurns
        ));
    }

    #[test]
    fn simplify_open_diagonal_route_collapses_to_final_waypoint() {
        let map = flat_test_map(40);
        let start_tile = (4, 4);
        let goal_tile = (28, 17);
        let start = map.tile_center(start_tile.0 as u32, start_tile.1 as u32);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let raw = raw_world_path(&map, EntityKind::Tank, start_tile, goal_tile);
        let final_goal = raw[0];

        let smoothed = simplify_reverse_waypoints(&map, &occ, EntityKind::Tank, start, raw.clone());

        assert_eq!(
            smoothed,
            vec![final_goal],
            "open route should collapse to only the final reverse-ordered waypoint"
        );
        assert!(smoothed.len() <= raw.len());
        assert_reverse_segments_standable(&map, &occ, EntityKind::Tank, start, &smoothed);
    }

    #[test]
    fn simplify_route_around_blocker_keeps_corner_waypoint() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start_tile = (4, 7);
        let goal_tile = (13, 7);
        let start = map.tile_center(start_tile.0 as u32, start_tile.1 as u32);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let raw = raw_world_path(&map, EntityKind::Rifleman, start_tile, goal_tile);

        let smoothed =
            simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw.clone());

        assert!(
            smoothed.len() > 1,
            "blocked route should retain at least one waypoint before the final goal"
        );
        assert!(smoothed.len() <= raw.len());
        assert_reverse_segments_standable(&map, &occ, EntityKind::Rifleman, start, &smoothed);

        let forward: Vec<_> = smoothed.iter().rev().copied().collect();
        assert!(
            forward
                .iter()
                .any(|&(_, y)| (y - start.1).abs() > f32::EPSILON),
            "smoothed route should keep a corner detour around the blocker, got {forward:?}"
        );
    }

    #[test]
    fn simplify_preserves_exact_final_command_goal() {
        let map = flat_test_map(32);
        let start_tile = (3, 3);
        let goal_tile = (18, 11);
        let exact_goal_center = map.tile_center(goal_tile.0 as u32, goal_tile.1 as u32);
        let exact_goal = (exact_goal_center.0 + 6.75, exact_goal_center.1 - 4.25);
        let start = map.tile_center(start_tile.0 as u32, start_tile.1 as u32);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut raw = raw_world_path(&map, EntityKind::Rifleman, start_tile, goal_tile);
        raw[0] = exact_goal;

        let smoothed =
            simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw.clone());

        assert_eq!(
            smoothed.first().copied(),
            Some(exact_goal),
            "reverse-ordered index 0 must remain the exact command goal"
        );
        assert!(smoothed.len() <= raw.len());
        assert_reverse_segments_standable(&map, &occ, EntityKind::Rifleman, start, &smoothed);
    }

    #[test]
    fn simplify_never_increases_waypoint_count() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start_tile = (4, 7);
        let goal_tile = (13, 7);
        let start = map.tile_center(start_tile.0 as u32, start_tile.1 as u32);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let raw = raw_world_path(&map, EntityKind::Rifleman, start_tile, goal_tile);

        let smoothed =
            simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw.clone());

        assert!(
            smoothed.len() <= raw.len(),
            "smoothing should only drop waypoints, raw={} smoothed={}",
            raw.len(),
            smoothed.len()
        );
    }

    #[test]
    fn simplify_is_deterministic_across_repeated_calls() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start_tile = (4, 7);
        let goal_tile = (13, 7);
        let start = map.tile_center(start_tile.0 as u32, start_tile.1 as u32);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let raw = raw_world_path(&map, EntityKind::Rifleman, start_tile, goal_tile);

        let first =
            simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw.clone());
        let second =
            simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw.clone());
        let third = simplify_reverse_waypoints(&map, &occ, EntityKind::Rifleman, start, raw);

        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn tank_smoothing_is_stricter_than_infantry_when_radius_matters() {
        let mut map = flat_test_map(12);
        let rock = map.index(5, 5);
        map.terrain[rock] = terrain::ROCK;
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let ts = config::TILE_SIZE as f32;
        let start = (3.5 * ts, 5.0 * ts - 10.0);
        let final_goal = (7.5 * ts, 5.0 * ts - 10.0);
        let detour = (7.5 * ts, 3.5 * ts);
        let raw_reverse = vec![final_goal, detour];

        assert!(standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Rifleman,
            start,
            final_goal,
        ));
        assert!(!standability::unit_static_segment_standable(
            &map,
            &occ,
            EntityKind::Tank,
            start,
            final_goal,
        ));

        let infantry = simplify_reverse_waypoints(
            &map,
            &occ,
            EntityKind::Rifleman,
            start,
            raw_reverse.clone(),
        );
        let tank = simplify_reverse_waypoints(&map, &occ, EntityKind::Tank, start, raw_reverse);

        assert_eq!(
            infantry,
            vec![final_goal],
            "infantry can take the shorter direct segment"
        );
        assert_eq!(
            tank,
            vec![final_goal, detour],
            "tank must keep the detour because its body clips the direct segment"
        );
        assert_reverse_segments_standable(&map, &occ, EntityKind::Tank, start, &tank);
    }

    #[test]
    fn raw_open_tank_route_keeps_every_tile_center_waypoint() {
        let map = flat_test_map(40);
        let start = (4, 4);
        let goal = (28, 17);
        let (tile_path, world_path) = request_fixture_path(&map, EntityKind::Tank, start, goal);

        assert_eq!(
            tile_path.len(),
            world_path.len(),
            "raw world waypoint count should mirror original tile path length"
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
            standability::unit_static_segment_standable(
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
    fn raw_obstacle_route_keeps_corner_waypoint() {
        let map = map_with_rock_rect(24, 7, 6, 10, 8);
        let start = (4, 7);
        let goal = (13, 7);
        let (tile_path, world_path) = request_fixture_path(&map, EntityKind::Rifleman, start, goal);

        assert!(!tile_path.is_empty(), "fixture route should be reachable");
        assert_eq!(
            tile_path.len(),
            world_path.len(),
            "raw world waypoint count should mirror original tile path length around blockers"
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
            !standability::unit_static_segment_standable(
                &map,
                &occ,
                EntityKind::Rifleman,
                map.tile_center(start.0 as u32, start.1 as u32),
                map.tile_center(goal.0 as u32, goal.1 as u32),
            ),
            "direct segment across the rock rectangle should be illegal for later smoothing tests"
        );
    }

    #[test]
    fn tank_pathing_uses_oriented_hull_in_two_tile_corridor() {
        let map = two_tile_wide_horizontal_corridor();
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(1_000, 16);
        let radius_tiles = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius_tiles();

        assert_eq!(
            radius_tiles, 0,
            "v1 tanks must stay point-sized for coarse A* so they can use two-tile corridors"
        );

        let start = (2, 3);
        let goal = (5, 3);
        let waypoints = service.request(
            &map,
            &occ,
            PathRequest {
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );

        assert!(
            !waypoints.is_empty(),
            "tank should find a coarse tile path through a two-tile-wide corridor"
        );
        assert_reverse_segments_standable(
            &map,
            &occ,
            EntityKind::Tank,
            map.tile_center(start.0 as u32, start.1 as u32),
            &waypoints,
        );
    }
}
