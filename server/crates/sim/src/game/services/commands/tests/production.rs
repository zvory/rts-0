use super::*;

#[test]
fn support_weapon_and_tank_training_require_finished_unlock_upgrades() {
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
            UpgradeKind::AntiTankGunUnlock,
            None,
        ),
        (
            EntityKind::Factory,
            EntityKind::Tank,
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
        (EntityKind::Steelworks, UpgradeKind::BallisticTables),
        (EntityKind::Factory, UpgradeKind::TankUnlock),
        (EntityKind::Steelworks, UpgradeKind::MortarAutocast),
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
        if upgrade == UpgradeKind::BallisticTables {
            players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
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
    assert_eq!(queue[0].total, crate::config::TICK_HZ * 10);
    assert_eq!(players[0].steel, 900);
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
fn scout_plane_training_requires_completed_gun_or_vehicle_works_and_uses_city_centre_queue_rules() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let (gw_x, gw_y) = footprint_center(&map, EntityKind::Steelworks, 10, 6);
    entities
        .spawn_building(1, EntityKind::Steelworks, gw_x, gw_y, false)
        .expect("gun works should spawn under construction");
    let mut players = vec![player_state(1), player_state(2)];
    let resources_before = (
        players[0].steel,
        players[0].oil,
        players[0].supply_used,
        players[0].supply_cap,
    );
    let train_scout_plane = SimCommand::Train {
        building: city_centre,
        unit: EntityKind::ScoutPlane,
    };

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, train_scout_plane.clone())],
    );

    assert!(
        entities
            .get(city_centre)
            .expect("city centre")
            .prod_queue()
            .is_empty(),
        "under-construction Gun Works must not unlock Scout Plane training"
    );
    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_before,
        "failed Scout Plane training must not spend resources or reserve supply"
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
    ));

    let (vw_x, vw_y) = footprint_center(&map, EntityKind::Factory, 14, 6);
    entities
        .spawn_building(1, EntityKind::Factory, vw_x, vw_y, true)
        .expect("completed vehicle works should spawn");

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(1, train_scout_plane)],
    );

    let queue = entities.get(city_centre).expect("city centre").prod_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].unit, EntityKind::ScoutPlane);
    assert_eq!(
        queue[0].total,
        crate::config::unit_stats(EntityKind::ScoutPlane)
            .expect("Scout Plane stats")
            .build_ticks
    );
    assert_eq!(players[0].steel, resources_before.0 - 50);
    assert_eq!(players[0].oil, resources_before.1 - 50);
    assert_eq!(players[0].supply_used, resources_before.2);

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Cancel {
                building: city_centre,
            },
        )],
    );

    assert!(entities
        .get(city_centre)
        .expect("city centre")
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
        "canceling queued Scout Plane should use normal City Centre refund behavior"
    );

    let mut gun_works_entities = EntityStore::new();
    let gun_city_centre = gun_works_entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    gun_works_entities
        .spawn_building(1, EntityKind::Steelworks, gw_x, gw_y, true)
        .expect("completed gun works should spawn");
    let mut gun_works_players = vec![player_state(1), player_state(2)];
    apply_with_players(
        &map,
        &mut gun_works_entities,
        &mut gun_works_players,
        vec![(
            1,
            SimCommand::Train {
                building: gun_city_centre,
                unit: EntityKind::ScoutPlane,
            },
        )],
    );
    assert_eq!(
        gun_works_entities
            .get(gun_city_centre)
            .expect("city centre")
            .prod_queue()[0]
            .unit,
        EntityKind::ScoutPlane,
        "completed Gun Works also unlocks Scout Plane training"
    );
}

#[test]
fn scout_plane_training_rejects_second_active_or_in_production_plane_before_spending() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc1_x, cc1_y) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre_a = entities
        .spawn_building(1, EntityKind::CityCentre, cc1_x, cc1_y, true)
        .expect("first city centre should spawn");
    let (cc2_x, cc2_y) = footprint_center(&map, EntityKind::CityCentre, 10, 6);
    let city_centre_b = entities
        .spawn_building(1, EntityKind::CityCentre, cc2_x, cc2_y, true)
        .expect("second city centre should spawn");
    let (vw_x, vw_y) = footprint_center(&map, EntityKind::Factory, 14, 6);
    entities
        .spawn_building(1, EntityKind::Factory, vw_x, vw_y, true)
        .expect("vehicle works should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![
            (
                1,
                SimCommand::Train {
                    building: city_centre_a,
                    unit: EntityKind::ScoutPlane,
                },
            ),
            (
                1,
                SimCommand::Train {
                    building: city_centre_b,
                    unit: EntityKind::ScoutPlane,
                },
            ),
        ],
    );

    assert_eq!(
        entities
            .get(city_centre_a)
            .expect("first city centre")
            .prod_queue()
            .len(),
        1
    );
    assert!(
        entities
            .get(city_centre_b)
            .expect("second city centre")
            .prod_queue()
            .is_empty(),
        "a second City Centre must not queue a second Scout Plane while one is already in production"
    );
    assert_eq!(players[0].steel, 950);
    assert_eq!(players[0].oil, 950);
    assert_notice(&events, 1, "Scout Plane already in production");

    let mut active_entities = EntityStore::new();
    let active_city_centre = active_entities
        .spawn_building(1, EntityKind::CityCentre, cc1_x, cc1_y, true)
        .expect("city centre should spawn");
    active_entities
        .spawn_building(1, EntityKind::Factory, vw_x, vw_y, true)
        .expect("vehicle works should spawn");
    active_entities
        .spawn_unit(1, EntityKind::ScoutPlane, cc1_x, cc1_y)
        .expect("active Scout Plane should spawn");
    let mut active_players = vec![player_state(1), player_state(2)];
    let active_resources_before = (
        active_players[0].steel,
        active_players[0].oil,
        active_players[0].supply_used,
        active_players[0].supply_cap,
    );

    let active_events = apply_with_players(
        &map,
        &mut active_entities,
        &mut active_players,
        vec![(
            1,
            SimCommand::Train {
                building: active_city_centre,
                unit: EntityKind::ScoutPlane,
            },
        )],
    );

    assert!(active_entities
        .get(active_city_centre)
        .expect("city centre")
        .prod_queue()
        .is_empty());
    assert_eq!(
        (
            active_players[0].steel,
            active_players[0].oil,
            active_players[0].supply_used,
            active_players[0].supply_cap,
        ),
        active_resources_before,
        "active Scout Plane rejection must happen before spending resources"
    );
    assert_notice(&active_events, 1, "Scout Plane already active");
}

#[test]
fn scout_plane_training_rejects_second_plane_queued_behind_another_city_centre_item() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cc_x, cc_y) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("city centre should spawn");
    let (vw_x, vw_y) = footprint_center(&map, EntityKind::Factory, 12, 6);
    entities
        .spawn_building(1, EntityKind::Factory, vw_x, vw_y, true)
        .expect("vehicle works should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![
            (
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::Worker,
                },
            ),
            (
                1,
                SimCommand::Train {
                    building: city_centre,
                    unit: EntityKind::ScoutPlane,
                },
            ),
        ],
    );
    let resources_after_queue = (
        players[0].steel,
        players[0].oil,
        players[0].supply_used,
        players[0].supply_cap,
    );
    let queue = entities.get(city_centre).expect("city centre").prod_queue();
    assert_eq!(queue.len(), 2);
    assert_eq!(queue[0].unit, EntityKind::Worker);
    assert_eq!(queue[1].unit, EntityKind::ScoutPlane);

    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Train {
                building: city_centre,
                unit: EntityKind::ScoutPlane,
            },
        )],
    );

    let queue = entities.get(city_centre).expect("city centre").prod_queue();
    assert_eq!(queue.len(), 2);
    assert_eq!(
        (
            players[0].steel,
            players[0].oil,
            players[0].supply_used,
            players[0].supply_cap,
        ),
        resources_after_queue,
        "hidden queued Scout Plane rejection must happen before spending resources"
    );
    assert_notice(&events, 1, "Scout Plane already in production");
}

#[test]
fn panzerfaust_training_reports_resource_and_supply_blocks() {
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
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Not enough oil"
    ));
    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());

    let mut supply_blocked = vec![player_state(1), player_state(2)];
    let supply_cap = supply_blocked[0].supply_cap;
    assert!(supply_blocked[0].reserve_supply(supply_cap));
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut supply_blocked,
        vec![(1, train_panzerfaust)],
    );
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Not enough supply"
    ));
    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
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
fn legacy_artillery_unlock_is_not_current_faction_research() {
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
        Some(Event::Notice { msg, .. }) if msg == "Cannot research that here"
    ));

    players[0].upgrades.insert(UpgradeKind::AntiTankGunUnlock);
    let events = apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
    let queue = entities
        .get(research_complex)
        .expect("research complex")
        .research_queue();
    assert!(queue.is_empty());
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Cannot research that here"
    ));
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
    apply_with_players(&map, &mut entities, &mut players, vec![(1, command)]);
    let queue = entities
        .get(research_complex)
        .expect("research complex")
        .research_queue();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].upgrade, UpgradeKind::BallisticTables);
}

#[test]
fn train_resource_shortages_emit_specific_notices() {
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
    assert!(
        matches!(
            oil_missing_events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Not enough oil"
        ),
        "oil-gated units should emit the oil voice-line notice"
    );

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
    assert!(
        matches!(
            steel_missing_events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Not enough steel"
        ),
        "steel-only units should emit the steel voice-line notice"
    );
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
    let (refunded_steel, refunded_oil) = rules::economy::cost(EntityKind::MachineGunner);
    assert_eq!(players[0].steel, steel_after_queue + refunded_steel);
    assert_eq!(players[0].oil, oil_after_queue + refunded_oil);
    assert_eq!(
        players[0].supply_used,
        supply_after_queue - rules::economy::supply_cost(EntityKind::MachineGunner)
    );
}
