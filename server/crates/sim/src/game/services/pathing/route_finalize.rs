use super::*;
use crate::config;

const SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX: f32 = config::TILE_SIZE as f32 * 3.0;

/// Refine a resolved tile route without allowing the optional tree-detour pass to erase it.
pub(in crate::game::services) fn finalize_reverse_waypoints_or_raw(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    goal: (f32, f32),
    route_shape: RouteShape,
    waypoints: Vec<(f32, f32)>,
) -> Vec<(f32, f32)> {
    let mut raw = waypoints.clone();
    if let Some(final_waypoint) = raw.first_mut() {
        *final_waypoint = goal;
    }
    finalize_reverse_waypoints(map, occupancy, kind, start, goal, route_shape, waypoints)
        .unwrap_or(raw)
}

/// Apply the same final tree expansion and Scout Car segment simplification to a resolved path.
pub(in crate::game::services) fn finalize_reverse_waypoints(
    map: &Map,
    occupancy: &Occupancy<'_>,
    kind: EntityKind,
    start: (f32, f32),
    goal: (f32, f32),
    route_shape: RouteShape,
    mut waypoints: Vec<(f32, f32)>,
) -> Option<Vec<(f32, f32)>> {
    if waypoints.is_empty() {
        return Some(waypoints);
    }
    waypoints[0] = goal;
    let mut waypoints =
        super::tree_detours::expand_reverse_waypoints(map, occupancy, kind, start, waypoints)?;
    if route_shape == RouteShape::VehicleClearance && !uses_pivot_vehicle_movement(kind) {
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
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;
    use crate::protocol::{terrain, MapDoodad};

    #[test]
    fn scout_car_finalization_applies_live_three_tile_segment_limit() {
        let map = Map {
            width: 16,
            height: 16,
            terrain: vec![terrain::GRASS; 16 * 16],
            ..Default::default()
        };
        let entities = EntityStore::new();
        let occupancy = Occupancy::build(&map, &entities);
        let start = map.tile_center(2, 8);
        let goal = map.tile_center(12, 8);
        let tile_path: Vec<_> = (3..=12).map(|x| (x, 8)).collect();
        let raw = crate::game::pathfinding::to_world_waypoints(&tile_path);
        let finalized = finalize_reverse_waypoints(
            &map,
            &occupancy,
            EntityKind::ScoutCar,
            start,
            goal,
            RouteShape::VehicleClearance,
            raw.clone(),
        )
        .expect("open route should finalize");

        assert!(finalized.len() < raw.len());
        let mut forward = finalized;
        forward.reverse();
        let mut from = start;
        for to in forward {
            assert!(distance_between(from, to) <= SCOUT_CAR_ROUTE_SIMPLIFY_MAX_SEGMENT_PX + 0.01);
            from = to;
        }
        assert_eq!(from, goal);
    }

    #[test]
    fn every_oriented_vehicle_keeps_raw_route_when_tree_refinement_is_bounded_out() {
        let mut map = Map {
            width: 16,
            height: 16,
            terrain: vec![terrain::GRASS; 16 * 16],
            ..Default::default()
        };
        let trunk = map.tile_center(8, 8);
        map.doodads = (1..=9)
            .map(|id| MapDoodad {
                id,
                type_id: "tree.alder".to_string(),
                x: trunk.0 as u32,
                y: trunk.1 as u32,
                color: None,
            })
            .collect();
        let entities = EntityStore::new();
        let occupancy = Occupancy::build(&map, &entities);
        let start = map.tile_center(6, 8);
        let goal = map.tile_center(10, 8);
        let raw = vec![goal];

        for kind in EntityKind::ALL
            .into_iter()
            .filter(|kind| uses_oriented_vehicle_body(*kind))
        {
            assert!(finalize_reverse_waypoints(
                &map,
                &occupancy,
                kind,
                start,
                goal,
                RouteShape::VehicleClearance,
                raw.clone(),
            )
            .is_none());
            assert_eq!(
                finalize_reverse_waypoints_or_raw(
                    &map,
                    &occupancy,
                    kind,
                    start,
                    goal,
                    RouteShape::VehicleClearance,
                    raw.clone(),
                ),
                raw,
                "{kind} lost its resolved tile route"
            );
        }
    }
}
