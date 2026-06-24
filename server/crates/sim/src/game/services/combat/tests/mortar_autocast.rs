use super::*;

#[test]
fn mortar_autocast_prefers_safe_target_over_nearer_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    let safe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
        .expect("safe enemy should spawn");
    let (impact_x, impact_y) = mortar_aim_point(&entities, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
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
fn mortar_autocast_explicit_attack_keeps_commanded_unsafe_target() {
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let unsafe_enemy = entities
        .spawn_unit(2, EntityKind::Rifleman, 220.0, 100.0)
        .expect("unsafe enemy should spawn");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 320.0, 100.0)
        .expect("safe enemy should spawn");
    let (impact_x, impact_y) = mortar_aim_point(&entities, unsafe_enemy, 10);
    entities
        .spawn_unit(1, EntityKind::Rifleman, impact_x, impact_y + 24.0)
        .expect("friendly should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_order(Order::attack(unsafe_enemy));
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
        mortar.set_weapon_setup(WeaponSetup::Deployed);
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
