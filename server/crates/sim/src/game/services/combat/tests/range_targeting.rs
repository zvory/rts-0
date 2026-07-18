use super::*;

#[test]
fn automatic_acquisition_considers_only_targets_in_weapon_range() {
    struct Case {
        attacker: EntityKind,
        fireable_target: EntityKind,
        out_of_range_target: EntityKind,
    }

    for case in [
        Case {
            attacker: EntityKind::Rifleman,
            fireable_target: EntityKind::Tank,
            out_of_range_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::Tank,
            fireable_target: EntityKind::PumpJack,
            out_of_range_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::ScoutCar,
            fireable_target: EntityKind::Tank,
            out_of_range_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::AntiTankGun,
            fireable_target: EntityKind::Rifleman,
            out_of_range_target: EntityKind::Tank,
        },
    ] {
        let map = open_map(32);
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, case.attacker, 100.0, 100.0)
            .expect("attacker should spawn");
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should exist for range setup");
        let profile = effective_attack_profile(attacker);
        let range_px =
            profile.range_tiles * config::TILE_SIZE as f32 + attacker.radius() + RANGE_SLACK;
        let fireable_x = 100.0 + range_px * 0.5;
        let out_of_range_x = 100.0 + range_px + config::TILE_SIZE as f32;
        let fireable_target = spawn_target(&mut entities, case.fireable_target, fireable_x, 100.0);
        let out_of_range_target = spawn_target(
            &mut entities,
            case.out_of_range_target,
            out_of_range_x,
            100.0,
        );

        let target = resolve_test_target(
            &map,
            &entities,
            &default_team_relations(),
            attacker_id,
            range_px,
        );

        assert_eq!(
            target,
            Some(fireable_target),
            "{:?} should select the target already in range and ignore {:?} beyond it",
            case.attacker,
            case.out_of_range_target
        );
        assert_ne!(target, Some(out_of_range_target));
    }
}

#[test]
fn out_of_range_direct_attacks_do_not_create_or_refresh_paths() {
    for kind in [
        EntityKind::Rifleman,
        EntityKind::MachineGunner,
        EntityKind::AntiTankGun,
    ] {
        let map = open_map(32);
        let mut entities = EntityStore::new();
        let attacker_id = entities
            .spawn_unit(1, kind, 100.0, 100.0)
            .expect("attacker should spawn");
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should exist for range setup");
        let profile = effective_attack_profile(attacker);
        let range_px =
            profile.range_tiles * config::TILE_SIZE as f32 + attacker.radius() + RANGE_SLACK;
        let sight_px = attacker.sight_tiles() as f32 * config::TILE_SIZE as f32;
        let target_x = 100.0 + (range_px + config::TILE_SIZE as f32).min(sight_px - 1.0);
        assert!(
            target_x - 100.0 > range_px,
            "{kind:?} fixture needs a visible target outside weapon range"
        );
        let target_id = entities
            .spawn_unit(2, EntityKind::Rifleman, target_x, 100.0)
            .expect("target should spawn");
        if let Some(attacker) = entities.get_mut(attacker_id) {
            attacker.set_order(Order::attack(target_id));
            attacker.set_target_id(Some(target_id));
            if kind == EntityKind::AntiTankGun {
                attacker.set_weapon_setup(WeaponSetup::Packed);
            }
        }

        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );

        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");
        assert!(matches!(
            attacker.order(),
            Order::Attack(crate::game::entity::AttackOrder {
                execution: crate::game::entity::AttackExecution {
                    phase: crate::game::entity::AttackPhase::Waiting,
                },
                ..
            })
        ));
        assert_eq!(attacker.target_id(), Some(target_id), "{kind:?}");
        assert!(attacker.path_is_empty(), "{kind:?} should stay put");
        assert_eq!(
            attacker.path_goal(),
            None,
            "{kind:?} should have no pursuit goal"
        );
    }
}

#[test]
fn mortar_autocast_candidates_respect_exact_max_range() {
    let map = open_map(32);
    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_weapon_setup(WeaponSetup::Deployed);
    }
    let mortar = entities
        .get(mortar_id)
        .expect("mortar should exist for range setup");
    let profile = effective_attack_profile(mortar);
    let exact_range = profile.range_tiles * config::TILE_SIZE as f32;
    let padded_range = exact_range + mortar.radius() + RANGE_SLACK;
    let target_x = 100.0 + (exact_range + (padded_range - exact_range) * 0.5);
    let _target_id = spawn_target(&mut entities, EntityKind::Rifleman, target_x, 100.0);
    let attacker = entities.get(mortar_id).expect("mortar should still exist");
    let target = resolve_target(
        &map,
        &entities,
        &default_team_relations(),
        &SpatialIndex::build(&entities, map.size),
        &LineOfSight::new(&map),
        &visible_fog(&map, &entities),
        &SmokeCloudStore::new(),
        mortar_id,
        attacker.owner,
        attacker.pos_x,
        attacker.pos_y,
        padded_range,
        combat_mode(attacker),
    );
    assert_eq!(
        target, None,
        "mortar should not select targets outside its exact max range"
    );
}

fn spawn_target(entities: &mut EntityStore, kind: EntityKind, x: f32, y: f32) -> u32 {
    if kind.is_building() {
        entities
            .spawn_building(2, kind, x, y, true)
            .expect("building target should spawn")
    } else {
        entities
            .spawn_unit(2, kind, x, y)
            .expect("unit target should spawn")
    }
}
