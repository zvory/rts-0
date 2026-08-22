use super::*;
use crate::config;
use crate::game::entity::RoutePolicy;

pub(super) const SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX: f32 = config::TILE_SIZE as f32 * 3.0;

pub(super) fn vehicle_finalization_max_segment_px(kind: EntityKind) -> Option<f32> {
    (kind == EntityKind::ScoutCar).then_some(SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX)
}

#[derive(Clone, Copy)]
pub(in crate::game::services) struct RouteFinalizationMode {
    route_shape: RouteShape,
    policy: RoutePolicy,
}

impl RouteFinalizationMode {
    pub(in crate::game::services) fn new(route_shape: RouteShape, policy: RoutePolicy) -> Self {
        Self {
            route_shape,
            policy,
        }
    }
}

/// Refine a resolved tile route without allowing the optional tree-detour pass to erase it.
pub(in crate::game::services) fn finalize_reverse_waypoints_or_raw(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    goal: (f32, f32),
    mode: RouteFinalizationMode,
    waypoints: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let mut raw = waypoints.clone();
    if let Some(final_waypoint) = raw.first_mut() {
        *final_waypoint = goal;
    }
    finalize_reverse_waypoints(map, occupancy, kind, start, goal, mode, waypoints).unwrap_or(raw)
}

/// Apply the same final tree expansion and Scout Car segment simplification to a resolved path.
pub(in crate::game::services) fn finalize_reverse_waypoints(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    goal: (f32, f32),
    mode: RouteFinalizationMode,
    mut waypoints: Vec<(f32, f32)>,
) -> Option<Vec<(f32, f32)>> {
    if waypoints.is_empty() {
        return Some(waypoints);
    }
    waypoints[0] = goal;
    if mode.policy == RoutePolicy::FastestTerrainTime {
        waypoints = super::terrain_finalize::simplify_fastest_terrain_route(
            map,
            occupancy,
            kind,
            start,
            mode.route_shape,
            waypoints,
        );
    }
    let mut waypoints =
        super::tree_detours::expand_reverse_waypoints(map, occupancy, kind, start, waypoints)?;
    if mode.policy == RoutePolicy::LegacyShape
        && mode.route_shape == RouteShape::VehicleClearance
        && !uses_pivot_vehicle_movement(kind)
    {
        waypoints = simplify_reverse_waypoints_with_limit(
            map,
            occupancy,
            kind,
            start,
            waypoints,
            SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX,
        );
    }
    Some(waypoints)
}

#[cfg(test)]
#[path = "route_finalize_tests.rs"]
mod tests;
