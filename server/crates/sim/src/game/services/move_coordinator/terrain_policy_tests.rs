use super::*;
use crate::protocol::terrain;

const PRODUCTION_SOURCES: [PathingRequestSource; 8] = [
    PathingRequestSource::Move,
    PathingRequestSource::AttackMove,
    PathingRequestSource::DirectAttack,
    PathingRequestSource::Gather,
    PathingRequestSource::Build,
    PathingRequestSource::Deconstruct,
    PathingRequestSource::Ability,
    PathingRequestSource::Other,
];

fn flat_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![],
        ..Default::default()
    }
}

#[test]
fn every_production_pathing_source_uses_terrain_time() {
    for source in PRODUCTION_SOURCES {
        assert_eq!(
            route_policy_for_source(source),
            RoutePolicy::FastestTerrainTime,
            "production source {source:?} must not fall back to legacy scoring"
        );
    }
}

#[test]
fn generic_request_path_uses_a_faster_offset_road_for_every_source() {
    let mut map = flat_map(20);
    for tx in 2..=16 {
        let index = map.index(tx, 6);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    for source in PRODUCTION_SOURCES {
        let mut entities = EntityStore::new();
        let start = map.tile_center(2, 9);
        let goal = map.tile_center(16, 9);
        let unit = entities
            .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
            .expect("unit should spawn");
        let occ = Occupancy::build(&map, &entities);
        let mut pathing = PathingService::new(8_192, 256);
        pathing.advance_tick(1);
        let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

        assert!(coordinator.request_path(&mut entities, unit, goal, source));
        let routed = entities.get(unit).expect("unit should remain");
        assert_eq!(routed.path_policy(), RoutePolicy::FastestTerrainTime);
        assert!(
            routed.movement.as_ref().is_some_and(|movement| movement
                .path
                .iter()
                .any(|waypoint| { map.tile_of(waypoint.0, waypoint.1).1 == 6 })),
            "production source {source:?} should retain the faster road anchor"
        );
    }
}

fn offset_road_map() -> Map {
    let mut map = flat_map(24);
    for tx in 2..=20 {
        let index = map.index(tx, 6);
        map.terrain[index] = terrain::ROAD_HORIZONTAL;
    }
    map
}

fn assert_route_uses_offset_road(map: &Map, entity: &Entity) {
    assert!(
        entity.movement.as_ref().is_some_and(|movement| movement
            .path
            .iter()
            .any(|waypoint| map.tile_of(waypoint.0, waypoint.1).1 == 6)),
        "live order should retain the faster offset-road anchor: {:?}",
        entity.movement.as_ref().map(|movement| &movement.path)
    );
}

#[test]
fn direct_attack_live_caller_ranks_range_band_endpoints_by_route_cost() {
    let map = offset_road_map();
    let mut entities = EntityStore::new();
    let start = map.tile_center(2, 10);
    let target_pos = map.tile_center(19, 10);
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("attacker should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);
    coordinator.order_attack(&mut entities, attacker, target);

    assert!(coordinator.request_direct_attack_path(
        &mut entities,
        attacker,
        target,
        0.0,
        config::TILE_SIZE as f32 * 4.5,
    ));
    assert!(
        !coordinator.request_direct_attack_path(
            &mut entities,
            attacker,
            target,
            0.0,
            config::TILE_SIZE as f32 * 4.5,
        ),
        "selecting a non-geometric endpoint must not bypass the existing repath throttle"
    );

    let attacker = entities.get(attacker).expect("attacker should remain");
    assert_route_uses_offset_road(&map, attacker);
    let goal = attacker.path_goal().expect("attack should select a goal");
    assert!(
        goal.1 < target_pos.1,
        "terrain-time endpoint selection should approach from the road side, got {goal:?}"
    );
}

#[test]
fn build_live_caller_ranks_staging_endpoints_by_route_cost() {
    let map = offset_road_map();
    let mut entities = EntityStore::new();
    let start = map.tile_center(2, 10);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, start.0, start.1)
        .expect("worker should spawn");
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    assert!(coordinator.order_build(&mut entities, worker, EntityKind::Depot, 18, 9,));

    let worker = entities.get(worker).expect("worker should remain");
    assert_route_uses_offset_road(&map, worker);
    let goal = worker.path_goal().expect("build should select a goal");
    assert!(
        map.tile_of(goal.0, goal.1).1 < 9,
        "terrain-time endpoint selection should stage north of the footprint, got {goal:?}"
    );
}

#[test]
fn deconstruct_live_caller_ranks_staging_endpoints_by_route_cost() {
    let map = offset_road_map();
    let mut entities = EntityStore::new();
    let start = map.tile_center(2, 10);
    let target_pos = map.tile_center(18, 9);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, start.0, start.1)
        .expect("worker should spawn");
    let target = entities
        .spawn_building(1, EntityKind::TankTrap, target_pos.0, target_pos.1, true)
        .expect("tank trap should spawn");
    let occ = Occupancy::build(&map, &entities);
    let mut pathing = PathingService::new(8_192, 256);
    pathing.advance_tick(1);
    let mut coordinator = MoveCoordinator::new(&mut pathing, &map, &occ, 1);

    assert!(coordinator.order_deconstruct(&mut entities, worker, target));

    let worker = entities.get(worker).expect("worker should remain");
    assert_route_uses_offset_road(&map, worker);
    let goal = worker
        .path_goal()
        .expect("deconstruct should select a goal");
    assert!(
        map.tile_of(goal.0, goal.1).1 < 9,
        "terrain-time endpoint selection should stage north of the footprint, got {goal:?}"
    );
}
