use super::*;
use crate::game::pathfinding;

impl MoveCoordinator<'_> {
    pub(super) fn plan_footprint_interaction_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        tile_x: u32,
        tile_y: u32,
        source: PathingRequestSource,
    ) -> PathAttempt {
        match self.request_build_path(entities, id, kind, tile_x, tile_y, source) {
            PathAttempt::Ready(()) => return PathAttempt::Ready(()),
            PathAttempt::Deferred => return PathAttempt::Deferred,
            PathAttempt::Failed => {}
        }
        for goal in build_staging_goals(self.map, self.occ, entities, id, kind, tile_x, tile_y) {
            match self.request_exact_path_to_build_goal(entities, id, goal, source) {
                PathAttempt::Ready(()) => return PathAttempt::Ready(()),
                PathAttempt::Deferred => return PathAttempt::Deferred,
                PathAttempt::Failed => {}
            }
        }
        PathAttempt::Failed
    }

    fn request_build_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        tile_x: u32,
        tile_y: u32,
        source: PathingRequestSource,
    ) -> PathAttempt {
        let footprint = footprint_tiles(kind, tile_x, tile_y);
        if footprint.is_empty() {
            return PathAttempt::Failed;
        }
        let footprint_set: BTreeSet<(u32, u32)> = footprint.iter().copied().collect();
        if let Some(goal) = current_staging_goal(self.map, entities, id, kind, &footprint_set) {
            set_entity_path(entities, id, Vec::new(), goal, self.tick);
            return PathAttempt::Ready(());
        }

        let approach_goal = self.map.tile_center(tile_x, tile_y);
        let tile_path = match self.request_exact_tile_path(entities, id, approach_goal, source) {
            PathAttempt::Ready(path) => path,
            PathAttempt::Failed => return PathAttempt::Failed,
            PathAttempt::Deferred => return PathAttempt::Deferred,
        };
        let Some(staging_index) = tile_path.iter().rposition(|(tx, ty)| {
            *tx >= 0 && *ty >= 0 && !footprint_set.contains(&(*tx as u32, *ty as u32))
        }) else {
            return PathAttempt::Failed;
        };
        let Some(&staging_tile) = tile_path.get(staging_index) else {
            return PathAttempt::Failed;
        };
        let goal = self
            .map
            .tile_center(staging_tile.0 as u32, staging_tile.1 as u32);
        if !build_staging_goal_in_range(self.map, kind, tile_x, tile_y, goal) {
            return PathAttempt::Failed;
        }
        let Some(trimmed) = tile_path.get(..=staging_index) else {
            return PathAttempt::Failed;
        };
        let waypoints = pathfinding::to_world_waypoints(trimmed);
        set_entity_path(entities, id, waypoints, goal, self.tick);
        PathAttempt::Ready(())
    }

    fn request_exact_path_to_build_goal(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        goal: (f32, f32),
        source: PathingRequestSource,
    ) -> PathAttempt {
        let tile_path = match self.request_exact_tile_path(entities, id, goal, source) {
            PathAttempt::Ready(path) => path,
            PathAttempt::Failed => return PathAttempt::Failed,
            PathAttempt::Deferred => return PathAttempt::Deferred,
        };
        let waypoints = pathfinding::to_world_waypoints(&tile_path);
        set_entity_path(entities, id, waypoints, goal, self.tick);
        PathAttempt::Ready(())
    }

    fn request_exact_tile_path(
        &mut self,
        entities: &EntityStore,
        id: u32,
        goal: (f32, f32),
        source: PathingRequestSource,
    ) -> PathAttempt<Vec<(i32, i32)>> {
        let request_start = self.diagnostics.as_ref().map(|_| Instant::now());
        let (unit_owner, unit_kind, sx, sy) = match entities.get(id) {
            Some(e) if e.is_unit() => {
                let (sx, sy) = self.map.tile_of(e.pos_x, e.pos_y);
                (e.owner, e.kind, sx, sy)
            }
            _ => return PathAttempt::Failed,
        };
        let (gx, gy) = self.map.tile_of(goal.0, goal.1);
        let radius_tiles = config::unit_radius_tiles(unit_kind);
        let req = PathRequest {
            relation: StaticPathingRelation::for_player(unit_owner, &self.teams),
            kind: unit_kind,
            start: (sx as i32, sy as i32),
            goal: (gx as i32, gy as i32),
            radius_tiles,
            route_shape: RouteShape::Normal,
            budget: None,
        };
        let PathingRequestOutcome::Resolved {
            path: tile_path,
            diagnostics: request_diagnostics,
        } = self.pathing.request_tile_path_with_diagnostics(
            self.map,
            self.occ,
            req,
            self.budget > 0,
        )
        else {
            return PathAttempt::Deferred;
        };
        self.consume_request_budget(Some(request_diagnostics));
        let path_ok = tile_path.last().copied() == Some((gx as i32, gy as i32));
        self.record_path_request(
            source,
            path_ok,
            false,
            Some(request_diagnostics),
            request_start
                .map(|start| start.elapsed())
                .unwrap_or_default(),
        );
        if path_ok {
            PathAttempt::Ready(tile_path)
        } else {
            PathAttempt::Failed
        }
    }
}
