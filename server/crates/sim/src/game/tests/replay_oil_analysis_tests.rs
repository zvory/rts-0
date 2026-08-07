use super::entity::{EntityKind, EntityStore};
use super::replay_oil_analysis::VehicleOilCollector;

#[test]
fn collector_retains_dead_vehicles_and_all_fuel_kinds() {
    let mut entities = EntityStore::new();
    let scout = entities
        .spawn_unit(1, EntityKind::ScoutCar, 64.0, 64.0)
        .expect("scout car");
    let command = entities
        .spawn_unit(1, EntityKind::CommandCar, 96.0, 64.0)
        .expect("command car");
    let tank = entities
        .spawn_unit(2, EntityKind::Tank, 128.0, 64.0)
        .expect("tank");
    entities
        .spawn_unit(2, EntityKind::Rifleman, 160.0, 64.0)
        .expect("rifleman");
    for (id, oil) in [(scout, 1.25), (command, 2.5), (tank, 7.75)] {
        entities
            .get_mut(id)
            .and_then(|entity| entity.movement.as_mut())
            .expect("vehicle movement")
            .lifetime_oil_used = oil;
    }

    let mut collector = VehicleOilCollector::default();
    collector.observe(20, &entities);
    entities.remove(command);
    collector.observe(21, &entities);
    let records = collector.finish(&entities);

    assert_eq!(records.len(), 3);
    assert_eq!(records[0].unit_kind, "scout_car");
    assert_eq!(records[1].unit_kind, "command_car");
    assert_eq!(records[2].unit_kind, "tank");
    assert!(records[0].survived_to_end);
    assert!(!records[1].survived_to_end);
    assert!(records[2].survived_to_end);
    assert_eq!(records[1].last_seen_tick, 20);
    assert_eq!(records[1].lifetime_oil_spend, 2.5);
}
