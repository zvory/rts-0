use super::entity::{EntityKind, EntityStore};
use super::replay::{analyze_vehicle_movement_oil, ReplayStartComposition};
use super::replay_oil_analysis::VehicleOilCollector;
use super::{Game, PlayerInit};
use crate::protocol::DEFAULT_FACTION_ID;

fn replay_players() -> [PlayerInit; 1] {
    [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: DEFAULT_FACTION_ID.to_string(),
        name: "Replay oil".to_string(),
        color: "#fff".to_string(),
        is_ai: false,
    }]
}

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

#[test]
fn analyzer_accepts_a_zero_tick_replay_without_advancing_it() {
    let game = Game::new(&replay_players(), 7);
    let start = ReplayStartComposition::capture(&game, "test-sha").expect("replay start");
    let artifact = start.finalize(&game, None, game.scores());

    let records = analyze_vehicle_movement_oil(&artifact).expect("zero-tick replay analysis");

    assert!(records.iter().all(|record| record.last_seen_tick == 0));
}

#[test]
fn analyzer_rejects_a_start_checkpoint_after_the_declared_end() {
    let mut game = Game::new(&replay_players(), 7);
    game.tick();
    let start = ReplayStartComposition::capture(&game, "test-sha").expect("replay start");
    let mut artifact = start.finalize(&game, None, game.scores());
    artifact.duration_ticks = 0;

    let error = analyze_vehicle_movement_oil(&artifact).expect_err("invalid replay length");

    assert!(error.contains("start tick 1 is beyond replay length 0"));
}
