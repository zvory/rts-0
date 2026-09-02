use super::*;
use crate::game::entity::{Entity, FootprintRouting};
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
            if let Some(entity) = entities.get_mut(id) {
                entity.set_path_with_policy(Vec::new(), route_policy_for_source(source));
                entity.set_last_repath_tick(self.tick);
                entity.set_path_goal(Some(goal));
            }
            return PathAttempt::Ready(());
        }

        let goals = build_staging_goals(self.map, self.occ, entities, id, kind, tile_x, tile_y);
        let Some(routing) = prepare_footprint_routing(self, entities, id) else {
            return PathAttempt::Failed;
        };
        match self.request_best_interaction_path(entities, id, &goals, source) {
            PathAttempt::Failed => {
                let next_attempt = routing
                    .attempt
                    .saturating_add(u32::try_from(MAX_REQUESTS_PER_TICK).unwrap_or(u32::MAX));
                let legacy_endpoint_count = goals.len().saturating_add(1);
                if usize::try_from(next_attempt)
                    .is_ok_and(|attempt| attempt < legacy_endpoint_count)
                {
                    set_footprint_routing(
                        entities,
                        id,
                        FootprintRouting {
                            attempt: next_attempt,
                            ..routing
                        },
                    );
                    PathAttempt::Deferred
                } else {
                    PathAttempt::Failed
                }
            }
            attempt => attempt,
        }
    }
}

fn prepare_footprint_routing(
    coordinator: &MoveCoordinator<'_>,
    entities: &mut EntityStore,
    id: u32,
) -> Option<FootprintRouting> {
    let entity = entities.get(id)?;
    let start_tile = coordinator.map.tile_of(entity.pos_x, entity.pos_y);
    let static_fingerprint = coordinator.occ.static_fingerprint_for_kind(entity.kind);
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
