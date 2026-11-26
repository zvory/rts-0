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
fn disciple_insight_counts_actual_hp_loss_resets_on_evolution_and_raises_cap() {
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
        7, // One insight per actual HP lost, regardless of target cost.
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
    assert!(entities
        .get_mut(unit)
        .unwrap()
        .evolve_dao(crate::game::ability::AbilityKind::EvolveWar));
    assert_eq!(entities.get(unit).unwrap().insight, 0);
    for expected in [35, 70, 105, 140, 175, 200, 200] {
        entities
            .get_mut(unit)
            .unwrap()
            .set_weapon_cooldown(combat_rules::WeaponKind::WarriorSword, 0);
        entities.get_mut(target).unwrap().set_spawn_health(45);
        run_combat_tick(&mut entities);
        let evolved = entities.get(unit).unwrap();
        assert_eq!(evolved.insight, expected);
        assert_eq!(
            evolved.dao(),
            Some(crate::game::upgrade::UpgradeKind::DaoOfHeaviness)
        );
        assert!(!evolved.dao_ability_available(crate::game::ability::AbilityKind::EvolveWar));
    }
}

#[test]
fn war_armor_uses_weapon_penetration_and_wind_changes_attack_rate() {
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 100.0, 100.0).unwrap();
    assert!((unit.movement_speed() - 2.0).abs() < 0.001);
    unit.set_disciple_kind_for_test(EntityKind::WarAdept);
    assert!((unit.movement_speed() - 2.1).abs() < 0.001);
    assert_eq!(unit.dao_damage_after_armor(100, 0.0), 50);
    assert_eq!(unit.dao_damage_after_armor(100, 0.5), 75);
    assert_eq!(unit.dao_damage_after_armor(100, 1.0), 100);
    let war = effective_attack_profile(&unit);
    assert_eq!(war.dmg, 35);
    assert_eq!(war.range_tiles, config::DAO_WAR_RANGE_TILES);
    unit.set_disciple_kind_for_test(EntityKind::WindAdept);
    let light = effective_attack_profile(&unit);
    assert_eq!(light.cooldown, 61);
    assert_eq!(light.range_tiles, 0.75);
    assert_eq!(
        light.weapon.unwrap().id,
        combat_rules::WeaponKind::WarriorSword
    );
    assert!(can_fire_while_moving(&unit, false));
    assert!(unit.dao_ability_available(crate::game::ability::AbilityKind::WindDash));
    assert!((unit.movement_speed() - 2.7025).abs() < 0.001);
    unit.set_disciple_kind_for_test(EntityKind::MagicAdept);
    assert!((unit.movement_speed() - 2.1).abs() < 0.001);
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
fn dao_profiles_keep_wind_melee_and_magic_ranged() {
    use crate::game::upgrade::UpgradeKind;
    for (dao, weapon, cooldown) in [
        (
            UpgradeKind::DaoWind,
            combat_rules::WeaponKind::WarriorSword,
            61,
        ),
        (
            UpgradeKind::DaoOfMagic,
            combat_rules::WeaponKind::DiscipleOrb,
            91,
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
        unit.set_disciple_kind_for_test(EntityKind::evolved_disciple(dao).unwrap());
        unit.set_order(Order::attack(target));
        let profile = effective_attack_profile(unit);
        assert_eq!(
            profile.range_tiles,
            if dao == UpgradeKind::DaoWind {
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
        assert_eq!(profile.weapon.unwrap().armor_penetration, 0.25);
        let events = run_combat_through_orb_impacts(&mut entities);
        assert_eq!(entities.get(target).unwrap().hp, 282);
        assert_eq!(entities.get(disciple).unwrap().attack_cd(), cooldown);
        assert_eq!(
            entities
                .get(disciple)
                .unwrap()
                .weapon_cooldown(combat_rules::WeaponKind::WarriorSword),
            if dao == UpgradeKind::DaoWind {
                cooldown
            } else {
                0
            }
        );
        assert_eq!(events.get(&1).unwrap().iter().filter(|event| matches!(event,
            Event::Attack {from, weapon_kind: Some(kind), ..} if *from == disciple && kind == weapon.stable_id()
        )).count(), 1);
        // The resolved primary cooldown is ticked by the compatibility helper as well.
        run_combat_through_orb_impacts(&mut entities);
        assert_eq!(entities.get(target).unwrap().hp, 282);
        assert_eq!(entities.get(disciple).unwrap().attack_cd(), cooldown - 1);
    }
    let mut unit =
        crate::game::entity::Entity::new_unit(1, EntityKind::Warrior, 100.0, 100.0).unwrap();
    for dao in [None, Some(UpgradeKind::DaoOfHeaviness)] {
        unit.set_disciple_kind_for_test(
            dao.and_then(EntityKind::evolved_disciple)
                .unwrap_or(EntityKind::Warrior),
        );
        let profile = effective_attack_profile(&unit);
        assert_eq!(
            profile.weapon.unwrap().id,
            combat_rules::WeaponKind::WarriorSword
        );
        assert_eq!(
            profile.range_tiles,
            if dao.is_some() {
                config::DAO_WAR_RANGE_TILES
            } else {
                0.75
            }
        );
        assert_eq!(profile.dmg, if dao.is_some() { 35 } else { 23 });
        assert_eq!(profile.cooldown, 91);
        assert!(!can_fire_while_moving(&unit, false));
    }
}

#[test]
fn wind_fires_during_move_and_magic_keeps_normal_stop_to_fire_policy() {
    use crate::game::upgrade::UpgradeKind;
    for dao in [UpgradeKind::DaoWind, UpgradeKind::DaoOfMagic] {
        let mut entities = EntityStore::new();
        let disciple = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0)
            .unwrap();
        let goal = (400.0, 100.0);
        let unit = entities.get_mut(disciple).unwrap();
        unit.set_disciple_kind_for_test(EntityKind::evolved_disciple(dao).unwrap());
        unit.set_order(Order::move_to(goal.0, goal.1));
        unit.set_path(vec![goal]);
        unit.set_path_goal(Some(goal));
        unit.mark_move_phase(MovePhase::Moving);
        unit.set_movement_delta(2.0, 0.0);
        run_combat_through_orb_impacts(&mut entities);
        assert_eq!(
            entities.get(target).unwrap().hp,
            if dao == UpgradeKind::DaoWind { 22 } else { 45 }
        );
        let unit = entities.get_mut(disciple).unwrap();
        assert!(!unit.path_is_empty());
        assert_eq!(unit.path_goal(), Some(goal));
        if dao == UpgradeKind::DaoOfMagic {
            unit.set_order(Order::attack_move_to(goal.0, goal.1));
            run_combat_through_orb_impacts(&mut entities);
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
            entities
                .get_mut(disciple)
                .unwrap()
                .set_disciple_kind_for_test(EntityKind::evolved_disciple(dao).unwrap());
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
            let hp = entities.get(target).unwrap().hp;
            run_combat_tick_and_impacts(
                &mut entities,
                &[player_state(1, false), player_state(2, false)],
                &map,
                0,
                &SmokeCloudStore::new(),
                10,
                true,
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
            AbilityKind::EvolveWind,
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
        assert_eq!(point.0 < 400.0, ability == AbilityKind::EvolveWind);
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
    for ability in [AbilityKind::EvolveWind, AbilityKind::EvolveMagic] {
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
fn wind_moving_sword_cannot_hit_at_former_arrow_range() {
    for distance in [20.0, 120.0] {
        let mut entities = EntityStore::new();
        let disciple = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let target = entities
            .spawn_unit(2, EntityKind::Rifleman, 100.0 + distance, 100.0)
            .unwrap();
        let unit = entities.get_mut(disciple).unwrap();
        unit.set_disciple_kind_for_test(EntityKind::WindAdept);
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

#[test]
fn disciple_building_insight_quarters_actual_damage_and_carries_fraction() {
    for dao in [
        None,
        Some(crate::game::upgrade::UpgradeKind::DaoOfHeaviness),
    ] {
        for hp in [1, 7, 180] {
            let mut entities = EntityStore::new();
            let unit = entities
                .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
                .unwrap();
            let target = entities
                .spawn_building(2, EntityKind::ResourceDepot, 125.0, 100.0, true)
                .unwrap();
            entities.get_mut(target).unwrap().set_spawn_health(hp);
            let attacker = entities.get_mut(unit).unwrap();
            attacker.set_disciple_kind_for_test(
                dao.and_then(EntityKind::evolved_disciple)
                    .unwrap_or(EntityKind::Warrior),
            );
            attacker.set_order(Order::attack(target));
            run_combat_tick(&mut entities);
            let actual = hp - entities.get(target).unwrap().hp;
            assert!(actual > 0);
            assert_eq!(entities.get(unit).unwrap().insight, actual / 4);
        }
    }
}

#[test]
fn war_smaller_body_controls_attack_boundary() {
    for inside in [true, false] {
        let mut entities = EntityStore::new();
        let war = entities
            .spawn_unit(1, EntityKind::Warrior, 100.0, 100.0)
            .unwrap();
        let unit = entities.get_mut(war).unwrap();
        unit.set_disciple_kind_for_test(EntityKind::WarAdept);
        let reach =
            config::DAO_WAR_RANGE_TILES * config::TILE_SIZE as f32 + unit.radius() + RANGE_SLACK;
        let target = entities
            .spawn_unit(
                2,
                EntityKind::Rifleman,
                100.0 + reach + if inside { -0.5 } else { 0.5 },
                100.0,
            )
            .unwrap();
        entities
            .get_mut(war)
            .unwrap()
            .set_order(Order::attack(target));
        let hp = entities.get(target).unwrap().hp;
        run_combat_tick(&mut entities);
        assert_eq!(entities.get(target).unwrap().hp < hp, inside);
    }
}

#[test]
fn wind_strikes_without_canceling_its_melee_approach() {
    for order_name in ["Attack", "AttackMove", "Idle"] {
        for kind in [
            EntityKind::WindAdept,
            EntityKind::Warrior,
            EntityKind::MagicAdept,
        ] {
            let mut entities = EntityStore::new();
            let attacker = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
            let target = entities
                .spawn_unit(2, EntityKind::ScoutCar, 165.0, 100.0)
                .unwrap();
            let unit = entities.get_mut(attacker).unwrap();
            unit.set_order(match order_name {
                "Attack" => Order::attack(target),
                "AttackMove" => Order::attack_move_to(400.0, 100.0),
                _ => Order::Idle,
            });
            // An existing legal approach ends inside melee range of the vehicle hull.
            unit.set_path(vec![(110.0, 100.0)]);
            unit.set_path_goal(Some((110.0, 100.0)));
            let hp_before = entities.get(target).unwrap().hp;
            run_combat_through_orb_impacts(&mut entities);
            assert!(entities.get(target).unwrap().hp < hp_before);
            let unit = entities.get(attacker).unwrap();
            assert_eq!(
                unit.path_is_empty(),
                kind != EntityKind::WindAdept,
                "{kind:?}, order={order_name}"
            );
            if kind == EntityKind::WindAdept {
                assert_eq!(unit.path_goal(), Some((110.0, 100.0)));
                run_movement_tick_on_map(&mut entities, &open_map(32), 1);
                assert!(entities.get(attacker).unwrap().pos_x() > 100.0);
            }
        }
    }
}

#[test]
fn wind_attack_move_stops_its_travel_route_to_engage() {
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::WindAdept, 100.0, 100.0)
        .unwrap();
    let target = entities
        .spawn_unit(2, EntityKind::Rifleman, 125.0, 100.0)
        .unwrap();
    let destination = (400.0, 100.0);
    let unit = entities.get_mut(attacker).unwrap();
    unit.set_order(Order::attack_move_to(destination.0, destination.1));
    unit.set_path(vec![destination]);
    unit.set_path_goal(Some(destination));

    let hp_before = entities.get(target).unwrap().hp;
    run_combat_tick(&mut entities);
    assert!(entities.get(target).unwrap().hp < hp_before);
    let unit = entities.get(attacker).unwrap();
    assert!(
        unit.path_is_empty(),
        "the travel route is not a melee approach"
    );
    assert_eq!(unit.move_intent(), Some(destination));
}

#[test]
fn war_adept_tank_insight_doubles_actual_damage_only() {
    for kind in [
        EntityKind::Warrior,
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        for target_kind in [EntityKind::Tank, EntityKind::ScoutCar, EntityKind::Rifleman] {
            for hp in [1, 180] {
                let mut entities = EntityStore::new();
                let attacker = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
                let target = entities.spawn_unit(2, target_kind, 125.0, 100.0).unwrap();
                entities.get_mut(target).unwrap().set_spawn_health(hp);
                let unit = entities.get_mut(attacker).unwrap();
                // A prior building hit's fractional credit must survive the tank bonus.
                unit.gain_insight(1, EntityKind::ResourceDepot);
                unit.set_order(Order::attack(target));
                run_combat_through_orb_impacts(&mut entities);
                let damage = hp - entities.get(target).unwrap().hp;
                assert!(damage > 0);
                let multiplier = if kind == EntityKind::WarAdept && target_kind == EntityKind::Tank
                {
                    2
                } else {
                    1
                };
                let unit = entities.get(attacker).unwrap();
                assert_eq!(
                    unit.insight,
                    damage * multiplier,
                    "{kind:?} vs {target_kind:?}, hp={hp}"
                );
                assert_eq!(unit.insight_fraction, 1);
            }
        }
    }
}
