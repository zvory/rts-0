use super::*;

type SelectedTilePath = (Vec<(i32, i32)>, Option<usize>);

fn segment_crosses_slow_movement(map: &Map, from: (f32, f32), to: (f32, f32)) -> bool {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    let steps = (dx.hypot(dy) / (config::TILE_SIZE as f32 / 4.0))
        .ceil()
        .max(1.0) as u32;
    (0..=steps).any(|index| {
        let t = index as f32 / steps as f32;
        let (tx, ty) = map.tile_of(from.0 + dx * t, from.1 + dy * t);
        map.is_slow_movement_tile(tx, ty)
    })
}

impl PathingService {
    /// Request a path. Returns world-pixel waypoints in reverse order (next waypoint = pop).
    #[cfg(test)]
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
        let policy = req.policy;
        if let Some((from, to)) = direct_segment {
            if req.policy == RoutePolicy::LegacyShape
                && req.start != req.goal
                && standability::unit_static_segment_standable(map, occupancy, req.kind, from, to)
                && (movement_body_class(req.kind) != MovementBodyClass::InfantryLike
                    || !segment_crosses_slow_movement(map, from, to))
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
                kind,
                radius_tiles: 0,
                route_shape: RouteShape::VehicleClearance,
                policy,
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

    #[cfg(test)]
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
        let pass = self.path_graph.view(
            map,
            occupancy,
            req.kind,
            req.radius_tiles,
            req.route_shape,
            req.policy,
        );

        let search_budget = req.budget.unwrap_or(self.default_budget);
        let static_fingerprint = occupancy.static_fingerprint_for_kind(req.kind);
        let cost_fingerprint = pass.cost_fingerprint();
        if let Some((tile_path, search_expanded_nodes)) = self.cache_lookup(
            &req,
            &pass,
            static_fingerprint,
            cost_fingerprint,
            search_budget,
        ) {
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
                req.route_shape.turn_penalty(req.policy),
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
            cost_fingerprint,
            search_budget,
            tile_path.clone(),
            diagnostics,
        );
        PathingRequestOutcome::Resolved {
            path: tile_path,
            diagnostics,
        }
    }

    pub(in crate::game::services) fn request_tile_path_to_any_with_diagnostics(
        &mut self,
        map: &Map,
        occupancy: &Occupancy,
        req: PathRequest,
        goals: &[(i32, i32)],
        allow_pathfinding: bool,
    ) -> PathingRequestOutcome<SelectedTilePath> {
        if !allow_pathfinding {
            return PathingRequestOutcome::Deferred;
        }
        let pass = self.path_graph.view(
            map,
            occupancy,
            req.kind,
            req.radius_tiles,
            req.route_shape,
            req.policy,
        );
        let search_budget = req.budget.unwrap_or(self.default_budget);
        let (tile_path, expanded_nodes, budget_exhausted, goal_index) =
            pathfinding::find_path_to_any_with_budget_and_turn_cost_with_diagnostics_and_scratch(
                &pass,
                req.start,
                goals,
                search_budget,
                req.route_shape.turn_penalty(req.policy),
                &mut self.search_scratch,
            );
        PathingRequestOutcome::Resolved {
            diagnostics: PathingRequestDiagnostics {
                cache_status: PathCacheStatus::Miss,
                expanded_nodes,
                scheduling_expanded_nodes: expanded_nodes,
                budget_exhausted,
                tile_path_len: tile_path.len(),
            },
            path: (tile_path, goal_index),
        }
    }
}
