use super::*;

impl PathingService {
    /// Request a path. Returns world-pixel waypoints in reverse order (next waypoint = pop).
    #[allow(dead_code)]
    pub fn request(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
    ) -> Vec<(f32, f32)> {
        match self.request_with_diagnostics(map, occupancy, req, None, true) {
            PathingRequestOutcome::Resolved { path, .. } => path,
            PathingRequestOutcome::Deferred => Vec::new(),
        }
    }

    pub(in crate::game::services) fn request_with_diagnostics(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
        direct_segment: Option<((f32, f32), (f32, f32))>,
        allow_pathfinding: bool,
    ) -> PathingRequestOutcome<Vec<(f32, f32)>> {
        let start = req.start;
        let kind = req.kind;
        let relation = req.relation();
        if let Some((from, to)) = direct_segment {
            if req.start != req.goal
                && standability::unit_static_segment_standable(map, occupancy, req.kind, from, to)
            {
                return PathingRequestOutcome::Resolved {
                    path: vec![to],
                    diagnostics: PathingRequestDiagnostics {
                        cache_status: PathCacheStatus::Bypassed,
                        expanded_nodes: 0,
                        scheduling_expanded_nodes: 0,
                        budget_exhausted: false,
                        tile_path_len: 1,
                    },
                };
            }
        }

        let PathingRequestOutcome::Resolved {
            path: tile_path,
            diagnostics,
        } = self.request_tile_path_with_diagnostics(map, occupancy, req, allow_pathfinding)
        else {
            return PathingRequestOutcome::Deferred;
        };
        if uses_pivot_vehicle_movement(kind) {
            let pass = TerrainPassability {
                map,
                occupancy,
                relation,
                kind,
                radius_tiles: 0,
                route_shape: RouteShape::VehicleClearance,
                avoid_diagonal_pinch: true,
            };
            let tile_path = expand_vehicle_diagonal_steps_to_l_waypoints(start, &tile_path, &pass);
            return PathingRequestOutcome::Resolved {
                path: pathfinding::to_world_waypoints(&tile_path),
                diagnostics,
            };
        }
        PathingRequestOutcome::Resolved {
            path: pathfinding::to_world_waypoints(&tile_path),
            diagnostics,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn request_tile_path(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
    ) -> Vec<(i32, i32)> {
        match self.request_tile_path_with_diagnostics(map, occupancy, req, true) {
            PathingRequestOutcome::Resolved { path, .. } => path,
            PathingRequestOutcome::Deferred => Vec::new(),
        }
    }

    pub(in crate::game::services) fn request_tile_path_with_diagnostics(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
        allow_pathfinding: bool,
    ) -> PathingRequestOutcome<Vec<(i32, i32)>> {
        // Cache residency is rebuildable state, so it must not decide whether an authoritative
        // request resolves this tick. Direct routes are handled above; all tile-path requests,
        // hits and misses alike, use the same coordinator-owned scheduling allowance.
        if !allow_pathfinding {
            return PathingRequestOutcome::Deferred;
        }
        let pass = TerrainPassability {
            map,
            occupancy,
            relation: req.relation(),
            kind: req.kind,
            radius_tiles: req.radius_tiles,
            route_shape: req.route_shape,
            avoid_diagonal_pinch: uses_oriented_vehicle_body(req.kind),
        };

        let search_budget = req.budget.unwrap_or(self.default_budget);
        let static_fingerprint =
            occupancy.static_fingerprint_for_kind_and_relation(req.kind, &req.relation);
        if let Some((tile_path, search_expanded_nodes)) =
            self.cache_lookup(&req, &pass, static_fingerprint, search_budget)
        {
            let diagnostics = PathingRequestDiagnostics {
                cache_status: PathCacheStatus::Hit,
                expanded_nodes: 0,
                scheduling_expanded_nodes: search_expanded_nodes,
                budget_exhausted: false,
                tile_path_len: tile_path.len(),
            };
            return PathingRequestOutcome::Resolved {
                path: tile_path,
                diagnostics,
            };
        }

        let (tile_path, expanded_nodes, budget_exhausted) =
            pathfinding::find_path_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                req.start,
                req.goal,
                search_budget,
                req.route_shape.turn_penalty(),
                &mut self.search_scratch,
            );

        let diagnostics = PathingRequestDiagnostics {
            cache_status: PathCacheStatus::Miss,
            expanded_nodes,
            scheduling_expanded_nodes: expanded_nodes,
            budget_exhausted,
            tile_path_len: tile_path.len(),
        };
        // The effective budget is part of the key, so best-effort results cannot poison a later
        // request with a larger allowance. Memoizing them avoids repeating identical bounded work.
        self.cache_insert(
            &req,
            static_fingerprint,
            search_budget,
            tile_path.clone(),
            diagnostics,
        );
        PathingRequestOutcome::Resolved {
            path: tile_path,
            diagnostics,
        }
    }
}
