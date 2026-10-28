use super::*;

#[test]
fn strategy_receives_cumulative_loss_summaries_on_first_and_later_decisions() {
    struct InspectLosses;
    impl AiStrategy for InspectLosses {
        fn initialize(&mut self, frame: &AiFrame) {
            assert_eq!(frame.own_losses().units_lost(), 3);
        }
        fn step(&mut self, frame: &AiFrame, _: &mut AiActions) {
            assert_eq!(frame.own_losses().units_lost(), 3);
            assert_eq!(frame.own_losses().buildings_lost(), 1);
            assert_eq!(
                frame.own_losses().units[&rts_rules::EntityKind::Rifleman],
                3
            );
            let enemy = &frame.observed_enemy_losses()[&2];
            assert_eq!(enemy.units_lost(), 2);
            assert_eq!(enemy.buildings_lost(), 0);
            let (steel, oil) = rts_rules::economy::cost(rts_rules::EntityKind::Tank);
            assert_eq!((enemy.steel_value, enemy.oil_value), (steel * 2, oil * 2));
            assert!(!frame.observed_enemy_losses().contains_key(&1));
            assert!(!frame.observed_enemy_losses().contains_key(&999));
        }
    }
    let game = Game::new_without_ai_controllers(&super::tests::driver_test_players(), 13);
    let start = game.start_payload();
    let mut snapshot = game.snapshot_for(1);
    let mut controller = AiController::with_strategy(1, Box::new(InspectLosses));
    for tick in [7, 8, 9, 17] {
        snapshot.tick = tick;
        controller.think(AiThinkContext {
            match_seed: game.seed(),
            start: &start,
            snapshot: &snapshot,
            alive_player_ids: &[1, 2],
            retreat_commands: Vec::new(),
            known_losses: BTreeMap::from([
                (
                    1,
                    BTreeMap::from([
                        (rts_rules::EntityKind::Rifleman, 3),
                        (rts_rules::EntityKind::Depot, 1),
                    ]),
                ),
                (2, BTreeMap::from([(rts_rules::EntityKind::Tank, 2)])),
                (999, BTreeMap::from([(rts_rules::EntityKind::Tank, 99)])),
            ]),
        });
    }
    assert!(controller.strategy_initialized);
}
