use super::*;
use crate::game::services::movement::pivot_drive::{
    pivot_drive_intent, vehicle_traffic_adjustment,
};

#[test]
fn aligned_vehicle_traffic_only_throttles_the_follower() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (x, y) = map.tile_center(20, 20);
    let follower = entities
        .spawn_unit(1, EntityKind::ScoutCar, x, y)
        .expect("follower spawn");
    let leader = entities
        .spawn_unit(1, EntityKind::CommandCar, x + 20.0, y)
        .expect("leader spawn");
    for (id, goal_x) in [(follower, x + 200.0), (leader, x + 220.0)] {
        if let Some(entity) = entities.get_mut(id) {
            entity.set_facing(0.0);
        }
        mark_moving(&mut entities, id, (goal_x, y));
    }
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    let follower_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        follower,
        EntityKind::ScoutCar,
        x,
        y,
        0.0,
    );
    let leader_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        leader,
        EntityKind::CommandCar,
        x + 20.0,
        y,
        0.0,
    );

    assert!(
        follower_adjustment.throttle_scale < 1.0,
        "the trailing vehicle should yield to its leader"
    );
    assert_eq!(
        leader_adjustment.throttle_scale, 1.0,
        "the leader must not brake for its follower"
    );
    assert_eq!(
        leader_adjustment.turn_bias, 0.0,
        "the leader must not steer away from its follower"
    );
}

#[test]
fn head_on_vehicle_traffic_keeps_existing_reciprocal_response() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (x, y) = map.tile_center(20, 20);
    let eastbound = entities
        .spawn_unit(1, EntityKind::ScoutCar, x, y)
        .expect("eastbound spawn");
    let westbound = entities
        .spawn_unit(2, EntityKind::CommandCar, x + 20.0, y)
        .expect("westbound spawn");
    if let Some(entity) = entities.get_mut(eastbound) {
        entity.set_facing(0.0);
    }
    mark_moving(&mut entities, eastbound, (x + 200.0, y));
    if let Some(entity) = entities.get_mut(westbound) {
        entity.set_facing(std::f32::consts::PI);
    }
    mark_moving(&mut entities, westbound, (x - 200.0, y));
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    let eastbound_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        eastbound,
        EntityKind::ScoutCar,
        x,
        y,
        0.0,
    );
    let westbound_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        westbound,
        EntityKind::CommandCar,
        x + 20.0,
        y,
        std::f32::consts::PI,
    );

    assert!(eastbound_adjustment.throttle_scale < 1.0);
    assert!(westbound_adjustment.throttle_scale < 1.0);
}

#[test]
fn first_reverse_tick_keeps_existing_reciprocal_traffic_response() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (x, y) = map.tile_center(20, 20);
    let heading_delta = std::f32::consts::PI / 12.0;
    let upper = entities
        .spawn_unit(1, EntityKind::ScoutCar, x, y)
        .expect("upper vehicle spawn");
    let lower = entities
        .spawn_unit(2, EntityKind::CommandCar, x, y - 20.0)
        .expect("lower vehicle spawn");
    for (id, pos_y, facing) in [(upper, y, -heading_delta), (lower, y - 20.0, heading_delta)] {
        if let Some(entity) = entities.get_mut(id) {
            entity.set_facing(facing);
        }
        mark_moving(
            &mut entities,
            id,
            (
                x - 2.0 * config::TILE_SIZE as f32 * facing.cos(),
                pos_y - 2.0 * config::TILE_SIZE as f32 * facing.sin(),
            ),
        );
    }
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    let upper_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        upper,
        EntityKind::ScoutCar,
        x,
        y,
        -heading_delta,
    );
    let lower_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        lower,
        EntityKind::CommandCar,
        x,
        y - 20.0,
        heading_delta,
    );

    assert!(upper_adjustment.throttle_scale < 1.0);
    assert!(lower_adjustment.throttle_scale < 1.0);
}

#[test]
fn reversing_tank_traffic_throttles_the_rearward_follower() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (x, y) = map.tile_center(20, 20);
    let leader = entities
        .spawn_unit(1, EntityKind::Tank, x, y)
        .expect("leader spawn");
    let follower = entities
        .spawn_unit(1, EntityKind::Tank, x + 20.0, y)
        .expect("follower spawn");
    for id in [leader, follower] {
        if let Some(entity) = entities.get_mut(id) {
            entity.set_facing(0.0);
        }
        mark_moving(&mut entities, id, (x - 200.0, y));
    }
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    let leader_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        leader,
        EntityKind::Tank,
        x,
        y,
        std::f32::consts::PI,
    );
    let follower_adjustment = vehicle_traffic_adjustment(
        &entities,
        &spatial,
        follower,
        EntityKind::Tank,
        x + 20.0,
        y,
        std::f32::consts::PI,
    );

    assert_eq!(leader_adjustment.throttle_scale, 1.0);
    assert!(
        follower_adjustment.throttle_scale < 1.0,
        "rearward traffic should make the reversing follower yield to the leader"
    );
}

fn under_fire_tank_with_far_retreat(
    map: &Map,
    entities: &mut EntityStore,
    hit_tick: u32,
) -> (u32, (f32, f32)) {
    let start = map.tile_center(20, 20);
    let goal = (
        start.0 - VEHICLE_REVERSE_GOAL_DISTANCE_PX - config::TILE_SIZE as f32 * 4.0,
        start.1,
    );
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    if let Some(entity) = entities.get_mut(tank) {
        entity.set_facing(0.0);
        entity.lock_tank_armor_reaction_source((start.0 + 200.0, start.1), hit_tick);
    }
    set_path_direct(entities, tank, vec![goal]);
    (tank, start)
}

#[test]
fn tank_under_fire_reverses_toward_a_far_goal_behind() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (tank, start) = under_fire_tank_with_far_retreat(&map, &mut entities, 10);
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    movement_system(&map, &mut entities, &mut [], &occ, &spatial, 11);

    let entity = entities.get(tank).expect("tank should exist");
    assert!(entity.pos_x < start.0, "tank should immediately reverse");
    assert!(
        angle_delta(0.0, entity.facing()).abs() <= 0.001,
        "tank should keep its front toward the damage source"
    );
}

#[test]
fn tank_armor_reaction_reverse_choice_has_an_inclusive_eighty_degree_pivot_cap() {
    let map = flat_map(1);
    let start = map.tile_center(20, 20);
    let goal = (start.0 + config::TILE_SIZE as f32 * 8.0, start.1);

    for (facing_degrees, should_reverse) in [(100.0_f32, true), (99.9_f32, false)] {
        let mut entities = EntityStore::new();
        let tank = entities
            .spawn_unit(1, EntityKind::Tank, start.0, start.1)
            .expect("tank should spawn");
        let entity = entities.get_mut(tank).expect("tank should exist");
        entity.set_facing(facing_degrees.to_radians());
        entity.lock_tank_armor_reaction_source((start.0 - 200.0, start.1), 10);
        set_path_direct(&mut entities, tank, vec![goal]);
        let occ = Occupancy::build(&map, &entities);
        let entity = entities.get(tank).expect("tank should exist");
        let intent = pivot_drive_intent(&map, &occ, entity, entity.pos_x, entity.pos_y)
            .expect("tank should have drive intent");
        let chose_reverse = angle_delta(intent.desired_facing, std::f32::consts::PI).abs() <= 0.001;

        assert_eq!(
            chose_reverse,
            should_reverse,
            "{facing_degrees}° current facing should {} the 80° reverse-choice cap",
            if should_reverse { "meet" } else { "exceed" }
        );
    }
}

#[test]
fn tank_armor_reaction_over_cap_forward_choice_preserves_normal_close_goal_reverse() {
    let map = flat_map(1);
    let start = map.tile_center(20, 20);
    let goal = (start.0 - config::TILE_SIZE as f32 * 2.0, start.1);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    let entity = entities.get_mut(tank).expect("tank should exist");
    entity.set_facing(0.0);
    entity.lock_tank_armor_reaction_source((start.0 - 200.0, start.1), 10);
    set_path_direct(&mut entities, tank, vec![goal]);
    let occ = Occupancy::build(&map, &entities);
    let entity = entities.get(tank).expect("tank should exist");
    let intent = pivot_drive_intent(&map, &occ, entity, entity.pos_x, entity.pos_y)
        .expect("tank should have drive intent");

    assert!(
        angle_delta(intent.desired_facing, 0.0).abs() <= 0.001,
        "an over-cap damage preference must not replace the ordinary close-goal reverse heading"
    );
}

#[test]
fn expired_under_fire_preference_restores_normal_far_goal_pivot() {
    let map = flat_map(1);
    let mut entities = EntityStore::new();
    let (tank, start) = under_fire_tank_with_far_retreat(&map, &mut entities, 10);
    let occ = Occupancy::build(&map, &entities);
    let spatial = SpatialIndex::build(&entities, map.width, map.height);

    movement_system(
        &map,
        &mut entities,
        &mut [],
        &occ,
        &spatial,
        10 + crate::rules::combat::TANK_ARMOR_REACTION_LOCK_TICKS,
    );

    let entity = entities.get(tank).expect("tank should exist");
    assert!(moved_distance(start, (entity.pos_x, entity.pos_y)) <= 0.01);
    assert!(entity.facing().abs() > 0.0, "tank should resume its pivot");
}
