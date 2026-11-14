use super::*;
#[test]
fn barbed_wire_build_order_requires_completed_training_centre() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 160.0, 160.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let locked_events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::BarbedWire,
                tile_x: 6,
                tile_y: 6,
                queued: false,
            },
        )],
    );
    assert!(
        matches!(
            locked_events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Requirement not met"
        ),
        "Barbed Wire should be locked before Training Centre"
    );
    assert!(matches!(
        entities.get(worker).map(|entity| entity.order()),
        Some(Order::Idle)
    ));

    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 3, 3);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    let unlocked_events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::BarbedWire,
                tile_x: 6,
                tile_y: 6,
                queued: false,
            },
        )],
    );

    assert!(
        unlocked_events.get(&1).is_none_or(Vec::is_empty),
        "valid Barbed Wire command should not emit a rejection notice: {unlocked_events:?}"
    );
    assert!(matches!(
        entities.get(worker).map(|entity| entity.order()),
        Some(Order::Build(order)) if order.intent.kind == EntityKind::BarbedWire
    ));
}

#[test]
fn barbed_wire_construction_charges_on_arrival_and_uses_spec_build_time() {
    let trap_stats = config::building_stats(EntityKind::BarbedWire).expect("Barbed Wire stats");
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::BarbedWire, 6, 6);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::BarbedWire, 6, 6));
    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 3, 3);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    let mut players = vec![player_state(1), player_state(2)];
    let steel_before = players[0].steel;
    let mut events: HashMap<u32, Vec<Event>> = players
        .iter()
        .map(|player| (player.id, Vec::new()))
        .collect();
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2], &entities, &map);
    let mut active_sites = BTreeSet::new();

    crate::game::services::construction::construction_system(
        &map,
        &mut entities,
        &mut players,
        &mut events,
        &fog,
        &mut active_sites,
    );

    assert_eq!(
        players[0].steel,
        steel_before - trap_stats.cost_steel,
        "construction should charge the authoritative Barbed Wire steel cost"
    );
    let site = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::BarbedWire)
        .expect("Barbed Wire site should spawn");
    assert!(site.under_construction());
    assert_eq!(
        site.construction.as_ref().map(|state| state.total),
        Some(trap_stats.build_ticks)
    );
}
