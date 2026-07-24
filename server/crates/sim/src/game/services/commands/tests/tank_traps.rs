use super::*;

#[test]
fn tank_trap_cluster_flag_falls_back_to_direct_attack_for_other_targets() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 160.0, 160.0)
        .expect("tank should spawn");
    let target = entities
        .spawn_unit(2, EntityKind::Tank, 256.0, 160.0)
        .expect("target should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::AttackTankTrapCluster {
                units: vec![tank],
                target,
                queued: false,
            },
        )],
    );

    let Order::Attack(order) = entities.get(tank).expect("tank should survive").order() else {
        panic!("cluster flag should preserve a direct attack on other target kinds");
    };
    assert_eq!(order.intent.target, target);
    assert!(order.intent.remaining_targets.is_empty());
}

#[test]
fn tank_trap_cluster_attack_captures_visible_traps_inside_four_tiles() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 160.0, 160.0)
        .expect("tank should spawn");
    let anchor = entities
        .spawn_building(2, EntityKind::TankTrap, 256.0, 160.0, true)
        .expect("anchor should spawn");
    let inside = entities
        .spawn_building(2, EntityKind::TankTrap, 352.0, 160.0, true)
        .expect("inside trap should spawn");
    let edge = entities
        .spawn_building(2, EntityKind::TankTrap, 256.0, 288.0, true)
        .expect("edge trap should spawn");
    let outside = entities
        .spawn_building(2, EntityKind::TankTrap, 416.0, 160.0, true)
        .expect("outside trap should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::AttackTankTrapCluster {
                units: vec![tank],
                target: anchor,
                queued: false,
            },
        )],
    );

    let Order::Attack(order) = entities.get(tank).expect("tank should survive").order() else {
        panic!("cluster command should become one direct attack order");
    };
    assert_eq!(
        order.intent.target, anchor,
        "clicked trap should be attacked first"
    );
    assert_eq!(order.intent.remaining_targets, vec![inside, edge]);
    assert!(!order.intent.remaining_targets.contains(&outside));

    entities
        .get_mut(anchor)
        .expect("anchor should still exist")
        .apply_damage(u32::MAX, None);
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2], &entities, &map);
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let mut players = vec![player_state(1), player_state(2)];
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
    let mut lingering_sight = Vec::new();
    crate::game::services::death::death_system(
        &mut entities,
        &fog,
        &SmokeCloudStore::new(),
        &teams,
        &mut players,
        &mut lingering_sight,
        &mut events,
        1,
    );

    let Order::Attack(order) = entities.get(tank).expect("tank should survive").order() else {
        panic!("cluster attack should continue after the first trap dies");
    };
    assert_eq!(order.intent.target, inside);
    assert_eq!(order.intent.remaining_targets, vec![edge]);
    assert_eq!(
        entities.get(tank).and_then(|entity| entity.target_id()),
        Some(inside)
    );
}

#[test]
fn queued_tank_trap_cluster_uses_one_order_queue_slot() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 160.0, 160.0)
        .expect("tank should spawn");
    let anchor = entities
        .spawn_building(2, EntityKind::TankTrap, 256.0, 160.0, true)
        .expect("anchor should spawn");
    entities
        .spawn_building(2, EntityKind::TankTrap, 320.0, 160.0, true)
        .expect("nearby trap should spawn");

    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::AttackTankTrapCluster {
                units: vec![tank],
                target: anchor,
                queued: true,
            },
        )],
    );

    let queued = entities
        .get(tank)
        .expect("tank should survive")
        .queued_orders();
    assert_eq!(queued.len(), 1);
    assert!(matches!(
        &queued[0],
        OrderIntent::Attack(intent)
            if intent.target == anchor && intent.remaining_targets.len() == 1
    ));
}

#[test]
fn tank_trap_build_order_requires_completed_training_centre() {
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
                building: EntityKind::TankTrap,
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
        "Tank Trap should be locked before Training Centre"
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
                building: EntityKind::TankTrap,
                tile_x: 6,
                tile_y: 6,
                queued: false,
            },
        )],
    );

    assert!(
        unlocked_events.get(&1).is_none_or(Vec::is_empty),
        "valid Tank Trap command should not emit a rejection notice: {unlocked_events:?}"
    );
    assert!(matches!(
        entities.get(worker).map(|entity| entity.order()),
        Some(Order::Build(order)) if order.intent.kind == EntityKind::TankTrap
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

    assert_eq!(players[0].steel, steel_before - 30);
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
