use super::*;

#[test]
fn point_tanks_rotate_at_normal_rate_without_translation() {
    let map = flat_map(2);
    let mut entities = EntityStore::new();
    let a = entities
        .spawn_unit(1, EntityKind::Tank, 500.0, 500.0)
        .unwrap();
    let b = entities
        .spawn_unit(1, EntityKind::Tank, 700.0, 500.0)
        .unwrap();
    let desired = std::f32::consts::FRAC_PI_2;
    for (id, start) in [(a, 0.0), (b, std::f32::consts::PI)] {
        let tank = entities.get_mut(id).unwrap();
        tank.set_facing(start);
        tank.set_order(Order::Point { facing: desired });
    }
    let occ = Occupancy::build(&map, &entities);
    super::super::tank_point::advance(&map, &mut entities, &occ);
    assert!((entities.get(a).unwrap().facing() - TANK_BODY_TURN_RATE_RAD_PER_TICK).abs() < 0.0001);
    for _ in 0..100 {
        super::super::tank_point::advance(&map, &mut entities, &occ);
    }
    for (id, x) in [(a, 500.0), (b, 700.0)] {
        let tank = entities.get(id).unwrap();
        assert!((tank.facing() - desired).abs() < 0.0001);
        assert_eq!((tank.pos_x, tank.pos_y), (x, 500.0));
        assert!(matches!(tank.order(), Order::Point { .. }));
    }
}

#[test]
fn tank_plus_point_turn_is_fifty_percent_faster() {
    let map = flat_map(2);
    let mut entities = EntityStore::new();
    let id = entities.spawn_unit(1, EntityKind::Tank, 500.0, 500.0).unwrap();
    let tank = entities.get_mut(id).unwrap();
    tank.combat.as_mut().unwrap().tank_plus = true;
    tank.set_order(Order::Point { facing: std::f32::consts::FRAC_PI_2 });
    let occ = Occupancy::build(&map, &entities);
    super::super::tank_point::advance(&map, &mut entities, &occ);
    assert!((entities.get(id).unwrap().facing() - TANK_BODY_TURN_RATE_RAD_PER_TICK * 1.5).abs() < 0.0001);
}
