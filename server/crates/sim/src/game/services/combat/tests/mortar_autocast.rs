use super::*;

#[test]
fn mortar_autocast_prefers_safe_target_over_nearer_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
        .expect("unsafe enemy should spawn");
    let safe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 360.0, 100.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        Some(safe_enemy),
        "autocast mortar should choose the best target with a clear predicted impact"
    );
    assert!(
        mortar.attack_cd() > 0,
        "autocast mortar should fire after switching to a safe target"
    );
}

#[test]
fn mortar_autocast_tracks_safe_target_while_reload_blocks_firing() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
        .expect("unsafe enemy should spawn");
    let safe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 260.0, 260.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_attack_cd(12);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar_entity = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar_entity.target_id(),
        Some(safe_enemy),
        "reloading autocast mortar should keep tracking the safe target"
    );
    let expected_turn = mortar::TURN_RATE_RAD_PER_TICK;
    assert!(
        angle_delta(mortar_entity.facing(), expected_turn).abs() <= 0.001,
        "mortar should turn toward the safe target while reloading, got {:.4}",
        mortar_entity.facing()
    );
    assert!(
        mortar_entity.attack_cd() > 0,
        "test setup should keep the mortar unable to fire this tick"
    );
}

#[test]
fn mortar_autocast_drops_unsafe_target_when_no_safe_target_exists() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
        .expect("unsafe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_target_id(Some(unsafe_enemy));
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        None,
        "autocast mortar should not keep an unsafe target when no safe target exists"
    );
    assert_eq!(
        mortar.attack_cd(),
        0,
        "autocast mortar should still hold fire when every candidate would splash same-team entities"
    );
}

#[test]
fn mortar_autocast_explicit_attack_keeps_commanded_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 280.0, 100.0)
        .expect("unsafe enemy should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 360.0, 100.0)
        .expect("safe enemy should spawn");
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let (impact_x, impact_y) =
        predicted_test_mortar_impact(&entities, &teams, &[1, 2], 1, mortar_id, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_order(Order::attack(unsafe_enemy));
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(
        mortar.target_id(),
        Some(unsafe_enemy),
        "explicit attack intent should keep the commanded target"
    );
    assert_eq!(
        mortar.attack_cd(),
        0,
        "explicit attack should still hold fire when the commanded target would splash friendlies"
    );
}

#[test]
fn packed_mortar_does_not_autocast() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 300.0, 100.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(mortar.target_id(), None);
    assert_eq!(
        mortar.attack_cd(),
        0,
        "packed mortar must hold autocast fire"
    );
}

#[test]
fn deployed_mortar_autocast_ignores_targets_inside_minimum_range() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(mortar.target_id(), None);
    assert_eq!(
        mortar.attack_cd(),
        0,
        "mortar must not fire inside five tiles"
    );
}

#[test]
fn deployed_mortar_autocast_ignores_targets_outside_maximum_range() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            100.0 + config::MORTAR_RANGE_TILES as f32 * config::TILE_SIZE as f32 + 1.0,
            100.0,
        )
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(mortar.target_id(), None);
    assert_eq!(
        mortar.attack_cd(),
        0,
        "mortar must not fire beyond fifteen tiles"
    );
}

#[test]
fn deployed_mortar_autocast_ignores_targets_outside_field_of_fire() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 100.0, 300.0)
        .expect("enemy should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_weapon_setup(WeaponSetup::Deployed);
        mortar.set_emplacement_facing(Some(0.0));
        mortar.set_autocast_enabled(AbilityKind::MortarFire, true);
    }

    run_combat_tick(&mut entities);

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert_eq!(mortar.target_id(), None);
    assert_eq!(
        mortar.attack_cd(),
        0,
        "mortar must hold fire outside its 120 degree cone"
    );
}
