use std::sync::Arc;
use std::time::Instant;

use super::{Occupancy, RoutePolicy, RouteShape, TerrainPassability};
use crate::game::entity::{
    movement_body_class, uses_oriented_vehicle_body, EntityKind, EntityStore, MovementBodyClass,
};
use crate::game::map::Map;
use crate::game::pathfinding::Passability;

const BLOCKED_EDGE: u32 = u32::MAX;
const EDGE_PAGE_TILES: usize = 256;
const DIRECTIONS: [(i32, i32, u32); 8] = [
    (1, 0, 10),
    (-1, 0, 10),
    (0, 1, 10),
    (0, -1, 10),
    (1, 1, 14),
    (1, -1, 14),
    (-1, 1, 14),
    (-1, -1, 14),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RoutingProfile {
    movement_body: MovementBodyClass,
    radius_tiles: u32,
    vehicle_clearance: bool,
    diagonal_pinch: bool,
    policy: RoutePolicy,
}

impl RoutingProfile {
    fn for_request(
        kind: EntityKind,
        radius_tiles: u32,
        route_shape: RouteShape,
        policy: RoutePolicy,
    ) -> Self {
        let oriented = uses_oriented_vehicle_body(kind);
        Self {
            movement_body: movement_body_class(kind),
            radius_tiles,
            vehicle_clearance: oriented && route_shape == RouteShape::VehicleClearance,
            diagonal_pinch: oriented,
            policy,
        }
    }

    fn representative_kind(self) -> EntityKind {
        match self.movement_body {
            MovementBodyClass::InfantryLike => EntityKind::Rifleman,
            MovementBodyClass::VehicleBody => EntityKind::Tank,
        }
    }

    fn route_shape(self) -> RouteShape {
        if self.vehicle_clearance {
            RouteShape::VehicleClearance
        } else {
            RouteShape::Normal
        }
    }

    fn invalidation_radius(self) -> i32 {
        // An edge reads its destination and, for diagonals, two orthogonal guards. Standability
        // reads the profile radius plus diagonal-pinch corners. Vehicle shaping additionally reads
        // clearance up to the preferred threshold of three tiles; beyond that its cost is zero.
        (self.radius_tiles as i32 + 2).max(if self.vehicle_clearance { 4 } else { 2 })
    }
}

#[derive(Clone)]
struct EdgeTable {
    width: u32,
    height: u32,
    pages: Vec<Arc<EdgePage>>,
    heuristic_cardinal: u32,
    heuristic_diagonal: u32,
}

#[derive(Clone)]
struct EdgePage {
    passable: Vec<u8>,
    /// Row-major tile, then the legacy eight-direction order. Values are the additional movement
    /// cost only, preserving the A* saturation order; `u32::MAX` denotes an illegal edge.
    edge_costs: Vec<u32>,
}

impl EdgeTable {
    fn build<P: Passability>(pass: &P) -> Self {
        let (width, height) = pass.dimensions();
        let cells = width.saturating_mul(height) as usize;
        let pages = cells.div_ceil(EDGE_PAGE_TILES);
        let mut table = Self {
            width,
            height,
            pages: (0..pages)
                .map(|page| {
                    let page_start = page * EDGE_PAGE_TILES;
                    let page_len = (cells - page_start).min(EDGE_PAGE_TILES);
                    Arc::new(EdgePage {
                        passable: vec![0; page_len],
                        edge_costs: vec![BLOCKED_EDGE; page_len.saturating_mul(DIRECTIONS.len())],
                    })
                })
                .collect(),
            heuristic_cardinal: u32::MAX,
            heuristic_diagonal: u32::MAX,
        };
        for ty in 0..height as i32 {
            for tx in 0..width as i32 {
                table.recompute_tile(pass, tx, ty);
            }
        }
        table.refresh_heuristic_minima();
        table
    }

    fn refresh_heuristic_minima(&mut self) {
        let mut cardinal = u32::MAX;
        let mut diagonal = u32::MAX;
        for page in &self.pages {
            for costs in page.edge_costs.chunks_exact(DIRECTIONS.len()) {
                for (direction, &cost) in costs.iter().enumerate() {
                    if cost == BLOCKED_EDGE {
                        continue;
                    }
                    if DIRECTIONS[direction].0 != 0 && DIRECTIONS[direction].1 != 0 {
                        diagonal = diagonal.min(cost);
                    } else {
                        cardinal = cardinal.min(cost);
                    }
                }
            }
        }
        self.heuristic_cardinal = if cardinal == u32::MAX { 0 } else { cardinal };
        self.heuristic_diagonal = if diagonal == u32::MAX {
            0
        } else {
            diagonal.min(self.heuristic_cardinal.saturating_mul(2))
        };
    }

    fn recompute_tile<P: Passability>(&mut self, pass: &P, tx: i32, ty: i32) {
        let Some(tile_index) = self.tile_index(tx, ty) else {
            return;
        };
        let page_index = tile_index / EDGE_PAGE_TILES;
        let page_tile = tile_index % EDGE_PAGE_TILES;
        let page = Arc::make_mut(&mut self.pages[page_index]);
        page.passable[page_tile] = u8::from(pass.passable(tx, ty));
        let edge_base = page_tile * DIRECTIONS.len();
        for (direction, &(dx, dy, step_cost)) in DIRECTIONS.iter().enumerate() {
            page.edge_costs[edge_base + direction] = pass
                .edge_cost(tx, ty, dx, dy, step_cost)
                .unwrap_or(BLOCKED_EDGE);
        }
    }

    fn tile_index(&self, tx: i32, ty: i32) -> Option<usize> {
        if tx < 0 || ty < 0 || (tx as u32) >= self.width || (ty as u32) >= self.height {
            return None;
        }
        Some((ty as u32 * self.width + tx as u32) as usize)
    }

    fn direction(dx: i32, dy: i32) -> Option<usize> {
        DIRECTIONS
            .iter()
            .position(|&(candidate_x, candidate_y, _)| candidate_x == dx && candidate_y == dy)
    }

    #[allow(dead_code)] // Read by release evidence tests; retained in non-test builds for diagnostics.
    fn retained_bytes(&self) -> usize {
        self.pages
            .iter()
            .map(|page| {
                page.passable.capacity() + page.edge_costs.capacity() * std::mem::size_of::<u32>()
            })
            .sum()
    }

    fn passable(&self, tile_index: usize) -> bool {
        let page = &self.pages[tile_index / EDGE_PAGE_TILES];
        page.passable[tile_index % EDGE_PAGE_TILES] != 0
    }

    fn edge_cost(&self, tile_index: usize, direction: usize) -> u32 {
        let page = &self.pages[tile_index / EDGE_PAGE_TILES];
        page.edge_costs[(tile_index % EDGE_PAGE_TILES) * DIRECTIONS.len() + direction]
    }
}

#[derive(Clone)]
struct ProfileTable {
    profile: RoutingProfile,
    #[allow(dead_code)] // Immutable authored-map table is also reported by release diagnostics.
    base: Arc<EdgeTable>,
    dynamic: Arc<EdgeTable>,
    topology: Vec<(u32, EntityKind, u32, u32)>,
    blocked_tiles: Vec<bool>,
}

/// Rebuildable, room-owned directed pathing graph. Authored-map/profile edges are immutable;
/// building changes patch only origins within the audited dependency radius.
#[derive(Clone, Default)]
pub(super) struct PathGraph {
    map_content_key: Option<String>,
    generation: u64,
    profiles: Vec<ProfileTable>,
    initialization_ns: u64,
    local_update_ns: Vec<u64>,
    local_update_origins: Vec<usize>,
}

impl PathGraph {
    pub(super) fn for_map(map: &Map) -> Self {
        Self {
            map_content_key: Some(map.materialized_hash()),
            ..Self::default()
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn view(
        &mut self,
        map: &Map,
        occupancy: &Occupancy<'_>,
        kind: EntityKind,
        radius_tiles: u32,
        route_shape: RouteShape,
        policy: RoutePolicy,
    ) -> GraphPassability {
        if self.map_content_key.is_none() {
            self.map_content_key = Some(map.materialized_hash());
        }
        let profile = RoutingProfile::for_request(kind, radius_tiles, route_shape, policy);
        let topology = occupancy.path_graph_topology();
        let profile_index = self
            .profiles
            .iter()
            .position(|table| table.profile == profile);
        let index = match profile_index {
            Some(index) => {
                if self.profiles[index].topology != topology {
                    let blocked_tiles = occupancy.path_graph_blocked_tiles(profile.movement_body);
                    if self.profiles[index].blocked_tiles != blocked_tiles {
                        self.update_profile(index, map, occupancy, topology, &blocked_tiles);
                    } else {
                        self.profiles[index].topology = topology.to_vec();
                    }
                }
                index
            }
            None => {
                let blocked_tiles = occupancy.path_graph_blocked_tiles(profile.movement_body);
                self.initialize_profile(profile, map, occupancy, &blocked_tiles)
            }
        };
        GraphPassability {
            table: Arc::clone(&self.profiles[index].dynamic),
            cost_fingerprint: terrain_cost_fingerprint(
                self.map_content_key.as_deref().unwrap_or_default(),
                policy,
            ),
        }
    }

    fn initialize_profile(
        &mut self,
        profile: RoutingProfile,
        map: &Map,
        occupancy: &Occupancy<'_>,
        blocked_tiles: &[bool],
    ) -> usize {
        let started = Instant::now();
        let empty_entities = EntityStore::new();
        let base_occupancy = Occupancy::build(map, &empty_entities);
        let base_blocked_tiles = base_occupancy.path_graph_blocked_tiles(profile.movement_body);
        let base_pass = terrain_passability(profile, map, &base_occupancy);
        let base = Arc::new(EdgeTable::build(&base_pass));
        // Start from the authored-map table so unchanged pages remain shared. Building overlays
        // copy only pages containing an affected edge origin.
        let dynamic = Arc::new((*base).clone());
        self.initialization_ns = self
            .initialization_ns
            .saturating_add(started.elapsed().as_nanos() as u64);
        self.generation = self.generation.wrapping_add(1).max(1);
        self.profiles.push(ProfileTable {
            profile,
            base,
            dynamic,
            topology: Vec::new(),
            blocked_tiles: base_blocked_tiles,
        });
        let index = self.profiles.len() - 1;
        let topology = occupancy.path_graph_topology();
        if self.profiles[index].blocked_tiles != blocked_tiles {
            self.update_profile(index, map, occupancy, topology, blocked_tiles);
        } else {
            self.profiles[index].topology = topology.to_vec();
        }
        index
    }

    fn update_profile(
        &mut self,
        index: usize,
        map: &Map,
        occupancy: &Occupancy<'_>,
        topology: &[(u32, EntityKind, u32, u32)],
        current: &[bool],
    ) {
        let started = Instant::now();
        let profile = self.profiles[index].profile;
        let width = map.width as i32;
        let height = map.height as i32;
        let radius = profile.invalidation_radius();
        let mut dirty = vec![false; current.len()];
        for (tile_index, (&before, &after)) in self.profiles[index]
            .blocked_tiles
            .iter()
            .zip(current)
            .enumerate()
        {
            if before == after {
                continue;
            }
            let changed_x = tile_index as i32 % width;
            let changed_y = tile_index as i32 / width;
            for ty in (changed_y - radius).max(0)..=(changed_y + radius).min(height - 1) {
                for tx in (changed_x - radius).max(0)..=(changed_x + radius).min(width - 1) {
                    dirty[(ty * width + tx) as usize] = true;
                }
            }
        }

        let pass = terrain_passability(profile, map, occupancy);
        let table = Arc::make_mut(&mut self.profiles[index].dynamic);
        let mut updated_origins = 0usize;
        for (tile_index, is_dirty) in dirty.into_iter().enumerate() {
            if !is_dirty {
                continue;
            }
            let tx = tile_index as i32 % width;
            let ty = tile_index as i32 / width;
            table.recompute_tile(&pass, tx, ty);
            updated_origins += 1;
        }
        self.profiles[index].topology = topology.to_vec();
        self.profiles[index].blocked_tiles = current.to_vec();
        self.generation = self.generation.wrapping_add(1).max(1);
        self.local_update_ns
            .push(started.elapsed().as_nanos() as u64);
        self.local_update_origins.push(updated_origins);
    }

    #[cfg(test)]
    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub(super) fn retained_bytes(&self) -> (usize, usize) {
        self.profiles
            .iter()
            .fold((0, 0), |(base, dynamic), profile| {
                (
                    base + profile.base.retained_bytes(),
                    dynamic
                        + profile.dynamic.retained_bytes()
                        + profile.blocked_tiles.capacity().div_ceil(8),
                )
            })
    }

    #[cfg(test)]
    pub(super) fn last_update_origins(&self) -> Option<usize> {
        self.local_update_origins.last().copied()
    }

    #[cfg(test)]
    pub(super) fn initialization_ns(&self) -> u64 {
        self.initialization_ns
    }

    #[cfg(test)]
    pub(super) fn local_update_ns(&self) -> &[u64] {
        &self.local_update_ns
    }
}

fn terrain_passability<'a>(
    profile: RoutingProfile,
    map: &'a Map,
    occupancy: &'a Occupancy<'a>,
) -> TerrainPassability<'a> {
    TerrainPassability {
        map,
        occupancy,
        kind: profile.representative_kind(),
        radius_tiles: profile.radius_tiles,
        route_shape: profile.route_shape(),
        policy: profile.policy,
        avoid_diagonal_pinch: profile.diagonal_pinch,
    }
}

pub(super) struct GraphPassability {
    table: Arc<EdgeTable>,
    cost_fingerprint: u64,
}

impl Passability for GraphPassability {
    fn dimensions(&self) -> (u32, u32) {
        (self.table.width, self.table.height)
    }

    #[inline]
    fn passable(&self, tx: i32, ty: i32) -> bool {
        self.table
            .tile_index(tx, ty)
            .is_some_and(|index| self.table.passable(index))
    }

    #[inline]
    fn edge_cost(
        &self,
        from_tx: i32,
        from_ty: i32,
        dx: i32,
        dy: i32,
        _base_step_cost: u32,
    ) -> Option<u32> {
        let tile = self.table.tile_index(from_tx, from_ty)?;
        let direction = EdgeTable::direction(dx, dy)?;
        let cost = self.table.edge_cost(tile, direction);
        (cost != BLOCKED_EDGE).then_some(cost)
    }

    fn heuristic_step_costs(&self) -> (u32, u32) {
        (self.table.heuristic_cardinal, self.table.heuristic_diagonal)
    }

    fn cost_fingerprint(&self) -> u64 {
        self.cost_fingerprint
    }
}

fn terrain_cost_fingerprint(map_content_key: &str, policy: RoutePolicy) -> u64 {
    const TERRAIN_COST_REVISION: u64 = 0x4654_5433_0000_0001;
    let prefix = map_content_key.get(..16).unwrap_or(map_content_key);
    let map_bits = u64::from_str_radix(prefix, 16).unwrap_or(0);
    map_bits
        ^ TERRAIN_COST_REVISION
        ^ match policy {
            RoutePolicy::LegacyShape => 0,
            RoutePolicy::FastestTerrainTime => 1,
        }
}

#[cfg(test)]
#[path = "graph/tests.rs"]
mod tests;
