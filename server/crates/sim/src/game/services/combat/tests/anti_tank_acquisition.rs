use super::*;

#[test]
fn deployed_anti_tank_gun_auto_acquisition_skips_out_of_arc_priority_target() {
    let map = open_map(16);
    let mut entities = EntityStore::new();
    let anti_tank_gun = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let out_of_arc_anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 300.0, 166.0)
        .expect("enemy anti-tank gun should spawn");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 310.0, 100.0)
        .expect("enemy tank should spawn");
    if let Some(at) = entities.get_mut(anti_tank_gun) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &map,
    );

    assert_eq!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(tank),
        "idle deployed AT should ignore higher-priority targets outside its fixed field"
    );
    assert!(
        events.values().flatten().any(|event| {
            matches!(
                event,
                Event::Attack { from, to, .. } if *from == anti_tank_gun && *to == tank
            )
        }),
        "in-arc fallback target should produce anti-tank attack feedback"
    );
    assert_ne!(
        entities
            .get(anti_tank_gun)
            .expect("anti-tank gun should exist")
            .target_id(),
        Some(out_of_arc_anti_tank_gun)
    );
}
