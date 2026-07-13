use super::*;

#[test]
fn set_rally_stores_point_on_producer_and_rejects_others() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");
    let (dx, dy) = footprint_center(&map, EntityKind::Depot, 12, 6);
    let depot = entities
        .spawn_building(1, EntityKind::Depot, dx, dy, true)
        .expect("depot should spawn");
    let (ex, ey) = footprint_center(&map, EntityKind::Barracks, 6, 12);
    let enemy_barracks = entities
        .spawn_building(2, EntityKind::Barracks, ex, ey, true)
        .expect("enemy barracks should spawn");

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 100.0,
                    y: 200.0,
                    kind: RallyKind::Move,
                    queued: false,
                },
            ),
            // Depot trains nothing -> rejected.
            (
                1,
                SimCommand::SetRally {
                    building: depot,
                    x: 50.0,
                    y: 50.0,
                    kind: RallyKind::Move,
                    queued: false,
                },
            ),
            // Not the owner -> rejected.
            (
                1,
                SimCommand::SetRally {
                    building: enemy_barracks,
                    x: 10.0,
                    y: 10.0,
                    kind: RallyKind::Move,
                    queued: false,
                },
            ),
        ],
    );

    assert_eq!(
        entities.get(barracks).unwrap().rally_point(),
        Some((100.0, 200.0)),
        "owned producer should store the rally point"
    );
    assert_eq!(
        entities.get(depot).unwrap().rally_point(),
        None,
        "non-producer building should not accept a rally point"
    );
    assert_eq!(
        entities.get(enemy_barracks).unwrap().rally_point(),
        None,
        "rally on an enemy building should be ignored"
    );
}

#[test]
fn set_rally_clamps_out_of_bounds_point() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetRally {
                building: barracks,
                x: 1.0e9,
                y: -50.0,
                kind: RallyKind::Move,
                queued: false,
            },
        )],
    );

    let max = map.world_size_px() - 1.0;
    assert_eq!(
        entities.get(barracks).unwrap().rally_point(),
        Some((max, 0.0)),
        "rally point should be clamped into the map bounds"
    );
}

#[test]
fn queued_rally_appends_until_four_stages_and_normal_rally_clears_queue() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (bx, by) = footprint_center(&map, EntityKind::Barracks, 6, 6);
    let barracks = entities
        .spawn_building(1, EntityKind::Barracks, bx, by, true)
        .expect("barracks should spawn");

    apply(
        &map,
        &mut entities,
        vec![
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 100.0,
                    y: 100.0,
                    kind: RallyKind::Move,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 200.0,
                    y: 200.0,
                    kind: RallyKind::AttackMove,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 300.0,
                    y: 300.0,
                    kind: RallyKind::Move,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 400.0,
                    y: 400.0,
                    kind: RallyKind::AttackMove,
                    queued: true,
                },
            ),
            (
                1,
                SimCommand::SetRally {
                    building: barracks,
                    x: 500.0,
                    y: 500.0,
                    kind: RallyKind::Move,
                    queued: true,
                },
            ),
        ],
    );

    assert_eq!(
        entities.get(barracks).unwrap().rally_point(),
        Some((100.0, 100.0)),
        "first queued rally should establish the active rally point"
    );
    let stages = entities.get(barracks).unwrap().rally_stages();
    assert_eq!(
        stages.len(),
        3,
        "rally plan should be capped at four total stages"
    );
    assert_eq!(stages[0].kind, RallyKind::AttackMove);
    assert_eq!((stages[0].point.x, stages[0].point.y), (200.0, 200.0));
    assert_eq!((stages[2].point.x, stages[2].point.y), (400.0, 400.0));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetRally {
                building: barracks,
                x: 600.0,
                y: 600.0,
                kind: RallyKind::Move,
                queued: false,
            },
        )],
    );

    let barracks = entities.get(barracks).expect("barracks should exist");
    assert!(barracks.rally_stages().is_empty());
    assert_eq!(barracks.rally_point(), Some((600.0, 600.0)));
}

#[test]
fn coordinate_only_resource_rallies_remain_plain_ground_points() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (cx, cy) = footprint_center(&map, EntityKind::CityCentre, 6, 6);
    let city_centre = entities
        .spawn_building(1, EntityKind::CityCentre, cx, cy, true)
        .expect("city centre should spawn");
    entities
        .spawn_node(EntityKind::Steel, 320.0, 256.0)
        .expect("steel should spawn");
    entities
        .spawn_node(EntityKind::Oil, 384.0, 256.0)
        .expect("oil should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetRally {
                building: city_centre,
                x: 320.0,
                y: 256.0,
                kind: RallyKind::Move,
                queued: false,
            },
        )],
    );
    let rally = entities.get(city_centre).expect("city centre").rally_plan()[0];
    assert_eq!((rally.point.x, rally.point.y), (320.0, 256.0));

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::SetRally {
                building: city_centre,
                x: 384.0,
                y: 256.0,
                kind: RallyKind::Move,
                queued: false,
            },
        )],
    );
    let rally = entities.get(city_centre).expect("city centre").rally_plan()[0];
    assert_eq!((rally.point.x, rally.point.y), (384.0, 256.0));
}
