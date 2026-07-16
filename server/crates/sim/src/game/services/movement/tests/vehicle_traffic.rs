use super::*;
use crate::game::services::movement::pivot_drive::vehicle_traffic_adjustment;

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
    let spatial = SpatialIndex::build(&entities, map.size);

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
    let spatial = SpatialIndex::build(&entities, map.size);

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
