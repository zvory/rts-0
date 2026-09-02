use super::*;

const MAX_INTERACTION_GOALS: usize = 256;

impl MoveCoordinator<'_> {
    pub(super) fn can_repath_to_any(
        &self,
        entities: &EntityStore,
        id: u32,
        goals: &[(f32, f32)],
    ) -> bool {
        let Some(entity) = entities.get(id).filter(|entity| entity.is_unit()) else {
            return false;
        };
        if self.tick.saturating_sub(entity.last_repath_tick()) >= MIN_REPATH_TICKS {
            return true;
        }
        let Some(old_goal) = entity.path_goal() else {
            return true;
        };
        !goals.iter().any(|goal| {
            (old_goal.0 - goal.0).abs() <= MATERIAL_GOAL_DELTA_PX
                && (old_goal.1 - goal.1).abs() <= MATERIAL_GOAL_DELTA_PX
        })
    }

    pub(super) fn request_best_interaction_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        goals: &[(f32, f32)],
        source: PathingRequestSource,
    ) -> PathAttempt {
        let request_start = self.diagnostics.as_ref().map(|_| Instant::now());
        let Some(entity) = entities.get(id).filter(|entity| entity.is_unit()) else {
            return PathAttempt::Failed;
        };
        let start_world = (entity.pos_x, entity.pos_y);
        let start = self.map.tile_of(start_world.0, start_world.1);
        let kind = entity.kind;
        let candidates = goals
            .iter()
            .copied()
            .map(|goal| {
                let tile = self.map.tile_of(goal.0, goal.1);
                ((tile.0 as i32, tile.1 as i32), goal)
            })
            .collect::<Vec<_>>();
        let Some(first_candidate) = candidates.first() else {
            return PathAttempt::Failed;
        };
        let route_shape = if uses_oriented_vehicle_body(kind) {
            RouteShape::VehicleClearance
        } else {
            RouteShape::Normal
        };
        let policy = route_policy_for_source(source);
        let req = PathRequest {
            kind,
            start: (start.0 as i32, start.1 as i32),
            goal: first_candidate.0,
            radius_tiles: config::unit_radius_tiles(kind),
            route_shape,
            policy,
            budget: None,
        };
        let PathingRequestOutcome::Resolved {
            path: (mut waypoints, goal_index),
            diagnostics,
        } = self.pathing.request_best_finalized_with_diagnostics(
            self.map,
            self.occ,
            req,
            start_world,
            &candidates,
            self.budget > 0,
        ) else {
            return PathAttempt::Deferred;
        };
        self.consume_request_budget(Some(diagnostics));
        let Some(goal) = goal_index.and_then(|index| goals.get(index).copied()) else {
            self.record_path_request(
                source,
                false,
                false,
                Some(diagnostics),
                request_start.map(|start| start.elapsed()).unwrap_or_default(),
            );
            return PathAttempt::Failed;
        };
        if waypoints.is_empty()
            && (goal.0 - start_world.0).hypot(goal.1 - start_world.1)
                > EXACT_GOAL_ARRIVAL_EPS_PX
        {
            waypoints.push(goal);
        }
        if let Some(entity) = entities.get_mut(id) {
            entity.set_path_with_policy(waypoints, policy);
            entity.set_last_repath_tick(self.tick);
            entity.set_path_goal(Some(goal));
            if matches!(entity.order(), Order::Attack(_)) {
                entity.mark_attack_phase(AttackPhase::Pursuing);
            }
        }
        self.record_path_request(
            source,
            true,
            false,
            Some(diagnostics),
            request_start.map(|start| start.elapsed()).unwrap_or_default(),
        );
        PathAttempt::Ready(())
    }
}

pub(super) fn direct_attack_staging_goal(
    map: &Map,
    attacker: (f32, f32),
    target: (f32, f32),
    min_range_px: f32,
    max_range_px: f32,
) -> Option<(f32, f32)> {
    if !attacker.0.is_finite()
        || !attacker.1.is_finite()
        || !target.0.is_finite()
        || !target.1.is_finite()
        || !min_range_px.is_finite()
        || !max_range_px.is_finite()
        || min_range_px < 0.0
        || max_range_px < min_range_px
    {
        return None;
    }
    let dx = attacker.0 - target.0;
    let dy = attacker.1 - target.1;
    let distance = dx.hypot(dy);
    if distance >= min_range_px && distance <= max_range_px {
        return None;
    }
    let margin = 4.0;
    let desired_distance = if distance > max_range_px {
        (max_range_px - margin).max(min_range_px)
    } else {
        (min_range_px + margin).min(max_range_px)
    };
    let (dir_x, dir_y) = if distance > f32::EPSILON {
        (dx / distance, dy / distance)
    } else {
        (1.0, 0.0)
    };
    let world_max_x = map.world_width_px() - 0.01;
    let world_max_y = map.world_height_px() - 0.01;
    Some((
        (target.0 + dir_x * desired_distance).clamp(0.0, world_max_x),
        (target.1 + dir_y * desired_distance).clamp(0.0, world_max_y),
    ))
}

pub(super) fn direct_attack_staging_goals(
    map: &Map,
    attacker: (f32, f32),
    target: &Entity,
    min_range_px: f32,
    max_range_px: f32,
    geometric_goal: (f32, f32),
) -> Vec<(f32, f32)> {
    let target_point = closest_combat_target_point(map, attacker, target);
    let distance = (attacker.0 - target_point.0).hypot(attacker.1 - target_point.1);
    let desired_distance = if distance > max_range_px {
        (max_range_px - 4.0).max(min_range_px)
    } else {
        (min_range_px + 4.0).min(max_range_px)
    };
    let target_tile = map.tile_of(target.pos_x, target.pos_y);
    let target_extent_tiles = config::building_stats(target.kind)
        .map(|stats| stats.foot_w.max(stats.foot_h))
        .unwrap_or(1);
    let radius_tiles = ((max_range_px / config::TILE_SIZE as f32).ceil() as i32)
        .saturating_add(i32::try_from(target_extent_tiles).unwrap_or(i32::MAX))
        .saturating_add(1);
    let target_tx = i32::try_from(target_tile.0).unwrap_or(i32::MAX);
    let target_ty = i32::try_from(target_tile.1).unwrap_or(i32::MAX);
    let mut candidates = Vec::new();
    for ty in target_ty.saturating_sub(radius_tiles)..=target_ty.saturating_add(radius_tiles) {
        for tx in target_tx.saturating_sub(radius_tiles)..=target_tx.saturating_add(radius_tiles) {
            if !map.in_bounds(tx, ty) {
                continue;
            }
            let goal = map.tile_center(tx as u32, ty as u32);
            let closest = closest_combat_target_point(map, goal, target);
            let range = (goal.0 - closest.0).hypot(goal.1 - closest.1);
            if range >= min_range_px && range <= max_range_px {
                candidates.push(((range - desired_distance).abs(), (tx, ty), goal));
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    let mut goals = Vec::with_capacity(MAX_INTERACTION_GOALS.min(candidates.len() + 1));
    goals.push(geometric_goal);
    for (_, _, goal) in candidates {
        if goals.len() >= MAX_INTERACTION_GOALS {
            break;
        }
        if map.tile_of(goal.0, goal.1) != map.tile_of(geometric_goal.0, geometric_goal.1) {
            goals.push(goal);
        }
    }
    goals
}
