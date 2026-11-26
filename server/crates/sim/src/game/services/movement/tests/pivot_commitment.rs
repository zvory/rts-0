use super::*;
use crate::game::entity::Entity;

#[test]
fn replay_573_tank_324_follows_route_without_rescue_commands() {
    // Soupman vs Alex, recorded build dceecfaafe5c, tick 12470 / 23435.
    // The original tank remained at this exact position through tick 12659,
    // alternating across 90 degrees until the player replaced its order.
    let map = Map::load("Classic", 2, 1876340199).unwrap();
    let mut entities = EntityStore::new();
    let recorded: Entity =
        serde_json::from_str(include_str!("../fixtures/replay_573_tank_324.json")).unwrap();
    let start = (recorded.pos_x(), recorded.pos_y());
    let id = entities.insert(recorded);
    for tick in 12471..=12630 {
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
        assert!(
            tank_standable_at_entity_facing(&map, &occ, &entities, id),
            "illegal hull at tick {tick}"
        );
    }
    let e = entities.get(id).unwrap();
    assert!(
        e.pos_x() < start.0 - 32.0,
        "must follow westbound route without another command: start={start:?}, end={:?}",
        (e.pos_x(), e.pos_y())
    );
}

#[test]
fn replay_573_near_perpendicular_headings_make_legal_progress() {
    for degrees in [89.0_f32, 89.9, 90.0, 90.1, 91.0] {
        for tank_plus in [false, true] {
            let map = Map::load("Classic", 2, 1876340199).unwrap();
            let mut e: Entity =
                serde_json::from_str(include_str!("../fixtures/replay_573_tank_324.json")).unwrap();
            e.set_facing(degrees.to_radians());
            e.combat.as_mut().unwrap().tank_plus = tank_plus;
            let start = (e.pos_x(), e.pos_y());
            let mut entities = EntityStore::new();
            let id = entities.insert(e);
            for tick in 0..160 {
                let occ = Occupancy::build(&map, &entities);
                let spatial = SpatialIndex::build(&entities, map.width, map.height);
                movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
                assert!(
                    tank_standable_at_entity_facing(&map, &occ, &entities, id),
                    "{degrees} degrees, tank_plus={tank_plus}, tick={tick}"
                );
            }
            assert!(
                entities.get(id).unwrap().pos_x() < start.0 - 32.0,
                "{degrees} degrees, tank_plus={tank_plus} must follow route"
            );
        }
    }
}

#[test]
fn pivot_choice_survives_turns_and_entity_restore_in_both_directions() {
    let map = Map::load("Classic", 2, 1876340199).unwrap();
    for degrees in [89.0_f32, 92.0] {
        let mut entities = EntityStore::new();
        let mut e: Entity =
            serde_json::from_str(include_str!("../fixtures/replay_573_tank_324.json")).unwrap();
        e.set_facing(degrees.to_radians());
        let id = entities.insert(e);
        let occ = Occupancy::build(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        movement_system(&map, &mut entities, &[], &occ, &spatial, 0);
        let e = entities.get(id).unwrap();
        let choice = e.movement.as_ref().unwrap().pivot_drive.unwrap();
        assert_eq!(choice.reversing, degrees > 90.0);
        let restored: Entity = serde_json::from_str(&serde_json::to_string(e).unwrap()).unwrap();
        let mut restored_entities = EntityStore::new();
        restored_entities.insert(restored);
        for tick in 1..30 {
            for store in [&mut entities, &mut restored_entities] {
                let occ = Occupancy::build(&map, store);
                let spatial = SpatialIndex::build(store, map.width, map.height);
                movement_system(&map, store, &[], &occ, &spatial, tick);
            }
            assert_eq!(
                serde_json::to_value(entities.get(id)).unwrap(),
                serde_json::to_value(restored_entities.get(id)).unwrap()
            );
            let e = entities.get(id).unwrap();
            if e.next_waypoint() == Some(choice.waypoint) {
                assert_eq!(e.movement.as_ref().unwrap().pivot_drive, Some(choice));
            }
        }
    }
}

#[test]
fn replacing_route_or_order_clears_pivot_choice() {
    let mut e: Entity =
        serde_json::from_str(include_str!("../fixtures/replay_573_tank_324.json")).unwrap();
    let goal = e.path_goal().unwrap();
    let choice = crate::game::entity::PivotDriveCommitment {
        waypoint: e.next_waypoint().unwrap(),
        reversing: true,
    };
    let mutations: &[fn(&mut Entity)] = &[
        |e| e.set_path(vec![(1600.0, 2800.0)]),
        |e| e.clear_path(),
        |e| e.pop_waypoint(),
        |e| e.push_waypoint((1800.0, 2800.0)),
        |e| e.clear_orders(),
        |e| e.clear_active_order(),
        |e| e.hold_position(),
        |e| e.replace_active_order(Order::move_to(1600.0, 2800.0)),
    ];
    for mutate in mutations {
        e.set_path(vec![goal, choice.waypoint]);
        e.movement.as_mut().unwrap().pivot_drive = Some(choice);
        mutate(&mut e);
        assert!(e.movement.as_ref().unwrap().pivot_drive.is_none());
    }
}

#[test]
fn mirrored_perpendicular_route_entries_finish_for_pivot_vehicles() {
    for kind in [
        EntityKind::Tank,
        EntityKind::AntiTankGun,
        EntityKind::Artillery,
    ] {
        for sign in [-1.0, 1.0] {
            for degrees in [89.0_f32, 90.0, 91.0] {
                let map = Map {
                    width: 64,
                    height: 64,
                    terrain: vec![crate::protocol::terrain::GRASS; 64 * 64],
                    ..Default::default()
                };
                let start = map.tile_center(20, 20);
                let next = (start.0 + sign * 28.0, start.1 - 5.0);
                let goal = (start.0 + sign * 300.0, start.1 - 5.0);
                let mut entities = EntityStore::new();
                let id = entities.spawn_unit(1, kind, start.0, start.1).unwrap();
                entities
                    .get_mut(id)
                    .unwrap()
                    .set_facing(degrees.to_radians());
                set_path_direct(&mut entities, id, vec![next, goal]);
                for tick in 0..500 {
                    let occ = Occupancy::build(&map, &entities);
                    let spatial = SpatialIndex::build(&entities, map.width, map.height);
                    movement_system(&map, &mut entities, &[], &occ, &spatial, tick);
                    let e = entities.get(id).unwrap();
                    assert!(super::super::standability::unit_static_standable(
                        &occ,
                        &map,
                        kind,
                        e.pos_x(),
                        e.pos_y(),
                        e.facing()
                    ));
                }
                let e = entities.get(id).unwrap();
                assert!(
                    moved_distance((e.pos_x(), e.pos_y()), goal) < 2.0,
                    "{kind:?}, sign={sign}, heading={degrees}: failed to finish route at {:?}",
                    (e.pos_x(), e.pos_y())
                );
            }
        }
    }
}
