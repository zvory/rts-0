use super::*;

#[test]
fn point_tanks_share_centroid_heading_and_filter_ownership_kind_and_duplicates() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let a = entities
        .spawn_unit(1, EntityKind::Tank, 400.0, 400.0)
        .unwrap();
    let b = entities
        .spawn_unit(1, EntityKind::Tank, 600.0, 400.0)
        .unwrap();
    let enemy = entities
        .spawn_unit(2, EntityKind::Tank, 900.0, 600.0)
        .unwrap();
    let worker = entities
        .spawn_unit(1, EntityKind::Worker, 300.0, 600.0)
        .unwrap();
    for id in [a, b] {
        let tank = entities.get_mut(id).unwrap();
        tank.set_order(Order::move_to(1000.0, 1000.0));
        tank.append_queued_order(OrderIntent::move_to(1200.0, 1200.0));
    }
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::PointTanks {
                queued: false,
                units: vec![a, a, b, enemy, worker, u32::MAX],
                x: 700.0,
                y: 600.0,
            },
        )],
    );
    for id in [a, b] {
        let tank = entities.get(id).unwrap();
        let Order::Point { facing } = tank.order() else {
            panic!("expected Point");
        };
        assert!((facing - std::f32::consts::FRAC_PI_4).abs() < 0.0001);
        assert!(tank.queued_orders().is_empty());
        assert!(tank.path_is_empty());
    }
    for id in [enemy, worker] {
        assert!(matches!(entities.get(id).unwrap().order(), Order::Idle));
    }
}

#[test]
fn point_tanks_invalid_and_centroid_targets_preserve_orders() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    entities
        .get_mut(id)
        .unwrap()
        .set_order(Order::move_to(1000.0, 1000.0));
    for (x, y) in [(500.0, 500.0), (f32::NAN, 2.0), (2.0, f32::INFINITY)] {
        apply(
            &map,
            &mut entities,
            vec![(
                1,
                SimCommand::PointTanks {
                    queued: false,
                    units: vec![id],
                    x,
                    y,
                },
            )],
        );
        assert!(matches!(entities.get(id).unwrap().order(), Order::Move(_)));
    }
}

#[test]
fn point_tanks_respects_raw_list_cap() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::PointTanks {
                queued: false,
                units: vec![id; MAX_UNITS_PER_COMMAND + 1],
                x: 600.0,
                y: 600.0,
            },
        )],
    );
    assert!(matches!(entities.get(id).unwrap().order(), Order::Idle));
}

#[test]
fn queued_point_tanks_preserves_active_order_and_queue_and_respects_capacity() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    let tank = entities.get_mut(id).unwrap();
    tank.set_order(Order::move_to(800.0, 500.0));
    tank.append_queued_order(OrderIntent::move_to(900.0, 500.0));
    let command = SimCommand::PointTanks {
        units: vec![id],
        x: 500.0,
        y: 900.0,
        queued: true,
    };
    apply(&map, &mut entities, vec![(1, command.clone())]);
    let tank = entities.get(id).unwrap();
    assert!(matches!(tank.order(), Order::Move(_)));
    assert_eq!(
        tank.queued_orders(),
        &[
            OrderIntent::move_to(900.0, 500.0),
            OrderIntent::Point {
                facing: std::f32::consts::FRAC_PI_2
            }
        ]
    );
    let saved = serde_json::to_string(tank.queued_orders()).unwrap();
    let restored: Vec<OrderIntent> = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored, tank.queued_orders());
    for _ in 0..20 {
        apply(&map, &mut entities, vec![(1, command.clone())]);
    }
    assert_eq!(
        entities.get(id).unwrap().queued_orders().len(),
        crate::game::entity::MAX_QUEUED_ORDERS
    );
}

#[test]
fn queued_move_does_not_interrupt_an_active_point_turn() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    entities.get_mut(id).unwrap().set_order(Order::Point {
        facing: std::f32::consts::FRAC_PI_2,
    });
    apply(
        &map,
        &mut entities,
        vec![(
            1,
            SimCommand::Move {
                units: vec![id],
                x: 900.0,
                y: 500.0,
                queued: true,
            },
        )],
    );
    let tank = entities.get(id).unwrap();
    assert!(matches!(tank.order(), Order::Point { .. }));
    assert_eq!(tank.queued_orders(), &[OrderIntent::move_to(900.0, 500.0)]);
}
