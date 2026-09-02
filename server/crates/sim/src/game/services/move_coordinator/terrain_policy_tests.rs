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
fn every_production_source_takes_a_faster_offset_road() {
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
