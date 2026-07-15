use super::*;

#[test]
fn unfinished_attack_move_machine_gunner_resumes_without_idle_setup() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    if let Some(mg) = entities.get_mut(mg_id) {
        mg.set_order(Order::attack_move_to(300.0, 100.0));
        mg.set_path_goal(Some((300.0, 100.0)));
        mg.set_path(Vec::new());
        mg.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(16);
    run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);

    let mg = entities.get(mg_id).expect("mg should exist");
    assert_eq!(
        mg.weapon_setup(),
        WeaponSetup::Packed,
        "unfinished attack-move should resume movement instead of entering idle setup"
    );
    assert!(
        !mg.path_is_empty(),
        "attack-move should re-request its original destination"
    );
}

#[test]
fn attack_move_machine_gunner_tears_down_after_no_target_grace() {
    let mut entities = EntityStore::new();
    let mg_id = entities
        .spawn_unit(1, EntityKind::MachineGunner, 100.0, 100.0)
        .expect("machine gunner should spawn");
    if let Some(mg) = entities.get_mut(mg_id) {
        mg.set_weapon_setup(WeaponSetup::Deployed);
        mg.set_order(Order::attack_move_to(300.0, 100.0));
        mg.set_path_goal(Some((300.0, 100.0)));
        mg.set_path(Vec::new());
        mg.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(16);
    for _ in 0..config::TICK_HZ - 1 {
        run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    }
    assert_eq!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::Deployed,
        "machine gunner should not tear down before the one-second no-target grace expires"
    );

    run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    assert!(matches!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::TearingDown { .. }
    ));
    assert!(
        !entities
            .get(mg_id)
            .expect("mg should exist")
            .path_is_empty(),
        "attack-move destination should be re-requested while teardown is pending"
    );

    for _ in 0..config::MACHINE_GUNNER_SETUP_TICKS {
        run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    }
    assert_eq!(
        entities.get(mg_id).expect("mg should exist").weapon_setup(),
        WeaponSetup::Packed
    );
    let before_move = entities.get(mg_id).expect("mg should exist").pos_x;
    run_movement_tick_on_map(&mut entities, &map, 0);
    assert!(
        entities.get(mg_id).expect("mg should exist").pos_x > before_move,
        "packed machine gunner should continue the still-active attack-move"
    );
}

#[test]
fn attack_move_deployed_anti_tank_gun_tears_down_after_no_target_grace() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
        at.set_order(Order::attack_move_to(300.0, 100.0));
        at.set_path_goal(Some((300.0, 100.0)));
        at.set_path(Vec::new());
        at.mark_move_phase(MovePhase::Moving);
    }

    let map = open_map(16);
    for _ in 0..config::TICK_HZ - 1 {
        run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    }
    assert_eq!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .weapon_setup(),
        WeaponSetup::Deployed,
        "anti-tank gun should not tear down before the one-second no-target grace expires"
    );

    run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    let at = entities.get(at_id).expect("anti-tank gun should exist");
    assert!(matches!(at.weapon_setup(), WeaponSetup::TearingDown { .. }));
    assert_eq!(at.emplacement_facing(), None);
    assert_eq!(at.pending_redeploy_facing(), None);
    assert!(
        !at.path_is_empty(),
        "attack-move destination should be re-requested while teardown is pending"
    );

    for _ in 0..config::ANTI_TANK_GUN_SETUP_TICKS {
        run_combat_tick_on_map(&mut entities, &[player_state(1, false)], &map);
    }
    assert_eq!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .weapon_setup(),
        WeaponSetup::Packed
    );
    let before_move = entities
        .get(at_id)
        .expect("anti-tank gun should exist")
        .pos_x;
    run_movement_tick_on_map(&mut entities, &map, 0);
    assert!(
        entities
            .get(at_id)
            .expect("anti-tank gun should exist")
            .pos_x
            > before_move,
        "packed anti-tank gun should continue the still-active attack-move"
    );
}
