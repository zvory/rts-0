use super::*;
use crate::game::entity::PanzerfaustState;

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

#[test]
fn spent_panzerfaust_carrier_uses_rifle_target_priority() {
    let map = open_map(12);
    let mut entities = EntityStore::new();
    let panzerfaust = entities
        .spawn_unit(1, EntityKind::Panzerfaust, 100.0, 100.0)
        .expect("Panzerfaust should spawn");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 140.0, 100.0)
        .expect("Tank should spawn");
    let rifleman = entities
        .spawn_unit(2, EntityKind::Rifleman, 180.0, 100.0)
        .expect("Rifleman should spawn");
    if let Some(attacker) = entities.get_mut(panzerfaust) {
        attacker.set_order(Order::attack_move_to(300.0, 100.0));
        let combat = attacker
            .combat
            .as_mut()
            .expect("Panzerfaust should have combat state");
        combat.panzerfaust = Some(PanzerfaustState::Recovery {
            target: tank,
            ticks_remaining: 30,
        });
    }

    let target = resolve_test_target(
        &map,
        &entities,
        &default_team_relations(),
        panzerfaust,
        300.0,
    );

    assert_eq!(target, Some(rifleman));
}
