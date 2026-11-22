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
    assert!((unit.movement_speed() - 1.84).abs() < 0.001);
    unit.dao = Some(UpgradeKind::DaoOfHeaviness);
    assert!((unit.movement_speed() - 1.84).abs() < 0.001);
    assert_eq!(unit.dao_damage_after_armor(100, 0.0), 50);
    assert_eq!(unit.dao_damage_after_armor(100, 0.5), 75);
    assert_eq!(unit.dao_damage_after_armor(100, 1.0), 100);
    let war = effective_attack_profile(&unit);
    assert_eq!(war.dmg, 35);
    unit.dao = Some(UpgradeKind::DaoOfLightness);
    let light = effective_attack_profile(&unit);
    assert_eq!(light.cooldown, 43);
    assert_eq!(light.range_tiles, 0.75);
    assert_eq!(
        light.weapon.unwrap().id,
        combat_rules::WeaponKind::WarriorSword
    );
    assert!(can_fire_while_moving(&unit, false));
    assert!(unit.dao_ability_available(crate::game::ability::AbilityKind::EkatTeleport));
    assert!((unit.movement_speed() - 2.7025).abs() < 0.001);
    unit.dao = Some(UpgradeKind::DaoOfMagic);
    assert!((unit.movement_speed() - 2.024).abs() < 0.001);
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

#[test]
fn dao_profiles_keep_lightness_melee_and_magic_ranged() {
    use crate::game::upgrade::UpgradeKind;
    for (dao, weapon, cooldown) in [
        (
            UpgradeKind::DaoOfLightness,
            combat_rules::WeaponKind::WarriorSword,
            43,
        ),
        (
            UpgradeKind::DaoOfMagic,
            combat_rules::WeaponKind::DiscipleOrb,
            64,
        ),
    ] {
        let mut entities = EntityStore::new();
        let disciple = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Tank, 130.0, 100.0)
            .unwrap();
        let unit = entities.get_mut(disciple).unwrap();
        unit.dao = Some(dao);
        unit.set_order(Order::attack(target));
        let profile = effective_attack_profile(unit);
        assert_eq!(
            profile.range_tiles,
            if dao == UpgradeKind::DaoOfLightness {
                0.75
            } else {
                5.0
            }
        );
        assert_eq!(
            combat_rules::engagement_range_tiles(unit.kind, profile.range_tiles),
            5.0
        );
        assert_eq!(profile.dmg, 23);
        assert_eq!(profile.cooldown, cooldown);
        assert_eq!(profile.weapon.unwrap().id, weapon);
        assert_eq!(profile.weapon.unwrap().armor_penetration, 0.5);
        let events = run_combat_tick(&mut entities);
        assert_eq!(entities.get(target).unwrap().hp, 278);
        assert_eq!(entities.get(disciple).unwrap().attack_cd(), cooldown);
        assert_eq!(
            entities
                .get(disciple)
                .unwrap()
                .weapon_cooldown(combat_rules::WeaponKind::WarriorSword),
            if dao == UpgradeKind::DaoOfLightness {
                cooldown
            } else {
                0
            }
        );
        assert_eq!(events.get(&1).unwrap().iter().filter(|event| matches!(event,
            Event::Attack {from, weapon_kind: Some(kind), ..} if *from == disciple && kind == weapon.stable_id()
        )).count(), 1);
        // The resolved primary cooldown is ticked by the compatibility helper as well.
        run_combat_tick(&mut entities);
        assert_eq!(entities.get(target).unwrap().hp, 278);
        assert_eq!(entities.get(disciple).unwrap().attack_cd(), cooldown - 1);
    }
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 100.0, 100.0).unwrap();
    for dao in [None, Some(UpgradeKind::DaoOfHeaviness)] {
        unit.dao = dao;
        let profile = effective_attack_profile(&unit);
        assert_eq!(
            profile.weapon.unwrap().id,
            combat_rules::WeaponKind::WarriorSword
        );
        assert_eq!(profile.range_tiles, 0.75);
        assert_eq!(profile.dmg, if dao.is_some() { 35 } else { 23 });
        assert_eq!(profile.cooldown, 64);
        assert!(!can_fire_while_moving(&unit, false));
    }
}

#[test]
fn lightness_fires_during_move_and_magic_keeps_normal_stop_to_fire_policy() {
    use crate::game::upgrade::UpgradeKind;
    for dao in [UpgradeKind::DaoOfLightness, UpgradeKind::DaoOfMagic] {
        let mut entities = EntityStore::new();
        let disciple = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .unwrap();
        let goal = (400.0, 100.0);
        let unit = entities.get_mut(disciple).unwrap();
        unit.dao = Some(dao);
        unit.set_order(Order::move_to(goal.0, goal.1));
        unit.set_path(vec![goal]);
        unit.set_path_goal(Some(goal));
        unit.mark_move_phase(MovePhase::Moving);
        unit.set_movement_delta(2.0, 0.0);
        run_combat_tick(&mut entities);
        assert_eq!(
            entities.get(target).unwrap().hp,
            if dao == UpgradeKind::DaoOfLightness {
                22
            } else {
                45
            }
        );
        let unit = entities.get_mut(disciple).unwrap();
        assert!(!unit.path_is_empty());
        assert_eq!(unit.path_goal(), Some(goal));
        if dao == UpgradeKind::DaoOfMagic {
            unit.set_order(Order::attack_move_to(goal.0, goal.1));
            run_combat_tick(&mut entities);
            assert_eq!(entities.get(target).unwrap().hp, 22);
            assert!(entities.get(disciple).unwrap().path_is_empty());
        }
    }
}

#[test]
fn magic_vehicle_range_uses_center_and_never_acquires_beyond_attack_range() {
    use crate::game::upgrade::UpgradeKind;
    let map = open_map(32);
    for dao in [UpgradeKind::DaoOfMagic] {
        for inside in [true, false] {
            let mut entities = EntityStore::new();
            let disciple = entities
                .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
                .unwrap();
            let range = 5.0 * config::TILE_SIZE as f32
                + entities.get(disciple).unwrap().radius()
                + RANGE_SLACK;
            let target = entities
                .spawn_unit(
                    2,
                    EntityKind::Tank,
                    400.0 + range + if inside { -0.5 } else { 0.5 },
                    400.0,
                )
                .unwrap();
            entities.get_mut(disciple).unwrap().dao = Some(dao);
            let hp = entities.get(target).unwrap().hp;
            run_combat_tick_on_map(
                &mut entities,
                &[player_state(1, false), player_state(2, false)],
                &map,
            );
            assert_eq!(entities.get(target).unwrap().hp < hp, inside);
            let unit = entities.get(disciple).unwrap();
            if !inside {
                assert_eq!(unit.target_id(), None);
                assert!(unit.path_is_empty());
                assert_eq!(unit.path_goal(), None);
            }
        }
    }
}

#[test]
fn dao_evolution_preserves_reload_and_uses_matching_pursuit_geometry() {
    use crate::game::ability::AbilityKind;
    let map = open_map(32);
    for (ability, weapon) in [
        (
            AbilityKind::EvolveLightness,
            combat_rules::WeaponKind::WarriorSword,
        ),
        (
            AbilityKind::EvolveMagic,
            combat_rules::WeaponKind::DiscipleOrb,
        ),
    ] {
        let mut unit =
            crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 100.0, 100.0).unwrap();
        unit.insight = 60;
        unit.set_attack_cd(19);
        assert!(unit.evolve_dao(ability));
        assert_eq!(unit.attack_cd(), 19);
        assert_eq!(unit.weapon_cooldown(weapon), 19);
        let tank =
            crate::game::entity::Entity::new_unit(2, EntityKind::Tank, 400.0, 100.0).unwrap();
        let point = crate::game::services::geometry::primary_combat_target_point(
            &map,
            unit.default_weapon_kind().unwrap(),
            (100.0, 100.0),
            &tank,
        );
        assert_eq!(point.0 < 400.0, ability == AbilityKind::EvolveLightness);
        assert_eq!(point.1, 100.0);
        assert!(
            crate::game::services::geometry::primary_combat_target_point(
                &map,
                combat_rules::WeaponKind::WarriorSword,
                (100.0, 100.0),
                &tank
            )
            .0 < 400.0
        );
    }
}

#[test]
fn ranged_evolution_cancels_lost_idle_approach_but_preserves_explicit_move() {
    use crate::game::ability::AbilityKind;
    let map = open_map(32);
    for ability in [AbilityKind::EvolveLightness, AbilityKind::EvolveMagic] {
        for explicit_move in [false, true] {
            let mut entities = EntityStore::new();
            let disciple = entities
                .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
                .unwrap();
            let target = entities
                .spawn_unit(2, EntityKind::Rifleman, 520.0, 400.0)
                .unwrap();
            let players = [player_state(1, false), player_state(2, false)];
            run_combat_tick_on_map(&mut entities, &players, &map);
            let unit = entities.get_mut(disciple).unwrap();
            assert!(
                !unit.path_is_empty(),
                "unevolved idle disciple should approach a target beyond sword reach"
            );
            let goal = (600.0, 400.0);
            if explicit_move {
                unit.set_order(Order::move_to(goal.0, goal.1));
                unit.set_path(vec![goal]);
                unit.set_path_goal(Some(goal));
                unit.mark_move_phase(MovePhase::Moving);
            }
            unit.insight = 60;
            assert!(unit.evolve_dao(ability));
            entities.get_mut(target).unwrap().set_spawn_health(0);
            run_combat_tick_on_map(&mut entities, &players, &map);
            let unit = entities.get(disciple).unwrap();
            assert_eq!(unit.path_is_empty(), !explicit_move);
            assert_eq!(unit.path_goal(), explicit_move.then_some(goal));
            assert_eq!(unit.target_id(), None);
        }
    }
}

#[test]
fn lightness_moving_sword_cannot_hit_at_former_arrow_range() {
    use crate::game::upgrade::UpgradeKind;
    for distance in [20.0, 120.0] {
        let mut entities = EntityStore::new();
        let disciple = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0 + distance, 100.0)
            .unwrap();
        let unit = entities.get_mut(disciple).unwrap();
        unit.dao = Some(UpgradeKind::DaoOfLightness);
        unit.set_order(Order::move_to(400.0, 100.0));
        unit.set_path(vec![(400.0, 100.0)]);
        unit.set_path_goal(Some((400.0, 100.0)));
        unit.mark_move_phase(MovePhase::Moving);
        unit.set_movement_delta(2.0, 0.0);
        let events = run_combat_tick(&mut entities);
        assert_eq!(
            entities.get(target).unwrap().hp,
            if distance < 30.0 { 22 } else { 45 }
        );
        assert_eq!(
            entities.get(disciple).unwrap().path_goal(),
            Some((400.0, 100.0))
        );
        assert!(events.values().flatten().all(|event| !matches!(event,
            Event::Attack { from, weapon_kind: Some(kind), .. }
            if *from == disciple && kind == combat_rules::WeaponKind::DiscipleArrow.stable_id()
        )));
    }
}
