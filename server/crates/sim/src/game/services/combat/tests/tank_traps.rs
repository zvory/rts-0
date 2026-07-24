use super::*;

#[test]
fn completed_tank_traps_are_valid_only_as_explicit_attack_targets() {
    let map = open_map(8);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
        .expect("attacker should spawn");
    let trap = entities
        .spawn_building(1, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("tank trap should spawn");
    entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_order(Order::attack(trap));

    assert_eq!(entities.get(trap).map(|trap| trap.owner), Some(0));
    assert_eq!(
        resolve_test_target(
            &map,
            &entities,
            &team_relations(&[(1, 7), (2, 7)]),
            attacker,
            192.0,
        ),
        Some(trap),
        "a direct Attack order should retain its neutral Tank Trap target"
    );
}

#[test]
fn idle_units_do_not_auto_acquire_neutral_tank_traps() {
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let trap = entities
        .spawn_building(1, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("tank trap should spawn");
    let trap_hp = entities.get(trap).expect("trap should exist").hp;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &open_map(12),
    );

    assert_eq!(
        entities.get(tank).expect("tank should exist").target_id(),
        None
    );
    assert_eq!(entities.get(trap).expect("trap should exist").hp, trap_hp);
}

#[test]
fn move_orders_do_not_auto_acquire_neutral_tank_traps() {
    for kind in [EntityKind::ScoutCar, EntityKind::Tank] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::move_to(300.0, 100.0));
        let trap = entities
            .spawn_building(1, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("tank trap should spawn");
        let trap_hp = entities.get(trap).expect("trap should exist").hp;

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &open_map(12),
        );

        assert_eq!(
            entities
                .get(attacker)
                .expect("attacker should exist")
                .target_id(),
            None,
            "{kind:?} should preserve a plain Move order near a neutral Tank Trap"
        );
        assert_eq!(
            entities.get(trap).expect("trap should exist").hp,
            trap_hp,
            "{kind:?} should not damage a neutral Tank Trap during a plain Move order"
        );
    }
}

#[test]
fn ground_attack_move_does_not_auto_acquire_neutral_tank_traps() {
    for kind in [EntityKind::ScoutCar, EntityKind::Tank] {
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_order(Order::attack_move_to(300.0, 100.0));
        let trap = entities
            .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
            .expect("tank trap should spawn");
        let trap_hp = entities.get(trap).expect("trap should exist").hp;
        entities
            .get_mut(attacker)
            .expect("attacker should exist")
            .set_target_id(Some(trap));

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &open_map(12),
        );

        assert_eq!(
            entities
                .get(attacker)
                .expect("attacker should exist")
                .target_id(),
            None,
            "{kind:?} should drop even a stale Tank Trap target during ground Attack Move"
        );
        assert_eq!(
            entities.get(trap).expect("trap should exist").hp,
            trap_hp,
            "{kind:?} should not damage a neutral Tank Trap during ground Attack Move"
        );
    }
}

#[test]
fn ground_attack_move_ignores_tank_trap_while_acquiring_enemy_unit() {
    let mut entities = EntityStore::new();
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
        .expect("scout car should spawn");
    if let Some(scout) = entities.get_mut(scout) {
        scout.set_order(Order::attack_move_to(300.0, 100.0));
        scout.set_path_goal(Some((300.0, 100.0)));
    }
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("tank trap should spawn");
    let trap_hp = entities.get(trap).expect("trap should exist").hp;
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 180.0, 100.0)
        .expect("worker should spawn");

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &open_map(12),
    );

    let target = entities
        .get(scout)
        .expect("scout car should exist")
        .target_id();
    assert_eq!(
        target,
        Some(worker),
        "ground Attack Move should acquire the enemy unit and ignore the Tank Trap"
    );
    assert_ne!(target, Some(trap));
    assert_eq!(
        entities.get(trap).expect("trap should exist").hp,
        trap_hp,
        "route obstruction must not make the Tank Trap an automatic target"
    );
}

#[test]
fn tank_attack_move_ignores_tank_trap_while_acquiring_anti_tank_gun() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    if let Some(tank) = entities.get_mut(tank) {
        tank.set_order(Order::attack_move_to(300.0, 100.0));
        tank.set_path_goal(Some((300.0, 100.0)));
    }
    entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 160.0, true)
        .expect("irrelevant tank trap should spawn");
    let anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 180.0, 100.0)
        .expect("anti-tank gun should spawn");

    assert_eq!(
        resolve_tank_test_target(&map, &entities, &default_team_relations(), tank),
        Some(anti_tank_gun)
    );
}

#[test]
fn tank_destroys_tank_trap_on_second_shot() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("tank trap should spawn");
    let uncommanded_trap = entities
        .spawn_building(2, EntityKind::TankTrap, 130.0, 100.0, true)
        .expect("uncommanded tank trap should spawn");
    let uncommanded_trap_hp = entities
        .get(uncommanded_trap)
        .expect("uncommanded trap should exist")
        .hp;
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::attack(trap));

    let tank_shot = combat_rules::attack_profile(EntityKind::Tank).dmg;
    let coax_profile = combat_rules::weapon_profile(combat_rules::WeaponKind::TankCoax)
        .expect("Tank coax profile should exist");
    let coax_damage = combat_rules::effective_damage_for_weapon(
        coax_profile,
        EntityKind::TankTrap,
        coax_profile.dmg,
        Some(crate::rules::terrain::TerrainKind::Open),
    );
    let tank_cooldown = combat_rules::attack_profile(EntityKind::Tank).cooldown;

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities.get(trap).expect("trap should exist").hp,
        tank_shot - coax_damage,
        "first Tank shot should leave the trap alive after one cannon shot plus coax fallback damage"
    );
    assert_eq!(
        entities
            .get(uncommanded_trap)
            .expect("uncommanded trap should exist")
            .hp,
        uncommanded_trap_hp,
        "the coax must not broaden an explicit attack to another neutral Tank Trap"
    );

    for _ in 0..tank_cooldown {
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
    }

    assert_eq!(
        entities.get(trap).expect("trap should exist").hp,
        0,
        "second Tank shot should destroy the trap"
    );
}
