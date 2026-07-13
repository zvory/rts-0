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
