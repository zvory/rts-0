use super::*;

#[test]
fn deployed_anti_tank_gun_does_not_turn_or_fire_at_target_outside_arc() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let enemy_id = entities
        .spawn_unit(2, EntityKind::Tank, 100.0, 180.0)
        .expect("enemy tank should spawn");
    let enemy_hp = entities.get(enemy_id).expect("enemy should exist").hp;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    for _ in 0..20 {
        run_combat_tick(&mut entities);
    }

    let at = entities.get(at_id).expect("at should exist");
    assert!(
        at.facing().abs() <= 0.001,
        "anti-tank gun should not turn toward a target outside its fixed arc, got {:.4}",
        at.facing()
    );
    assert_eq!(
        entities.get(enemy_id).expect("enemy should exist").hp,
        enemy_hp,
        "anti-tank gun should not fire outside its deployed field of fire"
    );
}

#[test]
fn support_weapon_redeploy_rotates_after_teardown_completes() {
    for (kind, setup_ticks, label) in [
        (
            EntityKind::AntiTankGun,
            config::ANTI_TANK_GUN_SETUP_TICKS,
            "anti-tank gun",
        ),
        (
            EntityKind::Artillery,
            config::ARTILLERY_SETUP_TICKS,
            "artillery",
        ),
    ] {
        let mut entities = EntityStore::new();
        let id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("support weapon should spawn");
        let target = std::f32::consts::FRAC_PI_2;
        if let Some(unit) = entities.get_mut(id) {
            unit.set_emplacement_facing(Some(0.0));
            unit.set_pending_redeploy_facing(Some(target));
            unit.set_facing(0.0);
            unit.set_weapon_facing(0.0);
            unit.set_weapon_setup(WeaponSetup::TearingDownToRedeploy { ticks: setup_ticks });
        }

        run_combat_tick(&mut entities);

        let unit = entities.get(id).expect("support weapon should exist");
        assert_eq!(
            unit.facing(),
            0.0,
            "{label} should not rotate before teardown completes"
        );
        assert!(matches!(
            unit.weapon_setup(),
            WeaponSetup::TearingDownToRedeploy { .. }
        ));

        for _ in 1..setup_ticks {
            run_combat_tick(&mut entities);
        }

        let unit = entities.get(id).expect("support weapon should exist");
        assert_eq!(unit.weapon_setup(), WeaponSetup::Packed);
        assert!(
            unit.facing().abs() <= 0.001,
            "{label} should still face its original direction when teardown finishes, got {:.4}",
            unit.facing()
        );

        run_combat_tick(&mut entities);

        let unit = entities.get(id).expect("support weapon should exist");
        assert!(
            unit.facing() > 0.0 && unit.facing() <= ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
            "{label} should start rotating only after it is packed, got {:.4}",
            unit.facing()
        );

        for _ in 0..(setup_ticks as usize * 2) {
            run_combat_tick(&mut entities);
        }

        let unit = entities.get(id).expect("support weapon should exist");
        assert_eq!(unit.weapon_setup(), WeaponSetup::Deployed);
        assert!(
            (unit.facing() - target).abs() <= ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
            "{label} should finish redeploy facing the requested direction, got {:.4}",
            unit.facing()
        );
    }
}

#[test]
fn packed_anti_tank_gun_rotates_before_setup_animation_begins() {
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let target = std::f32::consts::FRAC_PI_2;
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Packed);
        at.set_emplacement_facing(Some(target));
        at.set_desired_weapon_facing(target);
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }

    run_combat_tick(&mut entities);

    let at = entities.get(at_id).expect("at should exist");
    assert_eq!(
        at.weapon_setup(),
        WeaponSetup::Packed,
        "anti-tank gun should stay packed until it has rotated into setup tolerance"
    );
    assert!(
        at.facing() > 0.0 && at.facing() <= ANTI_TANK_GUN_TURN_RATE_RAD_PER_TICK + 0.001,
        "anti-tank gun should begin rotating while still packed, got {:.4}",
        at.facing()
    );

    let mut saw_setting_up = false;
    for _ in 0..200 {
        run_combat_tick(&mut entities);
        let at = entities.get(at_id).expect("at should exist");
        if matches!(
            at.weapon_setup(),
            WeaponSetup::SettingUp { .. } | WeaponSetup::Deployed
        ) {
            saw_setting_up = true;
            assert!(
                angle_delta(at.facing(), target).abs() <= ANTI_TANK_GUN_FIRE_TOLERANCE_RAD + 0.001,
                "setup animation should begin only after the anti-tank gun is aligned, got {:.4}",
                at.facing()
            );
            break;
        }
    }
    assert!(
        saw_setting_up,
        "anti-tank gun should eventually start setup once it rotates into tolerance"
    );
}
