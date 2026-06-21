use super::*;

#[test]
fn vehicle_body_auto_acquisition_prefers_soft_target_over_irrelevant_tank_trap() {
    let mut entities = EntityStore::new();
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
        .expect("scout car should spawn");
    if let Some(scout) = entities.get_mut(scout) {
        scout.set_order(Order::attack_move_to(300.0, 100.0));
        scout.set_path_goal(Some((300.0, 100.0)));
    }
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 160.0, true)
        .expect("irrelevant tank trap should spawn");
    let worker = entities
        .spawn_unit(2, EntityKind::Worker, 180.0, 100.0)
        .expect("worker should spawn");

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &open_map(12),
    );

    let target = entities
        .get(scout)
        .expect("scout car should exist")
        .target_id();
    assert_eq!(
        target,
        Some(worker),
        "vehicle should not waste priority on a Tank Trap away from its route"
    );
    assert_ne!(target, Some(trap));
}

#[test]
fn vehicle_body_auto_acquisition_prioritizes_obstructing_tank_trap_over_soft_target() {
    let mut entities = EntityStore::new();
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, 100.0, 100.0)
        .expect("scout car should spawn");
    if let Some(scout) = entities.get_mut(scout) {
        scout.set_order(Order::attack_move_to(300.0, 100.0));
        scout.set_path_goal(Some((300.0, 100.0)));
    }
    let trap = entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 100.0, true)
        .expect("obstructing tank trap should spawn");
    entities
        .spawn_unit(2, EntityKind::Worker, 180.0, 130.0)
        .expect("worker should spawn");

    run_combat_tick_on_map(
        &mut entities,
        &[player_state(1, false), player_state(2, false)],
        &open_map(12),
    );

    assert_eq!(
        entities
            .get(scout)
            .expect("scout car should exist")
            .target_id(),
        Some(trap),
        "vehicle should breach a Tank Trap that is on its route"
    );
}

#[test]
fn tank_prioritizes_anti_tank_gun_over_irrelevant_nearby_tank_trap() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let tank = entities
        .spawn_unit(1, EntityKind::Tank, 100.0, 100.0)
        .expect("tank should spawn");
    if let Some(tank) = entities.get_mut(tank) {
        tank.set_order(Order::attack_move_to(300.0, 100.0));
        tank.set_path_goal(Some((300.0, 100.0)));
    }
    entities
        .spawn_building(2, EntityKind::TankTrap, 150.0, 160.0, true)
        .expect("irrelevant tank trap should spawn");
    let anti_tank_gun = entities
        .spawn_unit(2, EntityKind::AntiTankGun, 180.0, 100.0)
        .expect("anti-tank gun should spawn");

    assert_eq!(
        resolve_tank_test_target(&map, &entities, &default_team_relations(), tank),
        Some(anti_tank_gun)
    );
}
