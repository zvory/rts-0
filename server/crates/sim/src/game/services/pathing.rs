//! PathingService: the single boundary for all pathfinding requests.
//!
//! Encapsulates terrain mask, dynamic blockers, radius/footprint,
//! per-request path budget, and an LRU cache of verified tile-path results.

use std::collections::HashMap;

use crate::game::entity::{uses_oriented_vehicle_body, uses_pivot_vehicle_movement, EntityKind};
use crate::game::map::Map;
use crate::game::pathfinding::{self, Passability};
use crate::game::services::occupancy::{Occupancy, StaticPathingRelation};
use crate::game::services::standability;
use crate::rules::terrain::{self, TerrainKind};

use cache::{CacheEntry, CacheKey};

const VEHICLE_HARD_CLEARANCE_TILES: u16 = 1;
const VEHICLE_PREFERRED_CLEARANCE_TILES: u16 = 3;
const VEHICLE_CLEARANCE_COST_SCALE: u32 = 2;
const VEHICLE_ROUTE_TURN_PENALTY: u32 = 5;
const VEHICLE_ADJACENT_BLOCKER_COST: u32 = 2;
const VEHICLE_CORNER_GRAZE_COST: u32 = 18;
const VEHICLE_DIAGONAL_BLOCKER_COST: u32 = 3;

/// Parameters for a single path query.
#[derive(Clone)]
pub(crate) struct PathRequest {
    /// Owner/team relation used for owner-aware static obstacle policy.
    pub(super) relation: StaticPathingRelation,
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

impl PathRequest {
    fn relation(&self) -> StaticPathingRelation {
        self.relation.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteShape {
    Normal,
    #[cfg(test)]
    PreferFewerTurns,
    VehicleClearance,
}

impl RouteShape {
    fn turn_penalty(self) -> u32 {
        match self {
            RouteShape::Normal => 0,
            #[cfg(test)]
            RouteShape::PreferFewerTurns => 3,
            RouteShape::VehicleClearance => VEHICLE_ROUTE_TURN_PENALTY,
        }
    }
}

/// Passability oracle that layers terrain + occupancy.
struct TerrainPassability<'a> {
    map: &'a Map,
    occupancy: &'a Occupancy<'a>,
    relation: StaticPathingRelation,
    kind: EntityKind,
    radius_tiles: u32,
    route_shape: RouteShape,
    /// When true, reject tiles pinched between two diagonally-opposite blocked corners.
    /// Used for oriented vehicle bodies so A* avoids 1-tile gaps that the rotating hull
    /// cannot legally thread (see docs/design/server-sim.md pathing notes).
    avoid_diagonal_pinch: bool,
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
        if !self
            .occupancy
            .passable_for_kind_and_relation(tx, ty, self.kind, &self.relation)
        {
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
        if self.avoid_diagonal_pinch {
            let nw = !self.tile_passable(tx - 1, ty - 1);
            let ne = !self.tile_passable(tx + 1, ty - 1);
            let sw = !self.tile_passable(tx - 1, ty + 1);
            let se = !self.tile_passable(tx + 1, ty + 1);
            if (nw && se) || (ne && sw) {
                return false;
            }
        }
        true
    }

    fn movement_cost(&self, tx: i32, ty: i32) -> u32 {
        if self.route_shape != RouteShape::VehicleClearance
            || !uses_oriented_vehicle_body(self.kind)
        {
            return 0;
        }
        vehicle_clearance_cost(self.occupancy.clearance_at_tile_for_kind_and_relation(
            tx,
            ty,
            self.kind,
            &self.relation,
        ))
        .saturating_add(self.vehicle_corner_cost(tx, ty))
    }
}

impl TerrainPassability<'_> {
    fn vehicle_corner_cost(&self, tx: i32, ty: i32) -> u32 {
        let n = !self.tile_passable(tx, ty - 1);
        let e = !self.tile_passable(tx + 1, ty);
        let s = !self.tile_passable(tx, ty + 1);
        let w = !self.tile_passable(tx - 1, ty);
        let nw = !self.tile_passable(tx - 1, ty - 1);
        let ne = !self.tile_passable(tx + 1, ty - 1);
        let se = !self.tile_passable(tx + 1, ty + 1);
        let sw = !self.tile_passable(tx - 1, ty + 1);

        let adjacent_blockers = [n, e, s, w].into_iter().filter(|blocked| *blocked).count() as u32;
        let diagonal_blockers = [nw, ne, se, sw]
            .into_iter()
            .filter(|blocked| *blocked)
            .count() as u32;
        let grazes_corner = (w || e) && (s || n);

        adjacent_blockers * VEHICLE_ADJACENT_BLOCKER_COST
            + diagonal_blockers * VEHICLE_DIAGONAL_BLOCKER_COST
            + if grazes_corner {
                VEHICLE_CORNER_GRAZE_COST
            } else {
                0
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PathCacheStatus {
    Hit,
    Miss,
    Bypassed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PathingRequestDiagnostics {
    pub cache_status: PathCacheStatus,
    /// Nodes expanded by this request; zero on a cache hit.
    pub expanded_nodes: usize,
    /// Deterministic cold-search work used for scheduling, including on a cache hit.
    pub scheduling_expanded_nodes: usize,
    pub budget_exhausted: bool,
    pub tile_path_len: usize,
}

pub(super) enum PathingRequestOutcome<T> {
    Resolved {
        path: T,
        diagnostics: PathingRequestDiagnostics,
    },
    Deferred,
}

/// The authoritative pathfinding boundary.
///
/// Holds an LRU cache so multiple units heading to the same destination, or the same unit
/// repathing across ticks, can reuse prior A* work. Cached paths are verified against the
/// current occupancy before reuse.
#[derive(Clone)]
pub struct PathingService {
    default_budget: usize,
    cache: HashMap<CacheKey, CacheEntry>,
    cache_cap: usize,
    search_scratch: pathfinding::SearchScratch,
    tick: u32,
}

impl PathingService {
    /// Create a new service with the given default budget and cache capacity.
    pub fn new(default_budget: usize, cache_cap: usize) -> Self {
        PathingService {
            default_budget,
            cache: HashMap::with_capacity(cache_cap),
            cache_cap,
            search_scratch: pathfinding::SearchScratch::default(),
            tick: 0,
        }
    }

    /// Advance the internal tick counter. Call once per simulation tick.
    pub fn advance_tick(&mut self, tick: u32) {
        self.tick = tick;
    }

    #[allow(dead_code)]
    pub(in crate::game) fn clear_rebuildable_state(&mut self) {
        self.cache.clear();
        self.search_scratch = pathfinding::SearchScratch::default();
    }

    pub(in crate::game) fn config(&self) -> (usize, usize) {
        (self.default_budget, self.cache_cap)
    }
}

fn expand_vehicle_diagonal_steps_to_l_waypoints<P: Passability>(
    start: (i32, i32),
    tile_path: &[(i32, i32)],
    pass: &P,
) -> Vec<(i32, i32)> {
    let mut expanded = Vec::with_capacity(tile_path.len() * 2);
    let mut prev = start;

    for &next in tile_path {
        let dx = next.0 - prev.0;
        let dy = next.1 - prev.1;
        if dx.abs() == 1 && dy.abs() == 1 {
            if let Some(elbow) = choose_vehicle_l_elbow(prev, next, pass) {
                if expanded.last().copied() != Some(elbow) {
                    expanded.push(elbow);
                }
            }
        }
        if expanded.last().copied() != Some(next) {
            expanded.push(next);
        }
        prev = next;
    }

    expanded
}

fn choose_vehicle_l_elbow<P: Passability>(
    prev: (i32, i32),
    next: (i32, i32),
    pass: &P,
) -> Option<(i32, i32)> {
    let horizontal_first = (next.0, prev.1);
    let vertical_first = (prev.0, next.1);
    let horizontal_ok = pass.passable(horizontal_first.0, horizontal_first.1);
    let vertical_ok = pass.passable(vertical_first.0, vertical_first.1);

    match (horizontal_ok, vertical_ok) {
        (true, true) => {
            let horizontal_cost = pass.movement_cost(horizontal_first.0, horizontal_first.1);
            let vertical_cost = pass.movement_cost(vertical_first.0, vertical_first.1);
            if horizontal_cost <= vertical_cost {
                Some(horizontal_first)
            } else {
                Some(vertical_first)
            }
        }
        (true, false) => Some(horizontal_first),
        (false, true) => Some(vertical_first),
        (false, false) => None,
    }
}

pub(crate) fn vehicle_clearance_cost(clearance_tiles: u16) -> u32 {
    if clearance_tiles < VEHICLE_HARD_CLEARANCE_TILES {
        return u32::MAX / 4;
    }
    let deficit = VEHICLE_PREFERRED_CLEARANCE_TILES.saturating_sub(clearance_tiles) as u32;
    deficit * deficit * VEHICLE_CLEARANCE_COST_SCALE
}

/// Simplify reverse-ordered world waypoints by dropping intermediate tile centers when the unit
/// body can travel straight to a later waypoint without clipping static terrain or buildings.
#[cfg(test)]
fn simplify_reverse_waypoints(
    map: &Map,
    occupancy: &Occupancy,
    kind: EntityKind,
    start: (f32, f32),
    waypoints: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    simplify_reverse_waypoints_with_limit(map, occupancy, kind, start, waypoints, f32::INFINITY)
}

pub(crate) fn simplify_reverse_waypoints_with_limit(
    map: &Map,
    occupancy: &Occupancy,
    kind: EntityKind,
    start: (f32, f32),
    waypoints: Vec<(f32, f32)>,
    max_segment_px: f32,
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
            if distance_between(from, forward[candidate_index]) > max_segment_px {
                continue;
            }
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

fn distance_between(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    (dx * dx + dy * dy).sqrt()
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
        self.cache.keys().any(|key| {
            key.1 == kind
                && key.2 == start
                && key.3 == goal
                && key.4 == radius
                && key.5 == route_shape
        })
    }
}

mod cache;
mod request;
#[cfg(test)]
mod request_tests;
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
            base_sites: Vec::new(),
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
            base_sites: vec![],
        }
    }

    fn clearance_choice_map() -> Map {
        let mut map = flat_test_map(16);
        for tx in 2..=13 {
            let idx = map.index(tx, 3);
            map.terrain[idx] = terrain::ROCK;
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
        let radius_tiles = config::unit_radius_tiles(kind);
        let req = PathRequest {
            relation: StaticPathingRelation::single_owner(1),
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
        let radius_tiles = config::unit_radius_tiles(kind);
        service.request_tile_path(
            map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind,
                start,
                goal,
                radius_tiles,
                route_shape,
                budget: None,
            },
        )
    }

    fn min_tile_clearance(occ: &Occupancy, tile_path: &[(i32, i32)]) -> u16 {
        let interior_len = tile_path.len().saturating_sub(1);
        tile_path
            .iter()
            .take(interior_len)
            .map(|&(tx, ty)| occ.clearance_at_tile(tx, ty))
            .min()
            .unwrap_or(0)
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

    fn diagonal_tile_steps(start: (i32, i32), tile_path: &[(i32, i32)]) -> usize {
        let mut prev = start;
        let mut diagonals = 0usize;
        for &next in tile_path {
            let dx = (next.0 - prev.0).abs();
            let dy = (next.1 - prev.1).abs();
            if dx == 1 && dy == 1 {
                diagonals += 1;
            }
            prev = next;
        }
        diagonals
    }

    fn assert_no_diagonal_world_steps(
        map: &Map,
        start: (i32, i32),
        reverse_waypoints: &[(f32, f32)],
    ) {
        let ts = config::TILE_SIZE as f32;
        let mut points: Vec<_> = std::iter::once(map.tile_center(start.0 as u32, start.1 as u32))
            .chain(reverse_waypoints.iter().rev().copied())
            .collect();
        points.dedup();
        for step in points.windows(2) {
            let dx = (step[1].0 - step[0].0).abs();
            let dy = (step[1].1 - step[0].1).abs();
            assert!(
                dx <= f32::EPSILON || dy <= f32::EPSILON || dx != ts || dy != ts,
                "vehicle world path must not retain a direct diagonal tile-center step: {:?} -> {:?}",
                step[0],
                step[1]
            );
        }
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
    fn vehicle_clearance_cost_tapers_below_preferred_clearance() {
        assert_eq!(vehicle_clearance_cost(VEHICLE_PREFERRED_CLEARANCE_TILES), 0);
        assert!(vehicle_clearance_cost(3) < vehicle_clearance_cost(2));
        assert!(vehicle_clearance_cost(2) < vehicle_clearance_cost(1));
    }

    #[test]
    fn path_cache_eviction_is_deterministic_across_instances() {
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
                relation: StaticPathingRelation::single_owner(1),
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

        let req4 = PathRequest {
            relation: StaticPathingRelation::single_owner(1),
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

        for (start, goal) in &[((1, 1), (3, 3)), ((2, 2), (4, 4)), ((1, 1), (5, 5))] {
            assert!(a.cache_contains(EntityKind::Worker, *start, *goal, 0, RouteShape::Normal));
            assert!(b.cache_contains(EntityKind::Worker, *start, *goal, 0, RouteShape::Normal));
        }
    }

    #[test]
    fn path_cache_scopes_results_by_effective_search_budget() {
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
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Worker,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::Normal,
                budget: Some(0),
            },
        );
        assert!(failed.is_empty());
        assert_eq!(service.cache_len(), 1);

        let found = service.request(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Worker,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert!(!found.is_empty());
        assert_eq!(service.cache_len(), 2);
        assert!(service.cache_contains(EntityKind::Worker, start, goal, 0, RouteShape::Normal));
    }

    #[test]
    fn pivot_vehicle_turn_cost_prefers_fewer_semi_open_route_turns_than_normal_pathing() {
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
            "turn-shaped pivot vehicle route should reduce heading changes, normal={normal:?} shaped={shaped:?}"
        );
    }

    #[test]
    fn pivot_vehicle_turn_cost_still_finds_route_around_obstacle() {
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
    fn pivot_vehicle_turn_cost_keeps_required_bend() {
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
    fn pivot_vehicle_turn_cost_requests_are_deterministic() {
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

        for route_shape in [
            RouteShape::Normal,
            RouteShape::PreferFewerTurns,
            RouteShape::VehicleClearance,
        ] {
            let path = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
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

        assert_eq!(service.cache_len(), 3);
        assert!(service.cache_contains(EntityKind::Tank, start, goal, 0, RouteShape::Normal));
        assert!(service.cache_contains(
            EntityKind::Tank,
            start,
            goal,
            0,
            RouteShape::PreferFewerTurns
        ));
        assert!(service.cache_contains(
            EntityKind::Tank,
            start,
            goal,
            0,
            RouteShape::VehicleClearance
        ));
    }

    #[test]
    fn vehicle_clearance_route_shape_is_part_of_path_cache_key() {
        let map = flat_test_map(40);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let start = (4, 4);
        let goal = (28, 17);
        let radius_tiles = config::unit_radius_tiles(EntityKind::ScoutCar);
        let mut service = PathingService::new(8_192, 256);

        for route_shape in [RouteShape::Normal, RouteShape::VehicleClearance] {
            let path = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
                    kind: EntityKind::ScoutCar,
                    start,
                    goal,
                    radius_tiles,
                    route_shape,
                    budget: None,
                },
            );
            assert!(!path.is_empty());
        }

        assert_eq!(service.cache_len(), 2);
        assert!(service.cache_contains(
            EntityKind::ScoutCar,
            start,
            goal,
            radius_tiles,
            RouteShape::Normal
        ));
        assert!(service.cache_contains(
            EntityKind::ScoutCar,
            start,
            goal,
            radius_tiles,
            RouteShape::VehicleClearance
        ));
    }

    #[test]
    fn static_fingerprint_is_part_of_path_cache_key() {
        let map = flat_test_map(16);
        let mut entities = EntityStore::new();
        let empty_occ = Occupancy::build(&map, &entities);
        let start = (1, 5);
        let goal = (13, 5);
        let mut service = PathingService::new(8_192, 256);
        let req = PathRequest {
            relation: StaticPathingRelation::single_owner(1),
            kind: EntityKind::ScoutCar,
            start,
            goal,
            radius_tiles: config::unit_radius_tiles(EntityKind::ScoutCar),
            route_shape: RouteShape::Normal,
            budget: None,
        };

        let before = service.request_tile_path(&map, &empty_occ, req.clone());
        assert_eq!(before.last().copied(), Some(goal));
        assert_eq!(service.cache_len(), 1);

        let (bx, by) =
            crate::game::services::occupancy::footprint_center(&map, EntityKind::Depot, 5, 3);
        entities
            .spawn_building(1, EntityKind::Depot, bx, by, true)
            .expect("depot should spawn");
        let blocked_occ = Occupancy::build(&map, &entities);
        assert_ne!(
            empty_occ.static_fingerprint(),
            blocked_occ.static_fingerprint()
        );

        let after = service.request_tile_path(&map, &blocked_occ, req);
        assert_eq!(after.last().copied(), Some(goal));
        assert_eq!(
            service.cache_len(),
            2,
            "same request should cache separately when static clearance changes"
        );
    }

    #[test]
    fn tank_trap_pathing_blocks_vehicle_body_but_not_infantry() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        let (tx, ty) =
            crate::game::services::occupancy::footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(1, EntityKind::TankTrap, tx, ty, true)
            .expect("tank trap should spawn");
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let start = (2, 5);
        let goal = (8, 5);

        let infantry_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Rifleman,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert_eq!(infantry_path.last().copied(), Some(goal));
        assert!(
            infantry_path.contains(&(5, 5)),
            "infantry should be able to path through the Tank Trap tile: {infantry_path:?}"
        );

        let tank_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::Tank),
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert_eq!(tank_path.last().copied(), Some(goal));
        assert!(
            !tank_path.contains(&(5, 5)),
            "vehicle-body path should route around the Tank Trap tile: {tank_path:?}"
        );
    }

    #[test]
    fn enemy_tank_trap_pathing_routes_vehicle_body_into_breachable_obstacle() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        let (tx, ty) =
            crate::game::services::occupancy::footprint_center(&map, EntityKind::TankTrap, 5, 5);
        entities
            .spawn_building(2, EntityKind::TankTrap, tx, ty, true)
            .expect("enemy tank trap should spawn");
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let start = (2, 5);
        let goal = (8, 5);

        let tank_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::Tank),
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert_eq!(tank_path.last().copied(), Some(goal));
        assert!(
            tank_path.contains(&(5, 5)),
            "vehicle-body path should route into enemy Tank Trap tiles: {tank_path:?}"
        );
    }

    #[test]
    fn tank_trap_vehicle_fingerprint_does_not_invalidate_infantry_cache() {
        let map = flat_test_map(12);
        let start = (2, 5);
        let goal = (8, 5);
        let empty_entities = EntityStore::new();
        let empty_occ = Occupancy::build(&map, &empty_entities);
        let mut service = PathingService::new(8_192, 256);
        let infantry_req = PathRequest {
            relation: StaticPathingRelation::single_owner(1),
            kind: EntityKind::Worker,
            start,
            goal,
            radius_tiles: config::unit_radius_tiles(EntityKind::Worker),
            route_shape: RouteShape::Normal,
            budget: None,
        };
        let vehicle_req = PathRequest {
            relation: StaticPathingRelation::single_owner(1),
            kind: EntityKind::Tank,
            start,
            goal,
            radius_tiles: config::unit_radius_tiles(EntityKind::Tank),
            route_shape: RouteShape::Normal,
            budget: None,
        };

        assert_eq!(
            service
                .request_tile_path(&map, &empty_occ, infantry_req.clone())
                .last()
                .copied(),
            Some(goal)
        );
        assert_eq!(
            service
                .request_tile_path(&map, &empty_occ, vehicle_req.clone())
                .last()
                .copied(),
            Some(goal)
        );
        assert_eq!(service.cache_len(), 2);

        let mut trap_entities = EntityStore::new();
        let (tx, ty) =
            crate::game::services::occupancy::footprint_center(&map, EntityKind::TankTrap, 5, 5);
        trap_entities
            .spawn_building(1, EntityKind::TankTrap, tx, ty, true)
            .expect("tank trap should spawn");
        let trap_occ = Occupancy::build(&map, &trap_entities);

        assert_eq!(
            service
                .request_tile_path(&map, &trap_occ, infantry_req)
                .last()
                .copied(),
            Some(goal)
        );
        assert_eq!(
            service
                .request_tile_path(&map, &trap_occ, vehicle_req)
                .last()
                .copied(),
            Some(goal)
        );
        assert_eq!(
            service.cache_len(),
            3,
            "Tank Trap should add a new vehicle-body cache entry without duplicating infantry"
        );
    }

    #[test]
    fn vehicle_path_does_not_cut_between_diagonal_touching_tank_traps() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        for (tile_x, tile_y) in [(5, 4), (6, 5)] {
            let (x, y) = crate::game::services::occupancy::footprint_center(
                &map,
                EntityKind::TankTrap,
                tile_x,
                tile_y,
            );
            entities
                .spawn_building(1, EntityKind::TankTrap, x, y, true)
                .expect("tank trap should spawn");
        }
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let start = (4, 5);
        let goal = (7, 4);
        let tank_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::Tank),
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );

        assert_eq!(tank_path.last().copied(), Some(goal));
        let mut prev = start;
        for next in tank_path {
            assert_ne!(
                (prev, next),
                ((5, 5), (6, 4)),
                "vehicle path should not cut the diagonal corner between touching Tank Traps"
            );
            prev = next;
        }
    }

    #[test]
    fn one_tile_tank_trap_gap_blocks_vehicle_pathing_but_not_infantry() {
        let map = flat_test_map(12);
        let mut entities = EntityStore::new();
        for tile_y in [4, 6] {
            let (x, y) = crate::game::services::occupancy::footprint_center(
                &map,
                EntityKind::TankTrap,
                5,
                tile_y,
            );
            entities
                .spawn_building(1, EntityKind::TankTrap, x, y, true)
                .expect("tank trap should spawn");
        }
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let start = (2, 5);
        let goal = (8, 5);

        let infantry_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Rifleman,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::Rifleman),
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );
        assert!(
            infantry_path.contains(&(5, 5)),
            "infantry should still path through the one-tile Tank Trap gap: {infantry_path:?}"
        );

        for kind in [EntityKind::ScoutCar, EntityKind::Tank] {
            let path = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
                    kind,
                    start,
                    goal,
                    radius_tiles: config::unit_radius_tiles(kind),
                    route_shape: RouteShape::Normal,
                    budget: None,
                },
            );
            assert_eq!(path.last().copied(), Some(goal));
            assert!(
                !path.contains(&(5, 5)),
                "{kind:?} should route around the closed Tank Trap pair gap: {path:?}"
            );
        }
    }

    #[test]
    fn two_tile_tank_trap_gap_remains_vehicle_pathable() {
        let map = flat_test_map(14);
        let mut entities = EntityStore::new();
        for tile_x in [5, 8] {
            let (x, y) = crate::game::services::occupancy::footprint_center(
                &map,
                EntityKind::TankTrap,
                tile_x,
                6,
            );
            entities
                .spawn_building(1, EntityKind::TankTrap, x, y, true)
                .expect("tank trap should spawn");
        }
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 256);
        let start = (6, 2);
        let goal = (6, 10);

        for kind in [EntityKind::Worker, EntityKind::Tank] {
            let path = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
                    kind,
                    start,
                    goal,
                    radius_tiles: config::unit_radius_tiles(kind),
                    route_shape: RouteShape::Normal,
                    budget: None,
                },
            );
            assert_eq!(
                path.last().copied(),
                Some(goal),
                "{kind:?} should path through a two-tile Tank Trap gap: {path:?}"
            );
        }
    }

    #[test]
    fn pivot_vehicle_turn_cost_respects_expansion_budget() {
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
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::PreferFewerTurns,
                budget: Some(0),
            },
        );
        assert!(bounded.is_empty());
        assert_eq!(service.cache_len(), 1);

        let unbounded = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles: 0,
                route_shape: RouteShape::PreferFewerTurns,
                budget: None,
            },
        );
        assert_eq!(unbounded.last().copied(), Some(goal));
        assert_eq!(service.cache_len(), 2);
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
    fn tank_style_vehicle_routes_expand_diagonal_steps_to_l_waypoints() {
        let map = flat_test_map(40);
        let start = (4, 4);
        let goal = (28, 17);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);

        for kind in [EntityKind::Tank, EntityKind::AntiTankGun] {
            let (tile_path, world_path) = request_fixture_path(&map, kind, start, goal);
            let diagonal_steps = diagonal_tile_steps(start, &tile_path);
            assert!(
                diagonal_steps > 0,
                "fixture should include raw diagonal A* steps for {kind:?}; got {tile_path:?}"
            );
            assert_eq!(
                world_path.len(),
                tile_path.len() + diagonal_steps,
                "pivot-drive vehicles should insert one L elbow per raw diagonal step"
            );
            assert_no_diagonal_world_steps(&map, start, &world_path);
            assert_reverse_segments_standable(
                &map,
                &occ,
                kind,
                map.tile_center(start.0 as u32, start.1 as u32),
                &world_path,
            );

            let forward_world: Vec<_> =
                std::iter::once(map.tile_center(start.0 as u32, start.1 as u32))
                    .chain(world_path.iter().rev().copied())
                    .collect();
            let heading_changes = heading_changes_above(&forward_world, 10.0_f32.to_radians());
            assert!(
                heading_changes >= diagonal_steps,
                "L-expanded route should visibly preserve corner turns for {kind:?}"
            );
        }
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
        let radius_tiles = config::unit_radius_tiles(EntityKind::Tank);

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
                relation: StaticPathingRelation::single_owner(1),
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

    #[test]
    fn vehicle_clearance_pathing_prefers_clearance_in_wide_space() {
        let map = clearance_choice_map();
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 16);
        let start = (1, 4);
        let goal = (14, 4);

        for kind in [
            EntityKind::ScoutCar,
            EntityKind::Tank,
            EntityKind::AntiTankGun,
        ] {
            let normal = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
                    kind,
                    start,
                    goal,
                    radius_tiles: config::unit_radius_tiles(kind),
                    route_shape: RouteShape::Normal,
                    budget: None,
                },
            );
            let shaped = service.request_tile_path(
                &map,
                &occ,
                PathRequest {
                    relation: StaticPathingRelation::single_owner(1),
                    kind,
                    start,
                    goal,
                    radius_tiles: config::unit_radius_tiles(kind),
                    route_shape: RouteShape::VehicleClearance,
                    budget: None,
                },
            );

            assert_eq!(normal.last().copied(), Some(goal));
            assert_eq!(shaped.last().copied(), Some(goal));
            assert!(
                min_tile_clearance(&occ, &shaped) > min_tile_clearance(&occ, &normal),
                "{kind:?} clearance route should improve minimum clearance, normal={normal:?} shaped={shaped:?}"
            );
            assert!(
                shaped.iter().any(|&(_, ty)| ty >= 6),
                "{kind:?} route should move away from the wall shelf when open space is available, got {shaped:?}"
            );
        }
    }

    #[test]
    fn vehicle_clearance_cost_keeps_narrow_passage_traversable() {
        let map = two_tile_wide_horizontal_corridor();
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 16);
        let start = (2, 3);
        let goal = (5, 3);

        let tile_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::ScoutCar,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::ScoutCar),
                route_shape: RouteShape::VehicleClearance,
                budget: None,
            },
        );

        assert_eq!(tile_path.last().copied(), Some(goal));
    }

    #[test]
    fn vehicle_clearance_route_avoids_corner_graze_tiles_when_alternatives_exist() {
        let map = map_with_rock_rect(24, 9, 8, 11, 10);
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 16);
        let start = (5, 11);
        let goal = (15, 7);

        let shaped = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::ScoutCar,
                start,
                goal,
                radius_tiles: config::unit_radius_tiles(EntityKind::ScoutCar),
                route_shape: RouteShape::VehicleClearance,
                budget: None,
            },
        );

        assert_eq!(shaped.last().copied(), Some(goal));
        for corner_graze in [(8, 11), (12, 7)] {
            assert!(
                !shaped.contains(&corner_graze),
                "vehicle clearance route should avoid corner-graze tile {corner_graze:?} when a wider route exists, got {shaped:?}"
            );
        }
    }

    fn diagonal_pinch_map() -> Map {
        // Two 3x3 rock footprints arranged as in the tank-pinch bug:
        //   A at tiles (0..=2, 0..=2), B at tiles (4..=6, 3..=5).
        // The gap column is x = 3; tile (3, 3) sits between diagonally-opposite blocked corners
        // (2, 2) and (4, 4).
        let mut map = flat_test_map(16);
        for ty in 0..=2 {
            for tx in 0..=2 {
                let idx = map.index(tx, ty);
                map.terrain[idx] = terrain::ROCK;
            }
        }
        for ty in 3..=5 {
            for tx in 4..=6 {
                let idx = map.index(tx, ty);
                map.terrain[idx] = terrain::ROCK;
            }
        }
        map
    }

    #[test]
    fn tank_pathing_avoids_diagonal_pinch_between_offset_buildings() {
        let map = diagonal_pinch_map();
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 16);
        let radius_tiles = config::unit_radius_tiles(EntityKind::Tank);

        let start = (0, 5);
        let goal = (6, 0);
        let tile_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Tank,
                start,
                goal,
                radius_tiles,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );

        assert!(
            !tile_path.is_empty(),
            "tank should still reach the goal around the pinch"
        );
        assert_eq!(
            tile_path.last().copied(),
            Some(goal),
            "tank path must terminate at the goal"
        );
        assert!(
            !tile_path.contains(&(3, 3)),
            "tank A* must skip the diagonal-pinch tile (3, 3), got {tile_path:?}"
        );
    }

    #[test]
    fn infantry_pathing_ignores_diagonal_pinch_rule() {
        let map = diagonal_pinch_map();
        let entities = EntityStore::new();
        let occ = Occupancy::build(&map, &entities);
        let mut service = PathingService::new(8_192, 16);
        let radius_tiles = config::unit_radius_tiles(EntityKind::Rifleman);

        let start = (0, 5);
        let goal = (6, 0);
        let tile_path = service.request_tile_path(
            &map,
            &occ,
            PathRequest {
                relation: StaticPathingRelation::single_owner(1),
                kind: EntityKind::Rifleman,
                start,
                goal,
                radius_tiles,
                route_shape: RouteShape::Normal,
                budget: None,
            },
        );

        assert!(!tile_path.is_empty(), "infantry path should be found");
        assert!(
            tile_path.contains(&(3, 3)),
            "infantry must remain free to thread the diagonal pinch, got {tile_path:?}"
        );
    }
}
