use super::*;

impl PathingService {
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
