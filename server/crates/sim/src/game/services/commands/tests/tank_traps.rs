use super::*;

#[test]
fn tank_trap_build_order_is_rejected_while_disabled() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 160.0, 160.0)
        .expect("worker should spawn");
    let mut players = vec![player_state(1), player_state(2)];

    let (tx, ty) = footprint_center(&map, EntityKind::TrainingCentre, 3, 3);
    entities
        .spawn_building(1, EntityKind::TrainingCentre, tx, ty, true)
        .expect("training centre should spawn");
    let events = apply_with_players(
        &map,
        &mut entities,
        &mut players,
        vec![(
            1,
            SimCommand::Build {
                units: vec![worker],
                building: EntityKind::TankTrap,
                tile_x: 6,
                tile_y: 6,
                queued: false,
            },
        )],
    );

    assert!(
        matches!(
            events.get(&1).and_then(|events| events.first()),
            Some(Event::Notice { msg, .. }) if msg == "Building unavailable"
        ),
        "Tank Trap build commands remain rejected after the former Training Centre requirement"
    );
    assert!(matches!(
        entities.get(worker).map(|entity| entity.order()),
        Some(Order::Idle)
    ));
}

#[test]
fn tank_trap_construction_charges_on_arrival_and_uses_spec_build_time() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::TankTrap, 6, 6);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::TankTrap, 6, 6));
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
    let mut fog = Fog::new(map.size);
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

    assert_eq!(players[0].steel, steel_before - 15);
    let site = entities
        .iter()
        .find(|entity| entity.kind == EntityKind::TankTrap)
        .expect("Tank Trap site should spawn");
    assert!(site.under_construction());
    assert_eq!(
        site.construction.as_ref().map(|state| state.total),
        Some(config::TICK_HZ * 10)
    );
}

#[test]
fn tank_trap_arrival_recheck_waits_on_vehicle_body_then_times_out() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let (x, y) = footprint_center(&map, EntityKind::TankTrap, 6, 6);
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, x - config::TILE_SIZE as f32, y)
        .expect("worker should spawn");
    entities
        .get_mut(worker)
        .expect("worker should exist")
        .set_order(Order::build(EntityKind::TankTrap, 6, 6));
    entities
        .spawn_unit(1, EntityKind::Tank, x, y)
        .expect("tank should spawn");
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
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);
    let mut active_sites = BTreeSet::new();

    for _ in 0..config::TICK_HZ * 3 - 1 {
        crate::game::services::construction::construction_system(
            &map,
            &mut entities,
            &mut players,
            &mut events,
            &fog,
            &mut active_sites,
        );
        assert!(
            matches!(
                entities.get(worker).expect("worker should survive").order(),
                Order::Build(_)
            ),
            "Tank Trap build should keep waiting before the unit-block grace expires"
        );
    }

    assert_eq!(players[0].steel, steel_before);
    assert!(
        entities
            .iter()
            .all(|entity| entity.kind != EntityKind::TankTrap),
        "blocked arrival must not spawn the obstacle before timeout"
    );
    assert!(
        events.get(&1).is_none_or(Vec::is_empty),
        "unit-blocked wait should not emit a failure notice before timeout"
    );

    crate::game::services::construction::construction_system(
        &map,
        &mut entities,
        &mut players,
        &mut events,
        &fog,
        &mut active_sites,
    );

    assert!(matches!(
        entities.get(worker).expect("worker should survive").order(),
        Order::Idle
    ));
    assert!(matches!(
        events.get(&1).and_then(|events| events.first()),
        Some(Event::Notice { msg, .. }) if msg == "Cannot build there"
    ));
}
