use super::*;

#[test]
fn aggressive_auto_acquisition_prefers_fireable_targets_before_chase_targets() {
    struct Case {
        attacker: EntityKind,
        fireable_target: EntityKind,
        chase_target: EntityKind,
    }

    for case in [
        Case {
            attacker: EntityKind::Rifleman,
            fireable_target: EntityKind::Tank,
            chase_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::Tank,
            fireable_target: EntityKind::PumpJack,
            chase_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::ScoutCar,
            fireable_target: EntityKind::Tank,
            chase_target: EntityKind::Worker,
        },
        Case {
            attacker: EntityKind::AntiTankGun,
            fireable_target: EntityKind::Rifleman,
            chase_target: EntityKind::Tank,
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
        let sight_px = attacker.sight_tiles() as f32 * config::TILE_SIZE as f32;
        let fireable_x = 100.0 + range_px * 0.5;
        let chase_x = 100.0 + (range_px + config::TILE_SIZE as f32).min(sight_px - 1.0);
        assert!(
            chase_x - 100.0 > range_px,
            "{:?} fixture needs a visible target outside weapon range",
            case.attacker
        );
        let fireable_target = spawn_target(&mut entities, case.fireable_target, fireable_x, 100.0);
        let chase_target = spawn_target(&mut entities, case.chase_target, chase_x, 100.0);

        let target = resolve_test_target(
            &map,
            &entities,
            &default_team_relations(),
            attacker_id,
            sight_px,
        );

        assert_eq!(
            target,
            Some(fireable_target),
            "{:?} should shoot the target already in range before chasing {:?}",
            case.attacker,
            case.chase_target
        );
        assert_ne!(target, Some(chase_target));
    }
}

#[test]
fn chasing_units_refresh_paths_that_point_at_the_wrong_goal() {
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
            attacker.set_path(vec![(500.0, 100.0)]);
            attacker.set_path_goal(Some((500.0, 100.0)));
            attacker.set_last_repath_tick(0);
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
        assert_eq!(attacker.target_id(), Some(target_id), "{kind:?}");
        assert_ne!(
            attacker.path_goal(),
            Some((500.0, 100.0)),
            "{kind:?} should discard the stale chase path goal"
        );
        assert_eq!(
            attacker.path_goal(),
            entities
                .get(target_id)
                .map(|target| (target.pos_x, target.pos_y)),
            "{kind:?} should chase the current target position"
        );
    }
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
