use super::*;

#[test]
fn warrior_sword_two_shots_rifleman_without_secondary_damage() {
    let mut entities = EntityStore::new();
    let warrior = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .expect("Warrior should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 125.0, 100.0)
        .expect("Rifleman should spawn");
    entities
        .get_mut(warrior)
        .expect("Warrior should exist")
        .set_order(Order::attack(rifleman));
    entities
        .get_mut(rifleman)
        .expect("Rifleman should exist")
        .set_order(Order::HoldPosition);

    let mut all_events = run_combat_tick(&mut entities);
    assert_eq!(entities.get(rifleman).map(|unit| unit.hp), Some(22));

    for _ in 0..combat_rules::attack_profile(EntityKind::Warrior).cooldown {
        let tick_events = run_combat_tick(&mut entities);
        for (player, events) in tick_events {
            all_events.entry(player).or_default().extend(events);
        }
    }

    assert_eq!(entities.get(rifleman).map(|unit| unit.hp), Some(0));
    let owner_events = all_events
        .get(&1)
        .expect("Warrior owner should receive events");
    assert_eq!(
        owner_events
            .iter()
            .filter(|event| matches!(
                event,
                Event::Attack {
                    from,
                    to,
                    shot_origin: None,
                    weapon_kind: Some(weapon_kind),
                    ..
                } if *from == warrior && *to == rifleman && weapon_kind == "warrior_sword"
            ))
            .count(),
        2
    );
}

#[test]
fn warrior_sword_applies_fifty_percent_armor_penetration() {
    let mut entities = EntityStore::new();
    let warrior = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .expect("Warrior should spawn");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 125.0, 100.0)
        .expect("Tank should spawn");
    entities
        .get_mut(warrior)
        .expect("Warrior should exist")
        .set_order(Order::attack(tank));
    entities
        .get_mut(tank)
        .expect("Tank should exist")
        .set_order(Order::HoldPosition);

    run_combat_tick(&mut entities);

    assert_eq!(entities.get(tank).map(|unit| unit.hp), Some(278));
}

#[test]
fn disciple_insight_counts_actual_hp_loss_and_caps_at_sixty() {
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
        .unwrap();
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, 125.0, 100.0)
        .unwrap();
    entities.get_mut(target).unwrap().set_spawn_health(7);
    entities
        .get_mut(unit)
        .unwrap()
        .set_order(Order::attack(target));
    run_combat_tick(&mut entities);
    assert_eq!(
        entities.get(unit).unwrap().insight,
        7,
        "overkill does not grant insight"
    );
    entities.get_mut(unit).unwrap().insight = 59;
    entities
        .get_mut(unit)
        .unwrap()
        .set_weapon_cooldown(combat_rules::WeaponKind::WarriorSword, 0);
    entities.get_mut(target).unwrap().set_spawn_health(45);
    run_combat_tick(&mut entities);
    assert_eq!(entities.get(unit).unwrap().insight, 60);
}

#[test]
fn war_armor_uses_weapon_penetration_and_lightness_changes_attack_rate() {
    use crate::game::upgrade::UpgradeKind;
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 100.0, 100.0).unwrap();
    unit.dao = Some(UpgradeKind::DaoOfHeaviness);
    assert_eq!(unit.dao_damage_after_armor(100, 0.0), 50);
    assert_eq!(unit.dao_damage_after_armor(100, 0.5), 75);
    assert_eq!(unit.dao_damage_after_armor(100, 1.0), 100);
    let war = effective_attack_profile(&unit);
    assert_eq!(war.dmg, 35);
    unit.dao = Some(UpgradeKind::DaoOfLightness);
    let light = effective_attack_profile(&unit);
    assert_eq!(light.cooldown, 43);
    assert_eq!(
        unit.movement_speed(),
        crate::config::unit_stats(EntityKind::ScoutCar)
            .unwrap()
            .speed
    );
    unit.dao = Some(UpgradeKind::DaoOfMagic);
    assert!((unit.movement_speed() - 1.76).abs() < 0.001);
}

#[test]
fn disciple_attacks_vehicle_hulls_without_pushing_them() {
    for target_kind in [
        EntityKind::Tank,
        EntityKind::ScoutCar,
        EntityKind::Artillery,
    ] {
        for angle in [0.0_f32, std::f32::consts::FRAC_PI_2, 0.73] {
            for attack_move in [false, true] {
                let map = open_map(32);
                let mut entities = EntityStore::new();
                let target_pos = (400.0, 400.0);
                let target = entities
                    .spawn_unit(2, target_kind, target_pos.0, target_pos.1)
                    .unwrap();
                entities.get_mut(target).unwrap().set_facing(angle);
                // Isolate pursuit and collision from the defender's return fire.
                entities
                    .get_mut(target)
                    .unwrap()
                    .set_order(Order::HoldPosition);
                for weapon in combat_rules::WeaponKind::ALL {
                    entities
                        .get_mut(target)
                        .unwrap()
                        .set_weapon_cooldown(weapon, 10_000);
                }
                let disciple = entities
                    .spawn_unit(1, EntityKind::Warrior, 260.0, 400.0)
                    .unwrap();
                entities
                    .get_mut(disciple)
                    .unwrap()
                    .set_order(if attack_move {
                        Order::attack_move_to(540.0, 400.0)
                    } else {
                        Order::attack(target)
                    });
                if attack_move {
                    let attacker = entities.get_mut(disciple).unwrap();
                    attacker.set_path(vec![(540.0, 400.0)]);
                    attacker.set_path_goal(Some((540.0, 400.0)));
                }
                let hp_before = entities.get(target).unwrap().hp;
                let mut max_displacement = 0.0_f32;
                for tick in 0..180 {
                    run_movement_tick_on_map(&mut entities, &map, tick);
                    run_combat_tick_on_map(
                        &mut entities,
                        &[player_state(1, false), player_state(2, false)],
                        &map,
                    );
                    let occ = Occupancy::build(&map, &entities);
                    let spatial = SpatialIndex::build(&entities, map.width, map.height);
                    crate::game::services::movement::resolve_collisions(
                        &mut entities,
                        &spatial,
                        &map,
                        &occ,
                    );
                    let defender = entities.get(target).unwrap();
                    max_displacement = max_displacement.max(
                        (defender.pos_x() - target_pos.0)
                            .det_hypot(defender.pos_y() - target_pos.1),
                    );
                }
                assert!(entities.get(target).unwrap().hp < hp_before, "disciple must reach and damage {target_kind:?}, angle={angle}, attack_move={attack_move}");
                assert!(max_displacement < 1.0, "disciple pushed {target_kind:?} by {max_displacement}px, angle={angle}, attack_move={attack_move}");
            }
        }
    }
}

#[test]
fn disciple_vehicle_range_is_bounded_by_hull_surface() {
    let map = open_map(32);
    for attacker_kind in [
        EntityKind::Warrior,
        EntityKind::Worker,
        EntityKind::Golem,
        EntityKind::Rifleman,
    ] {
        for in_range in [true, false] {
            let mut entities = EntityStore::new();
            let target = entities
                .spawn_unit(2, EntityKind::Tank, 400.0, 400.0)
                .unwrap();
            let stats = config::unit_stats(attacker_kind).unwrap();
            let range = stats.range_tiles * config::TILE_SIZE as f32 + stats.radius + RANGE_SLACK;
            let extent = if attacker_kind == EntityKind::Rifleman {
                0.0
            } else {
                config::TANK_BODY_LENGTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX
            };
            let distance = range + extent + if in_range { -0.5 } else { 0.5 };
            let attacker = entities
                .spawn_unit(1, attacker_kind, 400.0 - distance, 400.0)
                .unwrap();
            entities
                .get_mut(attacker)
                .unwrap()
                .set_order(Order::attack(target));
            let hp_before = entities.get(target).unwrap().hp;
            run_combat_tick_on_map(
                &mut entities,
                &[player_state(1, false), player_state(2, false)],
                &map,
            );
            assert_eq!(
                entities.get(target).unwrap().hp < hp_before,
                in_range,
                "range boundary for {attacker_kind:?}"
            );
        }
    }
}
