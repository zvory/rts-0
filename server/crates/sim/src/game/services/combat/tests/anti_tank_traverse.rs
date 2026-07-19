use super::*;

#[test]
fn anti_tank_gun_turns_slowly_before_firing() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 20.0)
        .expect("enemy rifleman should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_weapon_setup(WeaponSetup::Deployed);
    }

    for _ in 0..config::TICK_HZ {
        run_combat_tick(&mut entities);
    }

    let at = entities.get(at_id).expect("at should exist");
    let expected_turn = config::ANTI_TANK_GUN_DEPLOYED_TURN_RATE_DEGREES_PER_SECOND.to_radians();
    assert!(
        (at.facing().abs() - expected_turn).abs() <= 0.001,
        "anti-tank gun should slew by 5 degrees per second, got {:.4} radians after one second",
        at.facing().abs()
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "anti-tank gun should not fire until its barrel is aligned"
    );
}

#[test]
fn deployed_anti_tank_gun_fires_inside_its_cone_without_slewing() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let target_angle = 10.0_f32.to_radians();
    let target_id = entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            100.0 + 200.0 * target_angle.cos(),
            100.0 + 200.0 * target_angle.sin(),
        )
        .expect("enemy tank should spawn");
    let target_hp = entities.get(target_id).expect("enemy should exist").hp;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let at = entities.get(at_id).expect("at should exist");
    assert!(
        entities.get(target_id).expect("enemy should exist").hp < target_hp,
        "a target inside the deployed cone should be hit immediately"
    );
    assert!(
        at.facing().abs() <= 0.001
            && at.weapon_facing().unwrap_or_default().abs() <= 0.001
            && at.emplacement_facing().unwrap_or_default().abs() <= 0.001,
        "an in-cone target should not slew the anti-tank gun or its firing cone"
    );
}

#[test]
fn deployed_anti_tank_gun_drops_an_outside_retained_tank_for_an_inside_command_car() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let tank_angle = 30.0_f32.to_radians();
    let retained_tank = entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            100.0 + 200.0 * tank_angle.cos(),
            100.0 + 200.0 * tank_angle.sin(),
        )
        .expect("enemy tank should spawn");
    let scout_angle = 10.0_f32.to_radians();
    let command_car = entities
        .spawn_unit(
            2,
            EntityKind::CommandCar,
            100.0 + 180.0 * scout_angle.cos(),
            100.0 + 180.0 * scout_angle.sin(),
        )
        .expect("enemy command car should spawn");
    let command_car_hp = entities
        .get(command_car)
        .expect("command car should exist")
        .hp;
    entities
        .get_mut(retained_tank)
        .expect("enemy tank should exist")
        .set_attack_cd(u32::MAX);
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_target_id(Some(retained_tank));
    }

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    let at = entities.get(at_id).expect("anti-tank gun should exist");
    assert_eq!(
        at.target_id(),
        Some(command_car),
        "an in-cone command car should replace a retained tank outside the deployed cone"
    );
    assert!(
        entities
            .get(command_car)
            .expect("command car should exist")
            .hp
            < command_car_hp,
        "the in-cone command car should be fired on immediately"
    );
    assert!(
        at.facing().abs() <= 0.001,
        "switching to an in-cone target must not turn the anti-tank gun"
    );
}

#[test]
fn deployed_anti_tank_gun_traverses_until_an_outside_target_enters_its_cone() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let target_angle = 30.0_f32.to_radians();
    let target_distance = 200.0;
    let enemy_id = entities
        .spawn_unit(
            2,
            EntityKind::Tank,
            100.0 + target_distance * target_angle.cos(),
            100.0 + target_distance * target_angle.sin(),
        )
        .expect("enemy tank should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    entities
        .get_mut(enemy_id)
        .expect("enemy should exist")
        .set_attack_cd(u32::MAX);
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    let half_cone = config::ANTI_TANK_GUN_FIELD_OF_FIRE_RAD * 0.5;
    let turn_to_cone_entry = target_angle - half_cone;
    let ticks_to_cone_entry =
        (turn_to_cone_entry / config::ANTI_TANK_GUN_DEPLOYED_TURN_RATE_RAD_PER_TICK).ceil() as u32;
    for _ in 0..=ticks_to_cone_entry {
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
    }

    let at = entities.get(at_id).expect("at should exist");
    assert!(
        (at.facing() - turn_to_cone_entry).abs()
            <= ANTI_TANK_GUN_DEPLOYED_TURN_RATE_RAD_PER_TICK + 0.001,
        "anti-tank gun should traverse its cone until the target reaches the edge, got {:.4}",
        at.facing()
    );
    assert!(
        (at.emplacement_facing().unwrap_or_default() - turn_to_cone_entry).abs()
            <= ANTI_TANK_GUN_DEPLOYED_TURN_RATE_RAD_PER_TICK + 0.001,
        "the firing cone should traverse with the anti-tank gun"
    );
    assert!(
        entities.get(enemy_id).expect("enemy should exist").hp < enemy_hp,
        "the anti-tank gun should fire once its outside target enters the cone"
    );
}
