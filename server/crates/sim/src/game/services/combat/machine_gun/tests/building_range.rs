use super::*;

#[test]
fn replay_419_scout_cars_hit_barracks_at_acquired_edge_range() {
    // Replay 419 tick 8670: all three cars stopped in range of the nearest wall,
    // but their center-aimed 237.6 px rays expired just before reaching it.
    for (x, y) in [
        (1765.0858, 1763.9503),
        (1748.9705, 1827.6265),
        (1757.6121, 1794.1118),
    ] {
        let map = open_map(126);
        let mut entities = EntityStore::new();
        let attacker = entities.spawn_unit(1, EntityKind::ScoutCar, x, y).unwrap();
        let target = entities
            .spawn_building(2, EntityKind::Barracks, 2032.0, 1888.0, true)
            .unwrap();
        let range = 237.6;
        assert!(
            super::super::super::projection::combat_target_distance_sq(
                &map,
                (x, y),
                entities.get(target).unwrap()
            ) <= range * range
        );
        let fog = visible_fog(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
        fire(
            &map,
            &mut entities,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &SmokeCloudStore::new(),
            &mut SmallRng::seed_from_u64(1),
            &mut events,
            &mut vec![],
            attacker,
            target,
            rules::weapon_profile(rules::WeaponKind::ScoutCarMg).unwrap(),
            range,
            8670,
        );
        assert!(
            matches!(events[&1].first(), Some(Event::Attack { to, .. }) if *to == target),
            "aimed ray from ({x}, {y}) must reach the wall"
        );
        assert!(
            entities.get(target).unwrap().hp < 123,
            "a landed burst must damage the barracks"
        );
    }
}

#[test]
fn lone_machine_gun_ray_damages_armor_after_rounding() {
    for (kind, weapon) in [
        (
            EntityKind::MachineGunner,
            rules::WeaponKind::MachineGunnerMg,
        ),
        (EntityKind::ScoutCar, rules::WeaponKind::ScoutCarMg),
        (EntityKind::Tank, rules::WeaponKind::TankCoax),
    ] {
        // Fix both scattered rays at the cone edge so only the aimed ray hits armor.
        let map = open_map(32);
        let mut entities = EntityStore::new();
        let attacker = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Tank, 290.0, 100.0)
            .unwrap();
        let hp_before = entities.get(target).unwrap().hp;
        let fog = visible_fog(&map, &entities);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let mut events = HashMap::from([(1, vec![]), (2, vec![])]);
        fire(
            &map,
            &mut entities,
            &TeamRelations::from_player_teams([(1, 1), (2, 2)]),
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &SmokeCloudStore::new(),
            &mut rand::rngs::mock::StepRng::new(0, 0),
            &mut events,
            &mut vec![],
            attacker,
            target,
            rules::weapon_profile(weapon).unwrap(),
            210.0,
            1,
        );
        let hits = events[&1]
            .iter()
            .filter(|e| matches!(e, Event::Attack { to, .. } if *to == target))
            .count();
        assert_eq!(hits, 1, "fixture must isolate one {weapon:?} ray");
        assert_eq!(
            entities.get(target).unwrap().hp,
            hp_before - 1,
            "{weapon:?} must retain one damage after armor"
        );
    }
}
