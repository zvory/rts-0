use super::*;
use crate::game::entity::Entity;

#[test]
fn committed_escape_uses_its_travel_direction_for_slope_speed() {
    let mut map = flat_map(1);
    map.elevation.fill(1);
    let low = map.index(19, 20);
    let high = map.index(21, 20);
    map.elevation[low] = 0;
    map.elevation[high] = 2;
    let start = map.tile_center(20, 20);
    for sign in [-1.0, 1.0] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, EntityKind::Tank, start.0, start.1)
            .unwrap();
        let e = entities.get_mut(id).unwrap();
        e.set_facing(0.0);
        // The retained route points opposite the committed escape.
        let route_goal = (start.0 - sign * 100.0, start.1);
        e.replace_active_order(Order::move_to(route_goal.0, route_goal.1));
        e.set_path(vec![route_goal]);
        e.movement.as_mut().unwrap().pivot_recovery = Some(crate::game::entity::PivotRecovery {
            goal: (start.0 + sign * config::TILE_SIZE as f32, start.1),
            facing: 0.0,
            ticks_left: 30,
        });
        let expected_speed = e.movement_speed()
            * map.elevation_movement_multiplier_at(
                start.0,
                start.1,
                (sign * config::TILE_SIZE as f32, 0.0),
            );
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, 0);
        let e = entities.get(id).unwrap();
        assert!(
            ((e.pos_x() - start.0) * sign - expected_speed).abs() < 0.001,
            "escape direction {sign}: moved {} instead of {expected_speed}",
            (e.pos_x() - start.0) * sign
        );
        assert_eq!(e.pos_y(), start.1);
    }
}

#[test]
fn replay_530_tank_355_escapes_water_without_other_units_or_sideways_motion() {
    let map = Map::load("Crossroads", 2, 2967932699).expect("recorded map");
    let mut entities = EntityStore::new();
    // Match 530, tick 15000, original build 4f48dc8d2569. This pose stayed frozen
    // for 600 ticks after deleting every other unit/building in the original simulation.
    let recorded: Entity = serde_json::from_str(include_str!("fixtures/replay_530_tank_355.json"))
        .expect("recorded tank");
    let start = (recorded.pos_x(), recorded.pos_y());
    let id = entities.insert(recorded);
    for tick in 0..300 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let before = entities.get(id).expect("tank");
        let pos = (before.pos_x(), before.pos_y());
        let facing = before.facing();
        let recovering = before
            .movement
            .as_ref()
            .and_then(|m| m.pivot_recovery)
            .is_some();
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let e = entities.get(id).expect("tank");
        assert!(
            tank_standable_at_entity_facing(&map, &occ, &entities, id),
            "illegal hull at tick{tick}"
        );
        if recovering {
            assert!((e.facing() - facing).abs() < 0.001);
            let lateral =
                (e.pos_x() - pos.0) * (-facing.det_sin()) + (e.pos_y() - pos.1) * facing.det_cos();
            assert!(lateral.abs() < 0.01, "escape must remain along hull");
        }
    }
    let e = entities.get(id).expect("tank");
    assert!(
        moved_distance(start, (e.pos_x(), e.pos_y())) > 32.0,
        "tank must resume its route"
    );
}

#[test]
fn committed_escape_survives_entity_round_trip_and_new_order_cancels_it() {
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 300.0, 300.0)
        .unwrap();
    let e = entities.get_mut(id).unwrap();
    e.movement.as_mut().unwrap().pivot_recovery = Some(crate::game::entity::PivotRecovery {
        goal: (290.0, 300.0),
        facing: 0.0,
        ticks_left: 30,
    });
    let mut restored: Entity = serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap();
    assert_eq!(
        restored
            .movement
            .as_ref()
            .unwrap()
            .pivot_recovery
            .unwrap()
            .goal,
        (290.0, 300.0)
    );
    restored.replace_active_order(Order::move_to(400.0, 400.0));
    assert!(restored.movement.as_ref().unwrap().pivot_recovery.is_none());
}

#[test]
fn tree_corner_escape_can_commit_before_the_first_turn_is_clear() {
    let mut map = flat_map(1);
    let (cx, cy) = map.tile_center(20, 20);
    map.doodads.push(crate::protocol::MapDoodad {
        id: 1,
        type_id: "tree.alder".into(),
        x: cx as u32,
        y: cy as u32,
        color: None,
    });
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, cx - 30.0, cy)
        .unwrap();
    let facing = 1.78_f32;
    let rotated = facing + TANK_BODY_TURN_RATE_RAD_PER_TICK;
    let speed = config::unit_stats(EntityKind::Tank).unwrap().speed;
    let mut chosen = None;
    // Find a contact where neither one-speed escape could also turn immediately.
    'search: for ix in 0..160 {
        for iy in 0..160 {
            let pos = (cx - 40.0 + ix as f32 * 0.5, cy - 40.0 + iy as f32 * 0.5);
            let occ = Occupancy::build(&map, &entities);
            let legal = |p: (f32, f32), f| {
                standability::unit_static_standable_with_facing(
                    &map,
                    &occ,
                    EntityKind::Tank,
                    p.0,
                    p.1,
                    f,
                )
            };
            if !legal(pos, facing) || legal(pos, rotated) {
                continue;
            }
            if [1.0_f32, -1.0].iter().any(|sign| {
                legal(
                    (
                        pos.0 + facing.det_cos() * speed * sign,
                        pos.1 + facing.det_sin() * speed * sign,
                    ),
                    rotated,
                )
            }) {
                continue;
            }
            let e = entities.get_mut(id).unwrap();
            e.set_position(pos.0, pos.1);
            e.set_facing(facing);
            super::super::pivot_recovery::begin(e, &map, &occ, pos, rotated, speed, &[]);
            if e.movement.as_ref().unwrap().pivot_recovery.is_some() {
                chosen = Some(pos);
                break 'search;
            }
        }
    }
    let start = chosen.expect("tree contact requiring multiple old-hull steps");
    let goal = entities
        .get(id)
        .unwrap()
        .movement
        .as_ref()
        .unwrap()
        .pivot_recovery
        .unwrap()
        .goal;
    let mut steps = 0;
    while entities
        .get(id)
        .unwrap()
        .movement
        .as_ref()
        .unwrap()
        .pivot_recovery
        .is_some()
    {
        let occ = Occupancy::build(&map, &entities);
        assert!(super::super::pivot_recovery::advance(
            entities.get_mut(id).unwrap(),
            &map,
            &occ,
            speed,
            &[]
        ));
        assert!(tank_standable_at_entity_facing(&map, &occ, &entities, id));
        steps += 1;
        assert!(steps < 100);
    }
    assert!(steps > 1);
    assert!(moved_distance(start, goal) > speed);
    let occ = Occupancy::build(&map, &entities);
    assert!(standability::unit_static_standable_with_facing(
        &map,
        &occ,
        EntityKind::Tank,
        goal.0,
        goal.1,
        rotated
    ));
}

#[test]
fn recorded_water_pose_can_escape_before_attempting_the_blocked_turn() {
    let map = Map::load("Crossroads", 2, 2967932699).unwrap();
    let mut e: Entity =
        serde_json::from_str(include_str!("fixtures/replay_530_tank_355.json")).unwrap();
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let pos = (e.pos_x(), e.pos_y());
    let facing = e.facing();
    let rotated = facing + TANK_BODY_TURN_RATE_RAD_PER_TICK;
    let speed = config::unit_stats(EntityKind::Tank).unwrap().speed;
    let legal = |p: (f32, f32), f| {
        standability::unit_static_standable_with_facing(&map, &occ, EntityKind::Tank, p.0, p.1, f)
    };
    assert!(legal(pos, facing));
    assert!(!legal(pos, rotated));
    for sign in [1.0_f32, -1.0] {
        assert!(!legal(
            (
                pos.0 + facing.det_cos() * speed * sign,
                pos.1 + facing.det_sin() * speed * sign
            ),
            rotated
        ));
    }
    super::super::pivot_recovery::begin(&mut e, &map, &occ, pos, rotated, speed, &[]);
    assert!(e.movement.as_ref().unwrap().pivot_recovery.is_some());
    let mut steps = 0;
    while e.movement.as_ref().unwrap().pivot_recovery.is_some() {
        assert!(super::super::pivot_recovery::advance(
            &mut e,
            &map,
            &occ,
            speed,
            &[]
        ));
        assert!(legal((e.pos_x(), e.pos_y()), e.facing()));
        assert_eq!(e.facing(), facing);
        steps += 1;
        assert!(steps < 100);
    }
    assert!(steps > 1);
    assert!(legal((e.pos_x(), e.pos_y()), rotated));
}

#[test]
fn committed_escape_stops_when_another_unit_enters_its_corridor() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    let e = entities.get_mut(id).unwrap();
    e.movement.as_mut().unwrap().pivot_recovery = Some(crate::game::entity::PivotRecovery {
        goal: (508.0, 500.0),
        facing: 0.0,
        ticks_left: 30,
    });
    let other = unit_body_with_facing(EntityKind::Tank, 530.0, 500.0, 0.0).unwrap();
    let occ = Occupancy::build(&map, &EntityStore::new());
    assert!(!super::super::pivot_recovery::advance(
        e,
        &map,
        &occ,
        2.0,
        &[other]
    ));
    assert_eq!((e.pos_x(), e.pos_y()), (500.0, 500.0));
    assert!(e.movement.as_ref().unwrap().pivot_recovery.is_none());
}

#[test]
fn committed_escape_cancels_after_lateral_collision_displacement() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    let e = entities.get_mut(id).unwrap();
    e.set_facing(0.0);
    e.movement.as_mut().unwrap().pivot_recovery = Some(crate::game::entity::PivotRecovery {
        goal: (508.0, 500.0),
        facing: 0.0,
        ticks_left: 30,
    });
    // Model the displacement applied by collision separation between movement ticks.
    e.set_position(500.0, 503.0);
    let occ = Occupancy::build(&map, &EntityStore::new());
    assert!(!super::super::pivot_recovery::advance(
        e,
        &map,
        &occ,
        2.0,
        &[]
    ));
    assert_eq!((e.pos_x(), e.pos_y()), (500.0, 503.0));
    assert!(e.movement.as_ref().unwrap().pivot_recovery.is_none());
}

#[test]
fn long_route_from_recorded_water_pose_uses_committed_escape_and_resumes() {
    let map = Map::load("Crossroads", 2, 2967932699).unwrap();
    let recorded: Entity =
        serde_json::from_str(include_str!("fixtures/replay_530_tank_355.json")).unwrap();
    let mut entities = EntityStore::new();
    let start = (recorded.pos_x(), recorded.pos_y());
    let id = entities.insert(recorded);
    let target = (start.0 - 300.0, start.1 - 120.0);
    let e = entities.get_mut(id).unwrap();
    e.replace_active_order(Order::move_to(target.0, target.1));
    e.set_path(vec![target]);
    e.set_path_goal(Some(target));
    e.mark_move_phase(MovePhase::Moving);
    let mut recovering = false;
    for tick in 0..300 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &mut [], &occ, &spatial, tick);
        let e = entities.get(id).unwrap();
        recovering |= e.movement.as_ref().unwrap().pivot_recovery.is_some();
        assert!(tank_standable_at_entity_facing(&map, &occ, &entities, id));
    }
    assert!(
        recovering,
        "ordinary movement must start the committed escape"
    );
    let e = entities.get(id).unwrap();
    assert!(moved_distance(start, (e.pos_x(), e.pos_y())) > 32.0);
}
