use super::*;

impl Game {
    pub fn new_scout_car_open_ground_l_path_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::ScoutCar {
            return Err(format!("unsupported open-ground L-path unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported open-ground L-path unit count {unit_count}"
            ));
        }

        let mut map = flat_dev_map(1);
        let start_tile = (56, 32);
        let goal_tile = (25, 51);
        let start = map.tile_center(start_tile.0, start_tile.1);
        let goal = map.tile_center(goal_tile.0, goal_tile.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }

        let mut entities = EntityStore::new();
        let unit_id = entities
            .spawn_unit(1, unit, start.0, start.1)
            .ok_or_else(|| "failed to spawn scout car".to_string())?;
        if let Some(entity) = entities.get_mut(unit_id) {
            entity.set_facing(0.0);
        }

        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:scout_car_open_ground_l_path",
        );

        DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
            issue_after_ticks: config::TICK_HZ * 5,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:scout_car_open_ground_l_path")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::Order;

    #[test]
    fn scenario_preserves_open_ground_l_path_reproduction_setup() {
        let setup =
            Game::new_scout_car_open_ground_l_path_scenario(EntityKind::ScoutCar, 1, 0x5150_0724)
                .expect("scenario setup should succeed");
        let scout_id = setup.units[0];
        let scout = setup
            .game
            .state
            .entities
            .get(scout_id)
            .expect("scenario scout car should exist");
        let start = (scout.pos_x, scout.pos_y);

        assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 5);
        assert!(
            setup.goal.0 < start.0 && setup.goal.1 > start.1,
            "goal should be southwest of the east-facing scout car"
        );
        assert!(
            (start.0 - setup.goal.0 - config::TILE_SIZE as f32 * 31.0).abs() <= 0.001,
            "scenario should preserve the unequal 31-tile west component"
        );
        assert!(
            (setup.goal.1 - start.1 - config::TILE_SIZE as f32 * 19.0).abs() <= 0.001,
            "scenario should preserve the unequal 19-tile south component"
        );
        assert!(
            scout.facing().abs() <= 0.001,
            "scout car should begin facing east"
        );
        assert!(matches!(scout.order(), Order::Idle));

        match setup.command() {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => {
                assert_eq!(units, vec![scout_id]);
                assert!((x - setup.goal.0).abs() <= 0.001);
                assert!((y - setup.goal.1).abs() <= 0.001);
                assert!(!queued);
            }
            command => panic!("scenario should issue a move command, got {command:?}"),
        }
    }

    #[test]
    #[ignore = "known bug: Scout Car follows a cardinal-first L path on open ground"]
    fn scenario_scout_car_begins_one_continuous_turn_toward_goal() {
        let setup =
            Game::new_scout_car_open_ground_l_path_scenario(EntityKind::ScoutCar, 1, 0x5150_0724)
                .expect("scenario setup should succeed");
        let scout_id = setup.units[0];
        let start = setup
            .game
            .state
            .entities
            .get(scout_id)
            .map(|scout| (scout.pos_x, scout.pos_y))
            .expect("scenario scout car should exist");
        let command = setup.command();
        let mut game = setup.game;
        game.enqueue(setup.player_id, command);

        for _ in 0..120 {
            game.tick();
        }

        let scout = game
            .state
            .entities
            .get(scout_id)
            .expect("scenario scout car should remain alive");
        assert!(
            scout.pos_x < start.0 - config::TILE_SIZE as f32,
            "scout car should make westward progress toward the goal"
        );
        assert!(
            scout.pos_y > start.1 + config::TILE_SIZE as f32,
            "scout car should turn south instead of reversing along a cardinal west leg"
        );
    }
}
