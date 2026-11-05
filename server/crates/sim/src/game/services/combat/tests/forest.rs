use super::*;
use super::target_legality::direct_fire_legal;
use super::super::target_legality::DirectFireLegality;

#[test]
fn forest_blocks_spotted_units_and_buildings_for_automatic_and_ordered_fire() {
    for kind in [EntityKind::Worker, EntityKind::Barracks] {
        for ordered in [false, true] {
            let mut map = open_map(16);
            map.concealment_tiles = vec![(4, 4)];
            let mut entities = EntityStore::new();
            let start = map.tile_center(2, 4);
            let end = map.tile_center(6, 4);
            let attacker = entities.spawn_unit(1, EntityKind::Rifleman, start.0, start.1).unwrap();
            let target = if kind == EntityKind::Barracks {
                entities.spawn_building(2, kind, end.0, end.1, true)
            } else {
                entities.spawn_unit(2, kind, end.0, end.1)
            }.unwrap();
            // A side spotter reveals the target without clearing the shooter's forest ray.
            let spotter = map.tile_center(6, 7);
            entities.spawn_unit(1, EntityKind::Worker, spotter.0, spotter.1).unwrap();
            assert!(visible_fog(&map, &entities).is_visible_world(1, end.0, end.1));
            let smokes = SmokeCloudStore::new();
            for legality in [DirectFireLegality::AutoAcquire, DirectFireLegality::IntendedTarget] {
                assert!(!direct_fire_legal(&map, &entities, &smokes, attacker, target, legality));
            }
            if ordered {
                entities.get_mut(attacker).unwrap().set_order(Order::attack(target));
            }
            let hp = entities.get(target).unwrap().hp;
            let events = run_combat_tick_on_map(&mut entities,
                &[player_state(1, false), player_state(2, false)], &map);
            assert_eq!(entities.get(target).unwrap().hp, hp);
            assert!(!events.values().flatten().any(|event|
                matches!(event, Event::Attack { from, .. } if *from == attacker)));
        }
    }
}

#[test]
fn forest_allows_fire_into_visible_cover() {
    let mut map = open_map(16);
    map.concealment_tiles = vec![(4, 4)];
    let mut entities = EntityStore::new();
    let start = map.tile_center(2, 4);
    let end = map.tile_center(4, 4);
    let attacker = entities.spawn_unit(1, EntityKind::Rifleman, start.0, start.1).unwrap();
    let target = entities.spawn_unit(2, EntityKind::Worker, end.0, end.1).unwrap();
    let smokes = SmokeCloudStore::new();
    let teams = default_team_relations();
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    let mut fog = visible_fog(&map, &entities);
    fog.refresh_concealment_detection_for_teams(10, &entities, &map, &smokes, &teams, &spatial);
    let los = LineOfSight::with_smoke(&map, &smokes);
    let blockers = ShotBlockerIndex::build(&map, &entities);
    for legality in [DirectFireLegality::AutoAcquire, DirectFireLegality::IntendedTarget] {
        assert!(super::super::target_legality::direct_fire_target_legal(
            &map, &entities, &blockers, &teams, &los, &fog, &smokes,
            attacker, 1, start, target, legality));
    }
}
