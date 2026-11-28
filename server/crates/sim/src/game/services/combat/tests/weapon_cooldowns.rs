use super::*;

#[test]
fn default_weapon_cooldown_cadence_tracks_randomized_reload() {
    let cases = [
        (EntityKind::Worker, 140.0),
        (EntityKind::Golem, 140.0),
        (EntityKind::Warrior, 160.0),
        (EntityKind::Rifleman, 180.0),
        (EntityKind::MachineGunner, 220.0),
        (EntityKind::ScoutCar, 180.0),
        (EntityKind::AntiTankGun, 220.0),
        (EntityKind::Tank, 180.0),
    ];

    for (kind, target_x) in cases {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let target_id = entities
            .spawn_building(2, EntityKind::ResourceDepot, target_x, 100.0, true)
            .expect("target should spawn");
        let weapon = combat_rules::default_weapon_kind(kind).expect("default weapon should exist");
        let expected_cooldown = combat_rules::attack_profile(kind).cooldown;
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::attack(target_id));
            attacker.set_facing(0.0);
            attacker.set_weapon_facing(0.0);
            attacker.set_emplacement_facing(Some(0.0));
            if matches!(kind, EntityKind::MachineGunner | EntityKind::AntiTankGun) {
                attacker.set_weapon_setup(WeaponSetup::Deployed);
            }
            if kind == EntityKind::Tank {
                attacker.set_weapon_cooldown(combat_rules::WeaponKind::TankCoax, 999);
            }
        }

        run_combat_tick(&mut entities);

        let attacker = entities
            .get(attacker_id)
            .expect("attacker should survive first shot");
        let first_reload = attacker.attack_cd();
        assert!(
            (expected_cooldown - 1..=expected_cooldown + 2).contains(&first_reload),
            "{kind}"
        );
        assert_eq!(
            attacker.weapon_cooldown(weapon),
            first_reload,
            "{kind} keyed default weapon cooldown should match the legacy cadence"
        );
        let target_hp_after_first = entities
            .get(target_id)
            .map(|target| target.hp)
            .expect("target should survive first shot");

        for _ in 1..first_reload {
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
        assert!(
            (expected_cooldown - 1..=expected_cooldown + 2).contains(&attacker.attack_cd()),
            "{kind}"
        );
    }
}

#[test]
fn methamphetamines_randomizes_both_rifle_units_around_twenty_four_ticks() {
    for kind in [EntityKind::Rifleman, EntityKind::Panzerfaust] {
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let target_id = entities
            .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
            .expect("target should spawn");
        entities
            .get_mut(attacker_id)
            .expect("attacker should exist")
            .set_order(Order::attack(target_id));
        let mut meth_player = player_state(1, false);
        meth_player.upgrades.insert(UpgradeKind::Methamphetamines);
        let players = [meth_player, player_state(2, false)];
        assert_eq!(
            crate::rules::combat::default_weapon_profile(kind)
                .expect("rifle profile")
                .cooldown,
            32
        );
        run_combat_tick_with_players(&mut entities, &players);
        let first_reload = entities.get(attacker_id).expect("attacker").attack_cd();
        assert!((23..=26).contains(&first_reload), "{kind}");
        let hp_after_first = entities.get(target_id).expect("target should exist").hp;
        for _ in 1..first_reload {
            run_combat_tick_with_players(&mut entities, &players);
        }
        assert_eq!(entities.get(attacker_id).unwrap().attack_cd(), 1);
        assert_eq!(entities.get(target_id).unwrap().hp, hp_after_first);
        run_combat_tick_with_players(&mut entities, &players);
        assert!(
            (23..=26).contains(&entities.get(attacker_id).expect("attacker").attack_cd()),
            "{kind}"
        );
        assert!(
            entities.get(target_id).expect("target should exist").hp < hp_after_first,
            "{kind} should fire when its randomized reload expires"
        );
    }
}

#[test]
fn synchronized_riflemen_drift_apart_and_reroll_each_firing_cycle() {
    let map = open_map(24);
    let mut entities = EntityStore::new();
    let target = entities
        .spawn_building(2, EntityKind::ResourceDepot, 180.0, 150.0, true)
        .unwrap();
    entities
        .get_mut(target)
        .unwrap()
        .set_spawn_health(1_000_000);
    let attackers: Vec<_> = (0..12)
        .map(|i| {
            let id = entities
                .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0 + i as f32 * 8.0)
                .unwrap();
            entities
                .get_mut(id)
                .unwrap()
                .set_order(Order::attack(target));
            id
        })
        .collect();
    let players = [player_state(1, false), player_state(2, false)];
    let mut histories = vec![Vec::new(); attackers.len()];
    for tick in 10..210 {
        let events = run_combat_tick_on_map_with_seed_smokes_and_tick(
            &mut entities,
            &players,
            &map,
            0,
            &SmokeCloudStore::new(),
            tick,
        );
        for event in &events[&1] {
            if let Event::Attack { from, .. } = event {
                if let Some(index) = attackers.iter().position(|id| id == from) {
                    histories[index].push(tick);
                }
            }
        }
    }
    let second_volley: std::collections::BTreeSet<_> = histories
        .iter()
        .map(|shots| {
            assert_eq!(shots.first(), Some(&10), "opening shot stays responsive");
            assert!(shots.len() >= 5);
            assert!(
                shots
                    .windows(2)
                    .all(|pair| (31..=34).contains(&(pair[1] - pair[0]))),
                "{shots:?}"
            );
            shots[1]
        })
        .collect();
    assert!(second_volley.len() > 1, "identical units must drift apart");
    assert!(
        histories.iter().any(|shots| {
            shots
                .windows(2)
                .map(|pair| pair[1] - pair[0])
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                > 1
        }),
        "reload variation must repeat, rather than only offset the first cycle"
    );
}
