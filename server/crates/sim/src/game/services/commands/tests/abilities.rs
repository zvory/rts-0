use super::*;

#[test]
fn fixture_faction_point_fire_does_not_spend_steel() {
    let map = flat_map(64);
    let mut players = vec![player_state(1), player_state(2)];
    players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
    let mut entities = EntityStore::new();
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, 320.0, 320.0)
        .expect("artillery should spawn");
    {
        let gun = entities.get_mut(artillery).expect("artillery should exist");
        gun.set_weapon_setup(WeaponSetup::Deployed);
        gun.set_emplacement_facing(Some(0.0));
    }
    let steel_before = players[0].steel;

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::PointFire,
                units: vec![artillery],
                x: Some(960.0),
                y: Some(320.0),
                queued: false,
            },
        )],
    );

    assert_eq!(
        players[0].steel, steel_before,
        "out-of-faction artillery ability must not spend Steel"
    );
    assert_eq!(
        entities
            .get(artillery)
            .expect("artillery should exist")
            .attack_cd(),
        0,
        "out-of-faction artillery ability must not start the firing cooldown"
    );
}

#[test]
fn blanket_fire_command_starts_runtime_order() {
    let map = flat_map(64);
    let mut players = vec![player_state(1), player_state(2)];
    let mut entities = EntityStore::new();
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, 320.0, 320.0)
        .expect("artillery should spawn");
    {
        let gun = entities.get_mut(artillery).expect("artillery should exist");
        gun.set_weapon_setup(WeaponSetup::Deployed);
        gun.set_emplacement_facing(Some(0.0));
        gun.set_weapon_facing(0.0);
    }
    let steel_before = players[0].steel;

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::BlanketFire,
                units: vec![artillery],
                x: Some(960.0),
                y: Some(320.0),
                queued: false,
            },
        )],
    );

    let gun = entities.get(artillery).expect("artillery should exist");
    assert_eq!(
        players[0].steel,
        steel_before - config::ARTILLERY_AMMO_COST_STEEL,
        "Blanket Fire should spend the same artillery ammunition as Point Fire"
    );
    assert_eq!(
        gun.ability_cooldown_ticks(AbilityKind::BlanketFire),
        0,
        "Blanket Fire uses the artillery weapon reload, not an ability cooldown"
    );
    assert_eq!(gun.attack_cd(), config::ARTILLERY_RELOAD_TICKS);
    assert!(
        matches!(gun.order(), Order::ArtilleryBlanketFire { .. }),
        "Blanket Fire must replace the current order with its own runtime order"
    );
}

#[test]
fn set_mortar_autocast_requires_completed_research() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let command = SimCommand::SetAutocast {
        ability: AbilityKind::MortarFire,
        units: vec![mortar],
        enabled: true,
    };
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, command.clone())],
    );
    assert_eq!(
        entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(AbilityKind::MortarFire),
        Some(false),
        "pre-research autocast command should be ignored"
    );

    players[0].upgrades.insert(UpgradeKind::MortarAutocast);
    apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
    assert_eq!(
        entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(AbilityKind::MortarFire),
        Some(true),
        "researched autocast command should be accepted"
    );
}

#[test]
fn set_mortar_autocast_rejects_wrong_faction_carriers() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
    players[0].upgrades.insert(UpgradeKind::MortarAutocast);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::SetAutocast {
                ability: AbilityKind::MortarFire,
                units: vec![mortar],
                enabled: true,
            },
        )],
    );

    assert_eq!(
        entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(AbilityKind::MortarFire),
        Some(false),
        "out-of-faction autocast commands should not toggle carrier state"
    );
}

#[test]
fn legacy_charge_command_is_noop_after_removal() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 120.0, 100.0)
        .expect("worker should spawn");
    let enemy_rifle = entities
        .spawn_unit(2, EntityKind::Rifleman, 140.0, 100.0)
        .expect("enemy rifleman should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Charge,
                units: vec![rifle, worker, enemy_rifle, rifle],
                x: None,
                y: None,
                queued: false,
            },
        )],
    );

    assert_eq!(
        entities
            .get(rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "legacy Charge should not start cooldowns before Training Centre is complete"
    );

    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Charge,
                units: vec![rifle, worker, enemy_rifle, rifle],
                x: None,
                y: None,
                queued: false,
            },
        )],
    );

    assert_eq!(
        entities
            .get(rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "legacy Charge should no longer activate riflemen"
    );
    assert_eq!(
        entities
            .get(worker)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "non-riflemen in the selected list are ignored"
    );
    assert_eq!(
        entities
            .get(enemy_rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "enemy riflemen are ignored"
    );
}

#[test]
fn repeated_legacy_charge_command_remains_noop() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Charge,
                units: vec![rifle],
                x: None,
                y: None,
                queued: false,
            },
        )],
    );
    assert_eq!(
        entities
            .get(rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "legacy Charge should not start a cooldown"
    );

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Charge,
                units: vec![rifle],
                x: None,
                y: None,
                queued: false,
            },
        )],
    );
    assert_eq!(
        entities
            .get(rifle)
            .unwrap()
            .ability_cooldown_ticks(AbilityKind::Charge),
        0,
        "retrying legacy Charge should remain a no-op"
    );
}

#[test]
fn queued_legacy_charge_is_skipped_and_later_attack_move_hits_selection() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let ready = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("ready rifleman should spawn");
    let cooldown = entities
        .spawn_unit(1, EntityKind::Rifleman, 120.0, 100.0)
        .expect("cooldown rifleman should spawn");
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 140.0, 100.0)
        .expect("worker should spawn");
    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    entities
        .get_mut(cooldown)
        .unwrap()
        .start_ability_cooldown(AbilityKind::Charge, 5);

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::UseAbility {
                    ability: AbilityKind::Charge,
                    units: vec![ready, cooldown, worker],
                    x: None,
                    y: None,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::AttackMove {
                    units: vec![ready, cooldown, worker],
                    x: 400.0,
                    y: 100.0,
                    queued: true,
                },
            ),
        ],
    );

    assert_eq!(entities.get(ready).unwrap().queued_orders().len(), 1);
    assert_eq!(
        entities.get(cooldown).unwrap().queued_orders().len(),
        1,
        "cooldown rifleman should skip Charge but still receive the later attack-move"
    );
    assert_eq!(
        entities.get(worker).unwrap().queued_orders().len(),
        1,
        "non-rifleman should skip Charge but still receive the later attack-move"
    );
}

#[test]
fn in_range_smoke_launches_from_furthest_selected_carrier() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let target = map.tile_center(12, 8);
    let near = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
        .expect("near scout car should spawn");
    let far = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 192.0, target.1)
        .expect("far scout car should spawn");
    let (sx, sy) = footprint_center(&map, EntityKind::ResearchComplex, 4, 4);
    entities
        .spawn_building(1, EntityKind::ResearchComplex, sx, sy, true)
        .expect("R&D Complex should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let mut smokes = SmokeCloudStore::new();
    let events = apply_with_players_and_smokes(
        &map,
        &mut entities,
        &mut players,
        &mut smokes,
        normal_pending(vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Smoke,
                units: vec![near, far],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )]),
    );

    assert_eq!(smokes.iter().count(), 0);
    smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
    assert_eq!(smokes.iter().count(), 1);
    assert_eq!(players[0].steel, 1000);
    assert_eq!(players[0].oil, 1000);
    assert_eq!(
        entities
            .get(far)
            .unwrap()
            .ability_uses_remaining(AbilityKind::Smoke),
        Some(config::SCOUT_CAR_SMOKE_CHARGES - 1),
        "furthest in-range selected carrier should spend one charge"
    );
    assert_eq!(
        entities
            .get(near)
            .unwrap()
            .ability_uses_remaining(AbilityKind::Smoke),
        Some(config::SCOUT_CAR_SMOKE_CHARGES)
    );
    assert!(matches!(entities.get(far).unwrap().order(), Order::Idle));
    // Smoke launch emits local canister feedback plus a positioned info notice; no warn/alert events.
    let player_events = events.get(&1).map(Vec::as_slice).unwrap_or(&[]);
    assert!(player_events.iter().any(|ev| matches!(
        ev,
        Event::SmokeLaunch {
            from_x,
            from_y,
            to_x,
            to_y,
            delay_ticks,
        } if (*from_x - (target.0 - 192.0)).abs() < 0.001
            && (*from_y - target.1).abs() < 0.001
            && (*to_x - target.0).abs() < 0.001
            && (*to_y - target.1).abs() < 0.001
            && *delay_ticks == 2
    )));
    assert!(
        player_events.iter().all(|ev| matches!(
            ev,
            Event::Notice {
                severity: crate::protocol::NoticeSeverity::Info,
                ..
            } | Event::SmokeLaunch { .. }
        )),
        "smoke launch should emit at most info-level notices, got: {player_events:?}"
    );
}

#[test]
fn in_range_smoke_preserves_active_move_and_future_queue() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let target = map.tile_center(12, 8);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
        .expect("scout car should spawn");
    let (sx, sy) = footprint_center(&map, EntityKind::ResearchComplex, 4, 4);
    entities
        .spawn_building(1, EntityKind::ResearchComplex, sx, sy, true)
        .expect("R&D Complex should spawn");

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::Move {
                    units: vec![scout],
                    x: 640.0,
                    y: 320.0,
                    queued: false,
                },
            ),
            (
                1,
                SimCommand::Move {
                    units: vec![scout],
                    x: 704.0,
                    y: 384.0,
                    queued: true,
                },
            ),
        ],
    );
    let before_queue = entities.get(scout).unwrap().queued_orders().to_vec();
    assert!(matches!(
        entities.get(scout).unwrap().order(),
        Order::Move(_)
    ));

    let mut players = vec![player_state(1), player_state(2)];
    let mut smokes = SmokeCloudStore::new();
    apply_with_players_and_smokes(
        &map,
        &mut entities,
        &mut players,
        &mut smokes,
        normal_pending(vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Smoke,
                units: vec![scout],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )]),
    );

    assert_eq!(smokes.iter().count(), 0);
    smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
    assert_eq!(smokes.iter().count(), 1);
    let scout_entity = entities.get(scout).expect("scout should remain alive");
    assert!(
        matches!(scout_entity.order(), Order::Move(_)),
        "reactive in-range smoke should not interrupt the active move"
    );
    assert_eq!(
        scout_entity.queued_orders(),
        before_queue.as_slice(),
        "reactive in-range smoke should preserve queued future orders"
    );
}

#[test]
fn mortar_fire_replaces_active_move_order() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    {
        let mortar_entity = entities.get_mut(mortar).expect("mortar should exist");
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.set_order(Order::move_to(640.0, 100.0));
        mortar_entity.set_path(vec![(160.0, 100.0), (640.0, 100.0)]);
        mortar_entity.set_path_goal(Some((640.0, 100.0)));
        mortar_entity.append_queued_order(OrderIntent::move_to(704.0, 100.0));
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::MortarFire,
                units: vec![mortar],
                x: Some(292.0),
                y: Some(100.0),
                queued: false,
            },
        )],
    );

    let mortar_entity = entities.get(mortar).expect("mortar should remain alive");
    assert!(
        !matches!(mortar_entity.order(), Order::Move(_)),
        "manual Mortar Fire should replace the active move immediately"
    );
    assert!(
        mortar_entity.ability_cooldown_ticks(AbilityKind::MortarFire) > 0,
        "manual Mortar Fire should launch immediately and start cooldown"
    );
    assert!(
        mortar_entity.path_is_empty(),
        "replacing movement should stop the current path"
    );
    assert_eq!(
        mortar_entity.path_goal(),
        None,
        "in-range Mortar Fire should hold at the current position"
    );
    assert!(
        mortar_entity.queued_orders().is_empty(),
        "non-queued Mortar Fire should clear future queued orders"
    );
}

#[test]
fn queued_mortar_fire_appends_while_reload_is_waitable() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    {
        let mortar_entity = entities.get_mut(mortar).expect("mortar should exist");
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.start_ability_cooldown(AbilityKind::MortarFire, 5);
        mortar_entity.set_attack_cd(5);
    }

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::MortarFire,
                units: vec![mortar],
                x: Some(292.0),
                y: Some(100.0),
                queued: true,
            },
        )],
    );

    let mortar_entity = entities.get(mortar).expect("mortar should remain alive");
    assert_eq!(mortar_entity.queued_orders().len(), 1);
    assert!(
        matches!(
            mortar_entity.queued_orders().first(),
            Some(OrderIntent::WorldAbility(intent))
                if intent.ability == AbilityKind::MortarFire
                    && intent.x == 292.0
                    && intent.y == 100.0
        ),
        "queued Mortar Fire should append even while its cooldown/weapon cycle is waiting"
    );
}

#[test]
fn queued_smoke_appends_to_eligible_carriers_until_charges_reserved() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let target = map.tile_center(12, 8);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
        .expect("scout car should spawn");
    let rifle = entities
        .spawn_unit(1, EntityKind::Rifleman, target.0 - 64.0, target.1)
        .expect("rifleman should spawn");
    let (sx, sy) = footprint_center(&map, EntityKind::ResearchComplex, 4, 4);
    entities
        .spawn_building(1, EntityKind::ResearchComplex, sx, sy, true)
        .expect("R&D Complex should spawn");

    apply(
        &map,
        &mut entities,
        (0..10)
            .map(|_| {
                (
                    1,
                    SimCommand::UseAbility {
                        ability: AbilityKind::Smoke,
                        units: vec![scout, rifle],
                        x: Some(target.0),
                        y: Some(target.1),
                        queued: true,
                    },
                )
            })
            .collect(),
    );

    assert_eq!(
        entities.get(scout).unwrap().queued_orders().len(),
        config::SCOUT_CAR_SMOKE_CHARGES as usize,
        "queued Smoke should reserve the Scout Car's stored charges"
    );
    assert!(entities
        .get(scout)
        .unwrap()
        .queued_orders()
        .iter()
        .all(|intent| matches!(intent, OrderIntent::WorldAbility(_))));
    assert!(
        entities.get(rifle).unwrap().queued_orders().is_empty(),
        "non-carriers should not receive queued Smoke intents"
    );

    apply(
        &map,
        &mut entities,
        vec![(1, SimCommand::Stop { units: vec![scout] })],
    );
    assert!(entities.get(scout).unwrap().queued_orders().is_empty());
}

#[test]
fn queued_smoke_distributes_one_click_per_ready_scout_by_queue_length() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let target = map.tile_center(12, 8);
    let first = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
        .expect("first scout car should spawn");
    let second = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 128.0, target.1)
        .expect("second scout car should spawn");
    let cooling = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 160.0, target.1)
        .expect("cooling scout car should spawn");
    entities
        .get_mut(cooling)
        .unwrap()
        .start_ability_cooldown(AbilityKind::Smoke, 5);
    let (sx, sy) = footprint_center(&map, EntityKind::ResearchComplex, 4, 4);
    entities
        .spawn_building(1, EntityKind::ResearchComplex, sx, sy, true)
        .expect("R&D Complex should spawn");

    apply(
        &map,
        &mut entities,
        (0..5)
            .map(|i| {
                (
                    1,
                    SimCommand::UseAbility {
                        ability: AbilityKind::Smoke,
                        units: vec![first, second, cooling],
                        x: Some(target.0 + i as f32),
                        y: Some(target.1),
                        queued: true,
                    },
                )
            })
            .collect(),
    );

    assert_eq!(entities.get(first).unwrap().queued_orders().len(), 2);
    assert_eq!(entities.get(second).unwrap().queued_orders().len(), 2);
    assert!(
        entities.get(cooling).unwrap().queued_orders().is_empty(),
        "cooldown scout car should not receive queued smoke at issue time"
    );
}

#[test]
fn smoke_launches_without_resource_cost() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let target = map.tile_center(12, 8);
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, target.0 - 96.0, target.1)
        .expect("scout car should spawn");
    let (sx, sy) = footprint_center(&map, EntityKind::ResearchComplex, 4, 4);
    entities
        .spawn_building(1, EntityKind::ResearchComplex, sx, sy, true)
        .expect("R&D Complex should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    assert!(players[0].spend_cost(rules::economy::ResourceCost::new(1_000, 1_000)));
    let mut smokes = SmokeCloudStore::new();

    let events = apply_with_players_and_smokes(
        &map,
        &mut entities,
        &mut players,
        &mut smokes,
        normal_pending(vec![(
            1,
            SimCommand::UseAbility {
                ability: AbilityKind::Smoke,
                units: vec![scout],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
        )]),
    );

    assert_eq!(smokes.iter().count(), 0);
    smokes.spawn_due(1 + config::SMOKE_LAUNCH_MAX_DELAY_TICKS);
    assert_eq!(smokes.iter().count(), 1);
    assert_eq!(players[0].steel, 0);
    assert_eq!(players[0].oil, 0);
    assert!(events.get(&1).is_none_or(|events| {
        events.iter().all(|ev| {
            matches!(
                ev,
                Event::Notice {
                    severity: crate::protocol::NoticeSeverity::Info,
                    ..
                } | Event::SmokeLaunch { .. }
            )
        })
    }));
}
