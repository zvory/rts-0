use super::*;

#[test]
fn default_weapon_cooldown_cadence_matches_profiles() {
    let cases = [
        (EntityKind::Worker, 140.0),
        (EntityKind::Golem, 140.0),
        (EntityKind::Rifleman, 180.0),
        (EntityKind::MachineGunner, 220.0),
        (EntityKind::ScoutCar, 180.0),
        (EntityKind::AntiTankGun, 220.0),
        (EntityKind::MortarTeam, 280.0),
        (EntityKind::Tank, 180.0),
    ];

    for (kind, target_x) in cases {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let target_id = entities
            .spawn_building(2, EntityKind::Depot, target_x, 100.0, true)
            .expect("target should spawn");
        let weapon = combat_rules::default_weapon_kind(kind).expect("default weapon should exist");
        let expected_cooldown = combat_rules::attack_profile(kind).cooldown;
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::attack(target_id));
            attacker.set_facing(0.0);
            attacker.set_weapon_facing(0.0);
            attacker.set_emplacement_facing(Some(0.0));
            if matches!(
                kind,
                EntityKind::MachineGunner | EntityKind::AntiTankGun | EntityKind::MortarTeam
            ) {
                attacker.set_weapon_setup(WeaponSetup::Deployed);
            }
            if kind == EntityKind::MortarTeam {
                attacker.set_autocast_enabled(AbilityKind::MortarFire, true);
            }
            if kind == EntityKind::Tank {
                attacker.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 999);
            }
        }

        run_combat_tick(&mut entities);

        let attacker = entities
            .get(attacker_id)
            .expect("attacker should survive first shot");
        assert_eq!(
            attacker.attack_cd(),
            expected_cooldown,
            "{kind} legacy attack_cd should match the default weapon profile after firing"
        );
        assert_eq!(
            attacker.weapon_cooldown(weapon),
            expected_cooldown,
            "{kind} keyed default weapon cooldown should match the legacy cadence"
        );
        let target_hp_after_first = entities
            .get(target_id)
            .map(|target| target.hp)
            .expect("target should survive first shot");

        for _ in 1..expected_cooldown {
            run_combat_tick(&mut entities);
        }

        let attacker = entities
            .get(attacker_id)
            .expect("attacker should survive cooldown wait");
        assert_eq!(
            attacker.attack_cd(),
            1,
            "{kind} should still be one tick away before the next firing tick"
        );
        if kind != EntityKind::MortarTeam {
            assert_eq!(
                entities
                    .get(target_id)
                    .map(|target| target.hp)
                    .expect("target should survive cooldown wait"),
                target_hp_after_first,
                "{kind} should not fire again before the profile cooldown expires"
            );
        }

        run_combat_tick(&mut entities);

        let attacker = entities
            .get(attacker_id)
            .expect("attacker should survive second shot");
        assert_eq!(
            attacker.attack_cd(),
            expected_cooldown,
            "{kind} should reset to the same default weapon cooldown on the next firing tick"
        );
    }
}
