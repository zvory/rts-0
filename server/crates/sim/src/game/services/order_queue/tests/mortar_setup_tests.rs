use super::*;

#[test]
fn queued_mortar_setup_clears_defensive_later_orders_and_keeps_arrival_facing() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let mortar = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    {
        let unit = entities.get_mut(mortar).expect("mortar should exist");
        unit.set_facing(std::f32::consts::FRAC_PI_2);
        unit.append_queued_order(OrderIntent::setup_anti_tank_guns(0.0, 0.0));
        unit.append_queued_order(OrderIntent::attack_move_to(240.0, 100.0));
    }

    promote(&map, &mut entities);

    let unit = entities.get(mortar).expect("mortar should exist");
    assert_eq!(unit.queued_orders().len(), 0);
    assert!(
        (unit.emplacement_facing().unwrap_or_default() - std::f32::consts::FRAC_PI_2).abs() < 0.001,
        "mortar setup should keep the facing it had when the stage promoted"
    );
}
