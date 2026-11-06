use super::*;

#[test]
fn idle_unit_promotes_first_queued_move() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    entities
        .get_mut(unit)
        .expect("unit should exist")
        .append_queued_order(OrderIntent::move_to(180.0, 100.0));

    promote(&map, &mut entities);

    let entity = entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::Move(_)));
    assert_eq!(entity.move_phase(), Some(MovePhase::AwaitingPath));
    assert!(entity.queued_orders().is_empty());
}

#[test]
fn queued_hold_position_promotes_after_the_active_move() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let unit = entities
        .spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0)
        .expect("rifleman should spawn");
    {
        let entity = entities.get_mut(unit).expect("unit should exist");
        entity.set_order(Order::move_to(200.0, 100.0));
        entity.mark_move_phase(MovePhase::Arrived);
        entity.append_queued_order(OrderIntent::hold_position());
    }

    promote(&map, &mut entities);

    let entity = entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::HoldPosition));
    assert!(entity.queued_orders().is_empty());
    assert!(entity.path_is_empty());
}

#[test]
fn queued_tank_point_waits_for_move_then_alignment_before_follow_up() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let id = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    let facing = std::f32::consts::FRAC_PI_2;
    let tank = entities.get_mut(id).unwrap();
    tank.set_facing(0.0);
    tank.set_order(Order::move_to(700.0, 500.0));
    tank.append_queued_order(OrderIntent::Point { facing });
    tank.append_queued_order(OrderIntent::move_to(900.0, 500.0));
    promote(&map, &mut entities);
    assert!(matches!(entities.get(id).unwrap().order(), Order::Move(_)));
    entities
        .get_mut(id)
        .unwrap()
        .mark_move_phase(MovePhase::Arrived);
    promote(&map, &mut entities);
    assert!(matches!(
        entities.get(id).unwrap().order(),
        Order::Point { .. }
    ));
    assert_eq!(entities.get(id).unwrap().queued_orders().len(), 1);
    promote(&map, &mut entities);
    assert!(matches!(
        entities.get(id).unwrap().order(),
        Order::Point { .. }
    ));
    entities.get_mut(id).unwrap().set_facing(facing);
    promote(&map, &mut entities);
    let tank = entities.get(id).unwrap();
    assert!(matches!(tank.order(), Order::Move(_)));
    assert_eq!(tank.move_intent(), Some((900.0, 500.0)));
    assert!(tank.queued_orders().is_empty());
}
