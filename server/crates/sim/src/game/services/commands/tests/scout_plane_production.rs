use super::*;

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
