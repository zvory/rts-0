use super::super::target_legality::DirectFireLegality;
use super::target_legality::direct_fire_legal;
use super::*;

#[test]
fn forest_blocks_spotted_units_and_buildings_for_automatic_and_ordered_fire() {
    for kind in [EntityKind::Worker, EntityKind::Barracks] {
        for ordered in [false, true] {
            let mut map = open_map(16);
            map.concealment_tiles = vec![(4, 4)];
            let mut entities = EntityStore::new();
            let start = map.tile_center(2, 4);
            let end = map.tile_center(6, 4);
            let attacker = entities
                .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
                .unwrap();
            let target = if kind == EntityKind::Barracks {
                entities.spawn_building(2, kind, end.0, end.1, true)
            } else {
                entities.spawn_unit(2, kind, end.0, end.1)
            }
            .unwrap();
            // A side spotter reveals the target without clearing the shooter's forest ray.
            let spotter = map.tile_center(6, 7);
            entities
                .spawn_unit(1, EntityKind::Worker, spotter.0, spotter.1)
                .unwrap();
            assert!(visible_fog(&map, &entities).is_visible_world(1, end.0, end.1));
            let smokes = SmokeCloudStore::new();
            for legality in [
                DirectFireLegality::AutoAcquire,
                DirectFireLegality::IntendedTarget,
            ] {
                assert!(!direct_fire_legal(
                    &map, &entities, &smokes, attacker, target, legality
                ));
            }
            if ordered {
                entities
                    .get_mut(attacker)
                    .unwrap()
                    .set_order(Order::attack(target));
            }
            let hp = entities.get(target).unwrap().hp;
            let events = run_combat_tick_on_map(
                &mut entities,
                &[player_state(1, false), player_state(2, false)],
                &map,
            );
            assert_eq!(entities.get(target).unwrap().hp, hp);
            assert!(!events
                .values()
                .flatten()
                .any(|event| matches!(event, Event::Attack { from, .. } if *from == attacker)));
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
    let attacker = entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .unwrap();
    let target = entities
        .spawn_unit(2, EntityKind::Worker, end.0, end.1)
        .unwrap();
    let smokes = SmokeCloudStore::new();
    let teams = default_team_relations();
    let spatial = SpatialIndex::build(&entities, map.width, map.height);
    let mut fog = visible_fog(&map, &entities);
    fog.refresh_concealment_detection_for_teams(10, &entities, &map, &smokes, &teams, &spatial);
    let los = LineOfSight::with_smoke(&map, &smokes);
    let blockers = ShotBlockerIndex::build(&map, &entities);
    for legality in [
        DirectFireLegality::AutoAcquire,
        DirectFireLegality::IntendedTarget,
    ] {
        assert!(super::super::target_legality::direct_fire_target_legal(
            &map, &entities, &blockers, &teams, &los, &fog, &smokes, attacker, 1, start, target,
            legality
        ));
    }
}

#[test]
fn rifleman_inside_forest_fires_outward_with_automatic_and_ordered_targets() {
    for order in ["idle", "hold", "attack_move", "attack"] {
        let mut map = open_map(16);
        map.concealment_tiles = vec![(2, 4), (3, 4)];
        let mut entities = EntityStore::new();
        let start = map.tile_center(2, 4);
        let end = map.tile_center(5, 4);
        let attacker = entities
            .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Worker, end.0, end.1)
            .unwrap();
        entities.get_mut(attacker).unwrap().set_order(match order {
            "hold" => Order::HoldPosition,
            "attack_move" => Order::attack_move_to(end.0, end.1),
            "attack" => Order::attack(target),
            _ => Order::Idle,
        });
        let events = run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
        assert!(events.values().flatten().any(
            |event| matches!(event, Event::Attack { from, to, .. } if *from == attacker && *to == target)
        ), "rifleman should fire out of forest with {order}");
    }
}

#[test]
fn ordered_tank_pursues_through_blocked_range_instead_of_waiting() {
    // Cover both current weapon range and the outer stationary-range wait band,
    // with and without an existing approach route. Exercise forest and smoke
    // independently so neither obstruction can mask a regression in the other.
    for target_tile in [6, 14] {
        for has_path in [false, true] {
            for (forest, smoke) in [(true, false), (false, true), (true, true)] {
                let mut map = open_map(24);
                if forest {
                    map.concealment_tiles = vec![(4, 4)];
                }
                let start = map.tile_center(2, 4);
                let end = map.tile_center(target_tile, 4);
                let mut entities = EntityStore::new();
                let tank = entities
                    .spawn_unit(1, EntityKind::Tank, start.0, start.1)
                    .unwrap();
                let target = entities
                    .spawn_unit(2, EntityKind::Tank, end.0, end.1)
                    .unwrap();
                let spotter = map.tile_center(target_tile, 7);
                entities
                    .spawn_unit(1, EntityKind::Worker, spotter.0, spotter.1)
                    .unwrap();
                let unit = entities.get_mut(tank).unwrap();
                unit.set_order(Order::attack(target));
                if has_path {
                    unit.set_path(vec![end]);
                    unit.set_path_goal(Some(end));
                }
                let mut smokes = SmokeCloudStore::new();
                if smoke {
                    smokes.spawn(start.0, start.1, 1.0, 100, 0).unwrap();
                }
                let hp = entities.get(target).unwrap().hp;
                let events = run_combat_tick_on_map_with_seed_and_smokes(
                    &mut entities,
                    &[player_state(1, false), player_state(2, false)],
                    &map,
                    0,
                    &smokes,
                );
                let unit = entities.get(tank).unwrap();
                assert!(!unit.path_is_empty(), "blocked tank must have an approach: target={target_tile}, path={has_path}, forest={forest}, smoke={smoke}");
                assert!(
                    matches!(unit.order(), Order::Attack(order) if order.execution.phase == AttackPhase::Pursuing)
                );
                assert_eq!(unit.order().attack_target(), Some(target));
                assert_eq!(entities.get(target).unwrap().hp, hp);
                assert!(!events
                    .values()
                    .flatten()
                    .any(|event| matches!(event, Event::Attack { from, .. } if *from == tank)));

                // Clearing the obstruction restores both normal firing and the outer
                // stationary-range wait, without leaving the pursuit path active.
                map.concealment_tiles.clear();
                let events = run_combat_tick_on_map(
                    &mut entities,
                    &[player_state(1, false), player_state(2, false)],
                    &map,
                );
                let unit = entities.get(tank).unwrap();
                assert!(unit.path_is_empty());
                assert_eq!(unit.path_goal(), None);
                let fired = events.values().flatten().any(
                    |event| matches!(event, Event::Attack { from, to, .. } if *from == tank && *to == target),
                );
                assert_eq!(
                    fired,
                    target_tile == 6,
                    "clear close targets should be fired on; distant targets must wait for the range ramp"
                );
            }
        }
    }
}
