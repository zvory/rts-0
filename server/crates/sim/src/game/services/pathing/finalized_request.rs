use super::*;

pub(super) type GoalEndpoint = ((i32, i32), (f32, f32));
type SelectedWorldPath = (Vec<(f32, f32)>, Option<usize>);

impl PathingService {
    pub(in crate::game::services) fn request_best_finalized_with_diagnostics(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
        start_world: (f32, f32),
        goals: &[GoalEndpoint],
        allow_pathfinding: bool,
    ) -> PathingRequestOutcome<SelectedWorldPath> {
        let goal_tiles = goals.iter().map(|goal| goal.0).collect::<Vec<_>>();
        let kind = req.kind;
        let start_tile = req.start;
        let route_shape = req.route_shape;
        let policy = req.policy;
        let PathingRequestOutcome::Resolved {
            path: (tile_path, goal_index),
            diagnostics,
        } = self.request_tile_path_to_any_with_diagnostics(
            map,
            occupancy,
            req,
            &goal_tiles,
            allow_pathfinding,
        )
        else {
            return PathingRequestOutcome::Deferred;
        };
        let raw = if uses_pivot_vehicle_movement(kind) {
            let pass = TerrainPassability {
                map,
                occupancy,
                kind,
                radius_tiles: 0,
                route_shape: RouteShape::VehicleClearance,
                policy,
                avoid_diagonal_pinch: true,
            };
            pathfinding::to_world_waypoints(&expand_vehicle_diagonal_steps_to_l_waypoints(
                start_tile, &tile_path, &pass,
            ))
        } else {
            pathfinding::to_world_waypoints(&tile_path)
        };
        let Some(goal_world) = goal_index.and_then(|index| goals.get(index).map(|goal| goal.1))
        else {
            return PathingRequestOutcome::Resolved {
                path: (raw, None),
                diagnostics,
            };
        };
        let path = super::route_finalize::finalize_reverse_waypoints_or_raw(
            map,
            occupancy,
            kind,
            start_world,
            goal_world,
            super::route_finalize::RouteFinalizationMode::new(route_shape, policy),
            raw,
        );
        PathingRequestOutcome::Resolved {
            path: (path, goal_index),
            diagnostics,
        }
    }

    pub(in crate::game::services) fn request_finalized_with_diagnostics(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
        world_endpoints: ((f32, f32), (f32, f32)),
        direct_segment: Option<((f32, f32), (f32, f32))>,
        allow_pathfinding: bool,
    ) -> PathingRequestOutcome<Vec<(f32, f32)>> {
        let (start_world, goal_world) = world_endpoints;
        let kind = req.kind;
        let route_shape = req.route_shape;
        let policy = req.policy;
        let PathingRequestOutcome::Resolved { path, diagnostics } =
            self.request_with_diagnostics(map, occupancy, req, direct_segment, allow_pathfinding)
        else {
            return PathingRequestOutcome::Deferred;
        };
        if policy != RoutePolicy::FastestTerrainTime {
            return PathingRequestOutcome::Resolved {
                path: super::route_finalize::finalize_reverse_waypoints_or_raw(
                    map,
                    occupancy,
                    kind,
                    start_world,
                    goal_world,
                    super::route_finalize::RouteFinalizationMode::new(route_shape, policy),
                    path,
                ),
                diagnostics,
            };
        }

        let key: FinalizedCacheKey = (
            kind,
            route_shape,
            policy,
            (start_world.0.to_bits(), start_world.1.to_bits()),
            (goal_world.0.to_bits(), goal_world.1.to_bits()),
            occupancy.static_fingerprint_for_kind(kind),
            path.iter()
                .map(|&(x, y)| (x.to_bits(), y.to_bits()))
                .collect(),
        );
        if let Some(path) = self.finalized_cache_lookup(&key) {
            return PathingRequestOutcome::Resolved { path, diagnostics };
        }
        let path = super::route_finalize::finalize_reverse_waypoints_or_raw(
            map,
            occupancy,
            kind,
            start_world,
            goal_world,
            super::route_finalize::RouteFinalizationMode::new(route_shape, policy),
            path,
        );
        self.finalized_cache_insert(key, path.clone());
        PathingRequestOutcome::Resolved { path, diagnostics }
    }
}
