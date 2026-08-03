//! Static authored-map route queries backed by the live simulation pathing stack.
//!
//! This is intentionally a narrow adapter: authoring diagnostics get the same terrain, tree,
//! vehicle-body, clearance, and route-shaping behavior as live movement without exposing those
//! implementation types through the public `Game` API.

use crate::game::entity::{uses_oriented_vehicle_body, EntityKind, EntityStore};
use crate::{config, rules::terrain};

use super::{Map, Occupancy, PathRequest, PathingRequestOutcome, PathingService, RouteShape};

const SEARCH_STATES_PER_TILE: usize = 9;
const ROUTE_SAMPLE_STEP_PX: f32 = 8.0;

#[derive(Clone, Copy, Debug)]
pub(in crate::game) struct StaticRouteResult {
    pub reachable: bool,
    pub distance_px: Option<f64>,
    pub estimated_travel_seconds: Option<f64>,
    pub failure_reason: Option<&'static str>,
}

pub(in crate::game) struct StaticRouteAnalyzer<'a> {
    map: &'a Map,
    occupancy: Occupancy<'a>,
    pathing: PathingService,
    search_budget: usize,
}

impl<'a> StaticRouteAnalyzer<'a> {
    pub fn new(map: &'a Map) -> Self {
        let entities = EntityStore::new();
        let occupancy = Occupancy::build(map, &entities);
        let search_budget = (map.width as usize)
            .saturating_mul(map.height as usize)
            .saturating_mul(SEARCH_STATES_PER_TILE)
            .saturating_add(1);
        Self {
            map,
            occupancy,
            pathing: PathingService::new(search_budget, 256),
            search_budget,
        }
    }

    pub fn route(
        &mut self,
        kind: EntityKind,
        start_tile: (u32, u32),
        goal_tile: (u32, u32),
    ) -> StaticRouteResult {
        let start = (start_tile.0 as i32, start_tile.1 as i32);
        let goal = (goal_tile.0 as i32, goal_tile.1 as i32);
        if !self.occupancy.passable_for_kind(start.0, start.1, kind) {
            return unreachable("start_not_passable");
        }
        if !self.occupancy.passable_for_kind(goal.0, goal.1, kind) {
            return unreachable("goal_not_passable");
        }

        let start_world = self.map.tile_center(start_tile.0, start_tile.1);
        let goal_world = self.map.tile_center(goal_tile.0, goal_tile.1);
        let vehicle = uses_oriented_vehicle_body(kind);
        let request = PathRequest {
            kind,
            start,
            goal,
            radius_tiles: config::unit_radius_tiles(kind),
            route_shape: if vehicle {
                RouteShape::VehicleClearance
            } else {
                RouteShape::Normal
            },
            budget: Some(self.search_budget),
        };
        let direct_segment = (!vehicle).then_some((start_world, goal_world));
        let PathingRequestOutcome::Resolved {
            path: reverse_waypoints,
            diagnostics,
        } = self.pathing.request_with_diagnostics(
            self.map,
            &self.occupancy,
            request,
            direct_segment,
            true,
        )
        else {
            return unreachable("path_deferred");
        };

        if diagnostics.budget_exhausted {
            return unreachable("search_budget_exhausted");
        }
        if reverse_waypoints.first().copied() != Some(goal_world) {
            return unreachable("no_route");
        }

        let Some(mut forward_waypoints) =
            super::tree_detours::expand_reverse_waypoints(
                self.map,
                &self.occupancy,
                kind,
                start_world,
                reverse_waypoints,
            )
        else {
            return unreachable("tree_detour_unavailable");
        };
        forward_waypoints.reverse();
        let mut distance_px = 0.0_f64;
        let mut travel_ticks = 0.0_f64;
        let Some(speed) = config::unit_stats(kind).map(|stats| f64::from(stats.speed)) else {
            return unreachable("missing_unit_speed");
        };
        let mut from = start_world;
        for to in forward_waypoints {
            let segment_distance = distance(from, to);
            distance_px += segment_distance;
            travel_ticks += estimate_segment_ticks(self.map, kind, from, to, speed);
            from = to;
        }

        StaticRouteResult {
            reachable: true,
            distance_px: Some(round_milli(distance_px)),
            estimated_travel_seconds: Some(round_milli(travel_ticks / f64::from(config::TICK_HZ))),
            failure_reason: None,
        }
    }
}

fn unreachable(reason: &'static str) -> StaticRouteResult {
    StaticRouteResult {
        reachable: false,
        distance_px: None,
        estimated_travel_seconds: None,
        failure_reason: Some(reason),
    }
}

fn distance(from: (f32, f32), to: (f32, f32)) -> f64 {
    let dx = f64::from(to.0 - from.0);
    let dy = f64::from(to.1 - from.1);
    (dx * dx + dy * dy).sqrt()
}

fn estimate_segment_ticks(
    map: &Map,
    kind: EntityKind,
    from: (f32, f32),
    to: (f32, f32),
    base_speed: f64,
) -> f64 {
    let distance = distance(from, to);
    if distance == 0.0 {
        return 0.0;
    }
    let samples = (distance / f64::from(ROUTE_SAMPLE_STEP_PX)).ceil().max(1.0) as u32;
    let sample_distance = distance / f64::from(samples);
    (0..samples)
        .map(|sample| {
            let t = (f64::from(sample) + 0.5) / f64::from(samples);
            let x = f64::from(from.0) + f64::from(to.0 - from.0) * t;
            let y = f64::from(from.1) + f64::from(to.1 - from.1) * t;
            let (tx, ty) = map.tile_of(x as f32, y as f32);
            let multiplier = terrain::TerrainKind::from_map_code(map.terrain_at(tx, ty))
                .map(|terrain| f64::from(terrain::movement_speed_multiplier(kind, terrain)))
                .unwrap_or(1.0);
            sample_distance / (base_speed * multiplier)
        })
        .sum()
}

fn round_milli(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}
