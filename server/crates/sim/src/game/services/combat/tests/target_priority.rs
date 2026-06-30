use super::*;

#[test]
fn tank_prefers_nearby_unit_over_armored_command_center() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    let city_centre = entities
        .spawn_building(2, EntityKind::CityCentre, 160.0, 100.0, true)
        .expect("city centre should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 100.0, 180.0)
        .expect("worker should spawn");
    entities
        .get_mut(tank)
        .expect("tank should exist")
        .set_order(Order::attack_move_to(300.0, 100.0));

    let target = resolve_tank_test_target(&map, &entities, &default_team_relations(), tank);

    assert_eq!(target, Some(worker));
    assert_ne!(target, Some(city_centre));
}
