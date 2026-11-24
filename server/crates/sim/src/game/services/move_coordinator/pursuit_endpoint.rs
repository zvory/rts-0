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
            entity.begin_weapon_teardown_for_movement();
            entity.set_path(vec![goal]);
            entity.set_path_goal(Some(goal));
            entity.mark_attack_phase(AttackPhase::Pursuing);
            entity.reset_stuck(start.0, start.1);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::terrain;

    fn fixture() -> (Map, EntityStore, u32, u32) {
        let map = Map {
            width: 40,
            height: 40,
            terrain: vec![terrain::GRASS; 1600],
            ..Default::default()
        };
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::WindAdept, 410.0, 600.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::ScoutCar, 500.0, 600.0)
            .unwrap();
        entities
            .get_mut(target)
            .unwrap()
            .set_movement_delta(2.35, 0.0);
        let unit = entities.get_mut(attacker).unwrap();
        unit.set_order(Order::attack(target));
        unit.set_path(vec![(420.0, 600.0)]);
        unit.set_path_goal(Some((420.0, 600.0)));
        unit.set_last_repath_tick(10);
        (map, entities, attacker, target)
    }

    #[test]
    fn terminal_tracking_updates_rotated_hull_goals_without_search_budget() {
        for angle in [0.0, std::f32::consts::FRAC_PI_2, 0.73] {
            let (map, mut entities, attacker, target) = fixture();
            entities.get_mut(target).unwrap().set_facing(angle);
            let occ = Occupancy::build(&map, &entities);
            let mut pathing = PathingService::new(8192, 256);
            let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 11);
            let budget = coordinator.budget;
            assert!(coordinator.request_direct_attack_path(
                &mut entities,
                attacker,
                target,
                0.0,
                54.0
            ));
            assert_eq!(
                coordinator.budget, budget,
                "local tracking must not consume tile searches"
            );
            let unit = entities.get(attacker).unwrap();
            let point = primary_combat_target_point(
                &map,
                unit.default_weapon_kind().unwrap(),
                (410.0, 600.0),
                entities.get(target).unwrap(),
            );
            let expected =
                direct_attack_staging_goal(&map, (410.0, 600.0), point, 0.0, 54.0).unwrap();
            assert_eq!(unit.path_goal(), Some(expected));
            assert_eq!(unit.next_waypoint(), Some(expected));
            assert_eq!(
                unit.last_repath_tick(),
                10,
                "tracking is not a new tile search"
            );
        }
    }

    #[test]
    fn terminal_tracking_preserves_stationary_approaching_detour_and_travel_paths() {
        for case in [
            "stationary",
            "approaching",
            "detour",
            "travel",
            "minimum-range",
            "blocked",
            "large-jump",
        ] {
            let (mut map, mut entities, attacker, target) = fixture();
            match case {
                "stationary" => entities
                    .get_mut(target)
                    .unwrap()
                    .set_movement_delta(0.0, 0.0),
                "approaching" => entities
                    .get_mut(target)
                    .unwrap()
                    .set_movement_delta(-2.35, 0.0),
                "detour" => entities
                    .get_mut(attacker)
                    .unwrap()
                    .set_path(vec![(420.0, 600.0), (410.0, 632.0)]),
                "travel" => {
                    let unit = entities.get_mut(attacker).unwrap();
                    unit.set_order(Order::attack_move_to(420.0, 600.0));
                    unit.set_target_id(Some(target));
                }
                "blocked" => {
                    let tile = (600 / 32 * 40 + 13) as usize;
                    map.terrain[tile] = terrain::WATER;
                }
                "large-jump" => entities.get_mut(target).unwrap().set_position(700.0, 600.0),
                _ => {}
            }
            let before = entities
                .get(attacker)
                .unwrap()
                .movement
                .as_ref()
                .unwrap()
                .path
                .clone();
            let occ = Occupancy::build(&map, &entities);
            let mut pathing = PathingService::new(8192, 256);
            let coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 11);
            let min = if case == "minimum-range" { 10.0 } else { 0.0 };
            let goal = if case == "large-jump" {
                (630.0, 600.0)
            } else {
                (428.6, 600.0)
            };
            assert!(
                !coordinator.track_pursuit_endpoint(
                    &mut entities,
                    attacker,
                    target,
                    goal,
                    min,
                    54.0
                ),
                "{case}"
            );
            assert_eq!(
                entities
                    .get(attacker)
                    .unwrap()
                    .movement
                    .as_ref()
                    .unwrap()
                    .path,
                before,
                "{case}"
            );
        }
    }
}
