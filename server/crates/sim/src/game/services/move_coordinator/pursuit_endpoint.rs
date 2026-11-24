use super::*;

impl MoveCoordinator<'_> {
    /// Keep a short, already-established foot pursuit current without another tile search.
    /// Goal geometry and fire eligibility remain owned by the ordinary combat policy.
    pub(super) fn track_pursuit_endpoint(
        &self,
        entities: &mut EntityStore,
        id: u32,
        target_id: u32,
        goal: (f32, f32),
        min_range_px: f32,
        max_range_px: f32,
    ) -> bool {
        let Some(attacker) = entities.get(id) else {
            return false;
        };
        let Some(target) = entities.get(target_id) else {
            return false;
        };
        // Vehicles retain their steering/rotation route policy. Minimum-range retreats and
        // blocked-shot tank approaches must not become local pursuit shortcuts.
        if uses_oriented_vehicle_body(attacker.kind)
            || min_range_px != 0.0
            || max_range_px <= 0.0
            || !target.is_unit()
            || target.hp == 0
        {
            return false;
        }
        let Some(movement) = attacker.movement.as_ref() else {
            return false;
        };
        let Some(old_goal) = attacker.path_goal() else {
            return false;
        };
        let temporary_pursuit = match attacker.order() {
            Order::Attack(order) => order.intent.target == target_id,
            Order::Idle | Order::AttackMove(_) => {
                attacker.target_id() == Some(target_id) && Some(old_goal) != attacker.move_intent()
            }
            _ => false,
        };
        // Never discard detours or treat a remaining tile waypoint as the final approach.
        if !temporary_pursuit
            || movement.path.len() > 1
            || movement.path.last().is_some_and(|point| *point != old_goal)
        {
            return false;
        }
        let start = (attacker.pos_x(), attacker.pos_y());
        let (mx, my) = target.movement_delta();
        let away = mx * (target.pos_x() - start.0) + my * (target.pos_y() - start.1);
        let limit_sq = (config::TILE_SIZE * config::TILE_SIZE) as f32;
        let segment_sq = (goal.0 - start.0).det_powi(2) + (goal.1 - start.1).det_powi(2);
        let shift_sq = (goal.0 - old_goal.0).det_powi(2) + (goal.1 - old_goal.1).det_powi(2);
        if !away.is_finite()
            || away <= 0.0
            || !segment_sq.is_finite()
            || !shift_sq.is_finite()
            || segment_sq > limit_sq
            || shift_sq > limit_sq
        {
            return false;
        }
        let Some(UnitBody::Circle(mut body)) = unit_body_for_entity(attacker) else {
            return false;
        };
        body.x = goal.0;
        body.y = goal.1;
        if unit_body_for_entity(target)
            .is_none_or(|target_body| unit_bodies_intersect(UnitBody::Circle(body), target_body))
            || !standability::unit_static_segment_standable_with_radius(
                self.map,
                self.occ,
                attacker.kind,
                start,
                goal,
                Some(attacker.radius()),
            )
        {
            return false;
        }
        if let Some(entity) = entities.get_mut(id) {
            // Refreshing a destination is not progress. Let movement retain its stuck
            // accounting and recovery cooldowns while following this same pursuit.
            entity.begin_weapon_teardown_for_movement();
            entity.set_path(vec![goal]);
            entity.set_path_goal(Some(goal));
            entity.mark_attack_phase(AttackPhase::Pursuing);
        }
        true
    }
}

#[cfg(test)]
mod tests;
