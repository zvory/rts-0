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
            fireable_target: EntityKind::MortarTeam,
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
fn out_of_range_direct_attacks_create_and_refresh_pursuit_paths() {
    for kind in [EntityKind::Rifleman, EntityKind::MachineGunner] {
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
                    phase: crate::game::entity::AttackPhase::Pursuing,
                },
                ..
            })
        ));
        assert_eq!(attacker.target_id(), Some(target_id), "{kind:?}");
        assert!(!attacker.path_is_empty(), "{kind:?} should pursue");
        assert!(
            attacker.path_goal().is_some(),
            "{kind:?} should have a pursuit goal"
        );

        if let Some(target) = entities.get_mut(target_id) {
            target.set_position(100.0 + range_px * 0.5, target.pos_y);
        }
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");
        assert!(attacker.path_is_empty(), "{kind:?} should stop in range");
        assert_eq!(
            attacker.path_goal(),
            None,
            "{kind:?} should stop pursuing in range"
        );

        if let Some(target) = entities.get_mut(target_id) {
            target.set_position(target_x, target.pos_y);
        }
        run_combat_tick_on_map(
            &mut entities,
            &[player_state(1, false), player_state(2, false)],
            &map,
        );
        let attacker = entities
            .get(attacker_id)
            .expect("attacker should still exist");
        assert!(!attacker.path_is_empty(), "{kind:?} should resume pursuit");
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

#[test]
fn deployed_anti_tank_gun_fires_at_long_range() {
    const TARGET_RANGE_TILES: f32 = config::ANTI_TANK_GUN_DEPLOYED_RANGE_TILES as f32 - 1.0;
    let map = open_map(64);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let target_x = 100.0 + TARGET_RANGE_TILES * config::TILE_SIZE as f32;
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, target_x, 100.0)
        .expect("enemy tank should spawn");
    entities
        .spawn_unit(3, EntityKind::CommandCar, target_x, 132.0)
        .expect("allied spotter should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }
    entities
        .get_mut(tank_id)
        .expect("tank should exist")
        .set_facing(std::f32::consts::PI);
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let mut spotter = player_state(3, false);
    spotter.team_id = 1;
    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false), spotter],
        &map,
    );

    assert!(
        entities.get(tank_id).expect("enemy should exist").hp < enemy_hp,
        "deployed Anti-Tank Gun should fire at range {TARGET_RANGE_TILES} with allied vision"
    );
    assert!(
        events.get(&2).is_some_and(|events| events.iter().any(|event| {
            matches!(
                event,
                Event::Attack {
                    from,
                    to,
                    reveal: Some(reveal),
                    to_pos: Some(to_pos),
                    weapon_kind: Some(weapon_kind),
                } if *from == at_id
                    && *to == tank_id
                    && reveal.kind == crate::protocol::kind_to_wire(EntityKind::AntiTankGun)
                    && reveal.setup_state.as_deref() == Some(WeaponSetup::Deployed.to_protocol_str())
                    && *to_pos == [target_x, 100.0]
                    && weapon_kind == crate::rules::combat::WeaponKind::AntiTankGun.stable_id()
            )
        })),
        "anti-tank attack event should carry shooter reveal and target position for visual feedback"
    );
}

#[test]
fn deployed_anti_tank_gun_does_not_fire_at_former_long_range() {
    const TARGET_RANGE_TILES: f32 = 39.0;
    let map = open_map(64);
    let mut entities = EntityStore::new();
    let at_id = entities
        .spawn_unit(1, EntityKind::AntiTankGun, 100.0, 100.0)
        .expect("anti-tank gun should spawn");
    let target_x = 100.0 + TARGET_RANGE_TILES * config::TILE_SIZE as f32;
    let tank_id = entities
        .spawn_unit(2, EntityKind::Tank, target_x, 100.0)
        .expect("enemy tank should spawn");
    entities
        .spawn_unit(3, EntityKind::CommandCar, target_x, 132.0)
        .expect("allied spotter should spawn");
    if let Some(at) = entities.get_mut(at_id) {
        at.set_weapon_setup(WeaponSetup::Deployed);
        at.set_emplacement_facing(Some(0.0));
        at.set_facing(0.0);
        at.set_weapon_facing(0.0);
    }
    let enemy_hp = entities.get(tank_id).expect("enemy should exist").hp;

    let mut spotter = player_state(3, false);
    spotter.team_id = 1;
    let events = run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false), spotter],
        &map,
    );

    assert_eq!(
        entities.get(tank_id).expect("enemy should exist").hp,
        enemy_hp,
        "deployed anti-tank gun should no longer fire at its former range"
    );
    assert!(
        events.values().flatten().all(|event| {
            !matches!(
                event,
                Event::Attack { from, to, .. } if *from == at_id && *to == tank_id
            )
        }),
        "anti-tank gun should not emit an attack event at its former range"
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
