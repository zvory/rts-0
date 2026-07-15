use super::*;

#[test]
fn support_weapon_and_vehicle_training_require_finished_unlock_upgrades() {
    let map = flat_map(24);
    for (producer, unit, upgrade, setup_extra) in [
        (
            EntityKind::Steelworks,
            EntityKind::AntiTankGun,
            UpgradeKind::AntiTankGunUnlock,
            None,
        ),
        (
            EntityKind::Steelworks,
            EntityKind::Artillery,
            UpgradeKind::ArtilleryUnlock,
            None,
        ),
        (
            EntityKind::Factory,
            EntityKind::Tank,
            UpgradeKind::TankUnlock,
            None,
        ),
        (
            EntityKind::Factory,
            EntityKind::CommandCar,
            UpgradeKind::TankUnlock,
            None,
        ),
    ] {
        let mut entities = EntityStore::new();
        let (px, py) = footprint_center(&map, producer, 6, 6);
        let building = entities
            .spawn_building(1, producer, px, py, true)
            .expect("producer should spawn");
        if let Some(kind) = setup_extra {
            let (x, y) = footprint_center(&map, kind, 10, 6);
            entities
                .spawn_building(1, kind, x, y, true)
                .expect("tech building should spawn");
        }
        let mut players = vec![player_state(1), player_state(2)];
        let command = SimCommand::Train { building, unit };
        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(1, command.clone())],
        );
        assert!(
            entities
                .get(building)
                .expect("producer")
                .prod_queue()
                .is_empty(),
            "{unit:?} should not queue before {upgrade:?} finishes"
        );
        assert!(matches!(
            events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Upgrade required"
        ));

        players[0].upgrades.insert(upgrade);
        apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
        let queue = entities.get(building).expect("producer").prod_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].unit, unit);
    }
}

#[test]
fn advanced_unlocks_research_only_at_research_complex() {
    let map = flat_map(24);
    for (wrong_building_kind, upgrade) in [
        (EntityKind::Steelworks, UpgradeKind::AntiTankGunUnlock),
        (EntityKind::Steelworks, UpgradeKind::ArtilleryUnlock),
        (EntityKind::Steelworks, UpgradeKind::BallisticTables),
        (EntityKind::Factory, UpgradeKind::TankUnlock),
        (EntityKind::Steelworks, UpgradeKind::MortarAutocast),
        (EntityKind::Steelworks, UpgradeKind::SmokePlus),
    ] {
        let mut entities = EntityStore::new();
        let (wrong_x, wrong_y) = footprint_center(&map, wrong_building_kind, 4, 4);
        let wrong_building = entities
            .spawn_building(1, wrong_building_kind, wrong_x, wrong_y, true)
            .expect("wrong research building should spawn");
        let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 10, 4);
        let research_complex = entities
            .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
            .expect("research complex should spawn");
        let mut players = vec![player_state(1), player_state(2)];
        if upgrade == UpgradeKind::ArtilleryUnlock {
            players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
        } else if upgrade == UpgradeKind::BallisticTables {
            players[0].upgrades.insert(UpgradeKind::ArtilleryUnlock);
        }
        let events = apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::Research {
                    building: wrong_building,
                    upgrade,
                },
            )],
        );
        assert!(entities
            .get(wrong_building)
            .expect("wrong building")
            .research_queue()
            .is_empty());
        assert!(matches!(
            events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Cannot research that here"
        ));

        apply_with_players(
            &map,
            &mut entities,
            &mut players,
            vec![(
                1,
                SimCommand::Research {
                    building: research_complex,
                    upgrade,
                },
            )],
        );
        let queue = entities
            .get(research_complex)
            .expect("research complex")
            .research_queue();
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].upgrade, upgrade);
    }
}

#[test]
fn entrenchment_researches_at_training_centre_with_contract_cost_and_time() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (tc_x, tc_y) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
    let training_centre = entities
        .spawn_building(1, EntityKind::TrainingCentre, tc_x, tc_y, true)
        .expect("training centre should spawn");
    let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 12, 6);
    let research_complex = entities
        .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
        .expect("research complex should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Research {
                building: research_complex,
                upgrade: UpgradeKind::Entrenchment,
            },
        )],
    );
    assert!(entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Cannot research that here"
    ));

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Research {
                building: training_centre,
                upgrade: UpgradeKind::Entrenchment,
            },
        )],
    );
    let queue = entities
        .get(training_centre)
        .expect("training centre")
        .research_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].upgrade, UpgradeKind::Entrenchment);
    assert_eq!(queue[0].progress, 0);
    assert_eq!(queue[0].total, crate::config::TICK_HZ * 30);
    assert_eq!(players[0].steel, 800);
    assert_eq!(players[0].oil, 1_000);
}

#[test]
fn panzerfaust_training_requires_completed_training_centre_and_uses_barracks_queue_rules() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    let (tc_x, tc_y) = footprint_center(&map, EntityKind::TrainingCentre, 10, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tc_x, tc_y, false)
        .expect("training centre should spawn under construction");
    let mut players = vec![player_state(1), player_state(2)];
    let resources_before = (
        players[0].steel,
        players[0].oil,
        players[0].supply_used,
        players[0].supply_cap,
    );
    let train_panzerfaust = SimCommand::Train {
        building: barracks,
        unit: EntityKind::Panzerfaust,
    };

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, train_panzerfaust.clone())],
    );

    assert!(
        entities
            .get(barracks)
            .expect("barracks")
            .prod_queue()
            .is_empty(),
        "under-construction Training Centre must not unlock Panzerfaust training"
    );
    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_before,
        "failed Panzerfaust training must not spend resources or reserve supply"
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
    ));

    let (tc2_x, tc2_y) = footprint_center(&map, EntityKind::TrainingCentre, 14, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tc2_x, tc2_y, true)
        .expect("completed training centre should spawn");

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, train_panzerfaust)],
    );

    let queue = entities.get(barracks).expect("barracks").prod_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].unit, EntityKind::Panzerfaust);
    assert_eq!(
        queue[0].total,
        crate::config::unit_stats(EntityKind::Panzerfaust)
            .expect("Panzerfaust stats")
            .build_ticks
    );
    assert_eq!(players[0].steel, resources_before.0 - 60);
    assert_eq!(players[0].oil, resources_before.1 - 15);
    assert_eq!(players[0].supply_used, resources_before.2 + 1);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, SimCommand::Cancel { building: barracks })],
    );

    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_before,
        "canceling queued Panzerfaust should use normal Barracks refund and supply release"
    );
}

#[test]
fn panzerfaust_training_waits_for_resource_and_supply_blocks() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    let (tc_x, tc_y) = footprint_center(&map, EntityKind::TrainingCentre, 10, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tc_x, tc_y, true)
        .expect("training centre should spawn");
    let train_panzerfaust = SimCommand::Train {
        building: barracks,
        unit: EntityKind::Panzerfaust,
    };

    let mut oil_blocked = vec![player_state(1), player_state(2)];
    assert!(oil_blocked[0].spend_cost(rules::economy::ResourceCost::new(0, 1_000)));
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut oil_blocked,
        vec![(1, train_panzerfaust.clone())],
    );
    assert!(events.get(&1).is_none_or(Vec::is_empty));
    let queue = entities.get(barracks).expect("barracks").prod_queue();
    assert_eq!(queue.len(), 1);
    assert!(!queue[0].paid);

    entities
        .get_mut(barracks)
        .expect("barracks")
        .pop_last_production();

    let mut supply_blocked = vec![player_state(1), player_state(2)];
    let supply_cap = supply_blocked[0].supply_cap;
    assert!(supply_blocked[0].reserve_supply(supply_cap));
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut supply_blocked,
        vec![(1, train_panzerfaust)],
    );
    assert!(events.get(&1).is_none_or(Vec::is_empty));
    let queue = entities.get(barracks).expect("barracks").prod_queue();
    assert_eq!(queue.len(), 1);
    assert!(!queue[0].paid);
}

#[test]
fn fixture_faction_rejects_global_build_train_and_research_commands() {
    let map = flat_map(24);
    let mut players = vec![player_state(1), player_state(2)];
    players[0].faction_id = rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string();
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 96.0, 96.0)
        .expect("worker should spawn");
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 5, 5);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 10, 5);
    let research_complex = entities
        .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
        .expect("research complex should spawn");
    let resources_before = (
        players[0].steel,
        players[0].oil,
        players[0].supply_used,
        players[0].supply_cap,
    );

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![
            (
                1,
                SimCommand::Build {
                    units: vec![worker],
                    building: EntityKind::Depot,
                    tile_x: 8,
                    tile_y: 8,
                    queued: false,
                },
            ),
            (
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::Worker,
                },
            ),
            (
                1,
                SimCommand::Research {
                    building: research_complex,
                    upgrade: UpgradeKind::TankUnlock,
                },
            ),
        ],
    );

    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_before,
        "fixture-faction illegal build/train/research commands must not spend Steel/Oil or reserve Supply"
    );
    assert!(
        !matches!(
            entities.get(worker).expect("worker").order(),
            Order::Build(_)
        ),
        "fixture faction worker must not receive a current-faction build order"
    );
    assert!(
        entities
            .get(city_centre)
            .expect("city centre")
            .prod_queue()
            .is_empty(),
        "fixture faction must not train globally-defined current units"
    );
    assert!(
        entities
            .get(research_complex)
            .expect("research complex")
            .research_queue()
            .is_empty(),
        "fixture faction must not research globally-defined current upgrades"
    );
    let notices: Vec<_> = events
        .get(&1)
        .into_iter()
        .flatten()
        .filter_map(|event| match event {
            Event::Notice { msg, .. } => Some(msg.as_str()),
            _ => None,
        })
        .collect();
    assert!(notices.contains(&"Cannot train that here"));
    assert!(notices.contains(&"Cannot research that here"));
}

#[test]
fn heavy_guns_research_requires_medium_guns() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 6, 6);
    let research_complex = entities
        .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
        .expect("research complex should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let command = SimCommand::Research {
        building: research_complex,
        upgrade: UpgradeKind::ArtilleryUnlock,
    };

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, command.clone())],
    );
    assert!(entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
    ));

    players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
    apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
    let queue = entities
        .get(research_complex)
        .expect("research complex")
        .research_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].upgrade, UpgradeKind::ArtilleryUnlock);
}

#[test]
fn ballistic_tables_research_requires_heavy_guns() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (rd_x, rd_y) = footprint_center(&map, EntityKind::ResearchComplex, 6, 6);
    let research_complex = entities
        .spawn_building(1, EntityKind::ResearchComplex, rd_x, rd_y, true)
        .expect("research complex should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let command = SimCommand::Research {
        building: research_complex,
        upgrade: UpgradeKind::BallisticTables,
    };

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, command.clone())],
    );
    assert!(entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
    ));

    players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, command.clone())],
    );
    assert!(entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
    ));

    players[0].upgrades.insert(UpgradeKind::ArtilleryUnlock);
    apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
    let queue = entities
        .get(research_complex)
        .expect("research complex")
        .research_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].upgrade, UpgradeKind::BallisticTables);
}

#[test]
fn manual_train_resource_shortages_create_unpaid_queue_entries() {
    let map = flat_map(24);

    let mut oil_missing_entities = EntityStore::new();
    let (fx, fy) = footprint_center(&map, EntityKind::Factory, 6, 6);
    let factory = oil_missing_entities
        .spawn_building(1, EntityKind::Factory, fx, fy, true)
        .expect("factory should spawn");
    let mut oil_missing_players = vec![player_state(1), player_state(2)];
    assert!(oil_missing_players[0].spend_cost(rules::economy::ResourceCost::new(0, 1_000)));
    let oil_missing_events = apply_with_players(
        &map,
        &mut oil_missing_entities,
        &mut oil_missing_players,
        vec![(
            1,
            SimCommand::Train {
                building: factory,
                unit: EntityKind::ScoutCar,
            },
        )],
    );
    let oil_queue = oil_missing_entities
        .get(factory)
        .expect("factory")
        .prod_queue();
    assert_eq!(oil_queue.len(), 1);
    assert!(
        !oil_queue[0].paid,
        "broke manual training should wait unpaid"
    );
    assert!(oil_missing_events.get(&1).is_none_or(Vec::is_empty));

    let mut steel_missing_entities = EntityStore::new();
    let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = steel_missing_entities
        .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
        .expect("city centre should spawn");
    let mut steel_missing_players = vec![player_state(1), player_state(2)];
    assert!(steel_missing_players[0].spend_cost(rules::economy::ResourceCost::new(1_000, 0)));
    let steel_missing_events = apply_with_players(
        &map,
        &mut steel_missing_entities,
        &mut steel_missing_players,
        vec![(
            1,
            SimCommand::Train {
                building: city_centre,
                unit: EntityKind::Worker,
            },
        )],
    );
    let steel_queue = steel_missing_entities
        .get(city_centre)
        .expect("city centre")
        .prod_queue();
    assert_eq!(steel_queue.len(), 1);
    assert!(
        !steel_queue[0].paid,
        "steel-short training should wait unpaid"
    );
    assert!(steel_missing_events.get(&1).is_none_or(Vec::is_empty));
}

#[test]
fn manual_research_shortage_creates_unpaid_queue_entry() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::TrainingCentre, 6, 6);
    let training_centre = entities
        .spawn_building(1, EntityKind::TrainingCentre, x, y, true)
        .expect("training centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0].set_resources(0, 0);

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Research {
                building: training_centre,
                upgrade: UpgradeKind::Entrenchment,
            },
        )],
    );

    let queue = entities
        .get(training_centre)
        .expect("training centre")
        .research_queue();
    assert_eq!(queue.len(), 1);
    assert!(!queue[0].paid);
    assert!(events.get(&1).is_none_or(Vec::is_empty));
}

#[test]
fn manual_production_queue_is_capped_even_when_entries_are_unpaid() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, x, y, true)
        .expect("city centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    players[0].set_resources(0, 0);

    let commands = (0..=crate::game::entity::MAX_PRODUCTION_QUEUE)
        .map(|_| {
            (
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::Worker,
                },
            )
        })
        .collect();
    let events = apply_with_players(&map, &mut entities, &mut players, commands);

    assert_eq!(
        entities
            .get(city_centre)
            .expect("city centre")
            .prod_queue()
            .len(),
        crate::game::entity::MAX_PRODUCTION_QUEUE
    );
    assert!(events.get(&1).is_some_and(|events| events.iter().any(
        |event| matches!(event, Event::Notice { msg, .. } if msg == "Production queue full")
    )));
}

#[test]
fn cancel_train_removes_latest_queued_unit_without_resetting_active_progress() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 10, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Train {
                building: barracks,
                unit: EntityKind::Rifleman,
            },
        )],
    );
    entities
        .get_mut(barracks)
        .expect("barracks should exist")
        .set_front_production_progress(17);
    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Train {
                building: barracks,
                unit: EntityKind::MachineGunner,
            },
        )],
    );
    let steel_after_queue = players[0].steel;
    let oil_after_queue = players[0].oil;
    let supply_after_queue = players[0].supply_used;

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, SimCommand::Cancel { building: barracks })],
    );

    let queue = entities.get(barracks).expect("barracks").prod_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].unit, EntityKind::Rifleman);
    assert_eq!(
        queue[0].progress, 17,
        "canceling queued production should not reset active progress"
    );
    assert!(!queue
        .iter()
        .any(|item| item.unit == EntityKind::MachineGunner));
    assert_eq!(players[0].steel, steel_after_queue);
    assert_eq!(players[0].oil, oil_after_queue);
    assert_eq!(players[0].supply_used, supply_after_queue);
}

#[test]
fn manual_training_appends_behind_repeated_unit_and_cancel_clears_repeat() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 10, 6);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: vec![barracks],
                unit: EntityKind::Rifleman,
                delta: 1,
            },
        )],
    );
    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::AdjustProductionRepeat {
                buildings: vec![barracks],
                unit: EntityKind::MachineGunner,
                delta: 1,
            },
        )],
    );
    entities
        .get_mut(barracks)
        .expect("barracks")
        .push_production(ProdItem {
            unit: EntityKind::Rifleman,
            progress: 7,
            total: 100,
            paid: true,
        });
    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Train {
                building: barracks,
                unit: EntityKind::MachineGunner,
            },
        )],
    );

    let producer = entities.get(barracks).expect("barracks");
    assert_eq!(
        &producer
            .production
            .as_ref()
            .expect("production")
            .repeat_units,
        &[EntityKind::Rifleman, EntityKind::MachineGunner]
    );
    assert_eq!(producer.prod_queue().len(), 2);
    assert_eq!(producer.prod_queue()[0].unit, EntityKind::Rifleman);
    assert_eq!(producer.prod_queue()[0].progress, 7);
    assert_eq!(producer.prod_queue()[1].unit, EntityKind::MachineGunner);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, SimCommand::Cancel { building: barracks })],
    );
    let producer = entities.get(barracks).expect("barracks");
    assert!(producer
        .production
        .as_ref()
        .expect("production")
        .repeat_units
        .is_empty());
    assert_eq!(producer.prod_queue().len(), 1);
    assert_eq!(producer.prod_queue()[0].unit, EntityKind::Rifleman);
}
