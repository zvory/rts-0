use super::*;
use crate::game::entity::Entity;
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
        let footprint = footprint_tiles(kind, tile_x, tile_y);
        if footprint.is_empty() {
            return PathAttempt::Failed;
        }
        let footprint_set: BTreeSet<(u32, u32)> = footprint.into_iter().collect();
        if let Some(goal) = current_staging_goal(self.map, entities, id, kind, &footprint_set) {
            set_entity_path(entities, id, Vec::new(), goal, self.tick);
            return PathAttempt::Ready(());
        }

        let Some(mut routing) = self.prepare_footprint_routing(entities, id) else {
            return PathAttempt::Failed;
        };
        if routing.attempt == 0 {
            match self.request_footprint_approach_path(
                entities,
                id,
                kind,
                (tile_x, tile_y),
                &footprint_set,
                source,
            ) {
                PathAttempt::Ready(()) => return PathAttempt::Ready(()),
                PathAttempt::Deferred => return PathAttempt::Deferred,
                PathAttempt::Failed => {
                    routing.attempt = 1;
                    set_footprint_routing(entities, id, routing);
                }
            }
        }

        for (index, goal) in
            build_staging_goals(self.map, self.occ, entities, id, kind, tile_x, tile_y)
                .into_iter()
                .enumerate()
        {
            let candidate_attempt = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
            if candidate_attempt < routing.attempt {
                continue;
            }
            match self.request_exact_path_to_build_goal(entities, id, goal, source) {
                PathAttempt::Ready(()) => return PathAttempt::Ready(()),
                PathAttempt::Deferred => return PathAttempt::Deferred,
                PathAttempt::Failed => {
                    routing.attempt = candidate_attempt.saturating_add(1);
                    set_footprint_routing(entities, id, routing);
                }
            }
        }
        PathAttempt::Failed
    }

    fn prepare_footprint_routing(
        &self,
        entities: &mut EntityStore,
        id: u32,
    ) -> Option<FootprintRouting> {
        let entity = entities.get(id)?;
        let start_tile = self.map.tile_of(entity.pos_x, entity.pos_y);
        let static_fingerprint = self.occ.static_fingerprint_for_kind(entity.kind);
        let current = footprint_routing(entity)?;
        let routing = if current.static_fingerprint == Some(static_fingerprint)
            && current.start_tile == Some(start_tile)
        {
            current
        } else {
            FootprintRouting {
                attempt: 0,
                static_fingerprint: Some(static_fingerprint),
                start_tile: Some(start_tile),
            }
        };
        set_footprint_routing(entities, id, routing);
        Some(routing)
    }

    fn request_footprint_approach_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        kind: EntityKind,
        target_tile: (u32, u32),
        footprint_set: &BTreeSet<(u32, u32)>,
        source: PathingRequestSource,
    ) -> PathAttempt {
        let approach_goal = self.map.tile_center(target_tile.0, target_tile.1);
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
        if !build_staging_goal_in_range(self.map, kind, target_tile.0, target_tile.1, goal) {
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
        let (unit_kind, sx, sy) = match entities.get(id) {
            Some(e) if e.is_unit() => {
                let (sx, sy) = self.map.tile_of(e.pos_x, e.pos_y);
                (e.kind, sx, sy)
            }
            _ => return PathAttempt::Failed,
        };
        let (gx, gy) = self.map.tile_of(goal.0, goal.1);
        let radius_tiles = config::unit_radius_tiles(unit_kind);
        let req = PathRequest {
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

fn set_footprint_routing(entities: &mut EntityStore, id: u32, routing: FootprintRouting) {
    let Some(movement) = entities
        .get_mut(id)
        .and_then(|entity| entity.movement.as_mut())
    else {
        return;
    };
    match &mut movement.order {
        Order::Build(order) => order.execution.routing = routing,
        Order::Deconstruct(order) => order.execution.routing = routing,
        _ => {}
    }
}

fn footprint_routing(entity: &Entity) -> Option<FootprintRouting> {
    match &entity.movement.as_ref()?.order {
        Order::Build(order) => Some(order.execution.routing),
        Order::Deconstruct(order) => Some(order.execution.routing),
        _ => None,
    }
}

fn set_entity_path(
    entities: &mut EntityStore,
    id: u32,
    path: Vec<(f32, f32)>,
    goal: (f32, f32),
    tick: u32,
) {
    if let Some(entity) = entities.get_mut(id) {
        entity.set_path(path);
        entity.set_last_repath_tick(tick);
        entity.set_path_goal(Some(goal));
    }
}

fn current_staging_goal(
    map: &Map,
    entities: &EntityStore,
    id: u32,
    kind: EntityKind,
    footprint: &BTreeSet<(u32, u32)>,
) -> Option<(f32, f32)> {
    let worker = entities.get(id)?;
    let tile = map.tile_of(worker.pos_x, worker.pos_y);
    if footprint.contains(&tile) {
        return None;
    }
    let &(tile_x, tile_y) = footprint.iter().min()?;
    let goal = (worker.pos_x, worker.pos_y);
    build_staging_goal_in_range(map, kind, tile_x, tile_y, goal).then_some(goal)
}

pub(super) fn build_staging_goal_in_range(
    map: &Map,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
    goal: (f32, f32),
) -> bool {
    let (cx, cy) = footprint_center(map, kind, tile_x, tile_y);
    let dx = goal.0 - cx;
    let dy = goal.1 - cy;
    dx * dx + dy * dy <= interact_range_for_kind(kind).powi(2)
}

/// Pick a walk target outside a build footprint.
#[cfg(test)]
pub(super) fn build_staging_goal(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Option<(f32, f32)> {
    build_staging_goals(map, occ, entities, worker, kind, tile_x, tile_y)
        .into_iter()
        .next()
}

fn build_staging_goals(
    map: &Map,
    occ: &Occupancy,
    entities: &EntityStore,
    worker: u32,
    kind: EntityKind,
    tile_x: u32,
    tile_y: u32,
) -> Vec<(f32, f32)> {
    let Some(worker) = entities.get(worker) else {
        return Vec::new();
    };
    let footprint = footprint_tiles(kind, tile_x, tile_y);
    let Some(stats) = config::building_stats(kind) else {
        return Vec::new();
    };
    if footprint.is_empty() {
        return Vec::new();
    }
    let worker_tile = map.tile_of(worker.pos_x, worker.pos_y);
    let worker_start = map.tile_center(worker_tile.0, worker_tile.1);
    let footprint_set: BTreeSet<(u32, u32)> = footprint.iter().copied().collect();
    let min_x = tile_x as i32;
    let min_y = tile_y as i32;
    let Some(max_x) = tile_x.checked_add(stats.foot_w.saturating_sub(1)) else {
        return Vec::new();
    };
    let Some(max_y) = tile_y.checked_add(stats.foot_h.saturating_sub(1)) else {
        return Vec::new();
    };
    let max_x = max_x as i32;
    let max_y = max_y as i32;
    let mut candidates = Vec::new();

    for r in 1i32..=6 {
        for ty in (min_y - r)..=(max_y + r) {
            for tx in (min_x - r)..=(max_x + r) {
                if tx > min_x - r && tx < max_x + r && ty > min_y - r && ty < max_y + r {
                    continue;
                }
                if !map.in_bounds(tx, ty) {
                    continue;
                }
                let tile = (tx as u32, ty as u32);
                if footprint_set.contains(&tile)
                    || !map.is_passable(tx, ty)
                    || !occ.passable_for_kind(tx, ty, worker.kind)
                {
                    continue;
                }
                let center = map.tile_center(tile.0, tile.1);
                if !build_staging_goal_in_range(map, kind, tile_x, tile_y, center) {
                    continue;
                }
                let dx = worker_start.0 - center.0;
                let dy = worker_start.1 - center.1;
                candidates.push((r, dx * dx + dy * dy, tile));
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    candidates
        .into_iter()
        .map(|(_, _, tile)| map.tile_center(tile.0, tile.1))
        .collect()
}
