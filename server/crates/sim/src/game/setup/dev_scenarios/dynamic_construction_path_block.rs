use super::*;

impl Game {
    pub fn new_dynamic_construction_path_block_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Worker || unit_count != 1 {
            return Err(format!(
                "unsupported dynamic-construction path-block launch {unit} x{unit_count}"
            ));
        }

        let mut map = flat_dev_map(1);
        let center_y = map.size / 2;
        let start_tile = (map.size / 2 - 20, center_y);
        let mover_start = map.tile_center(start_tile.0, start_tile.1);
        let goal = map.tile_center(start_tile.0 + 20, start_tile.1);
        let build_tile = (start_tile.0 + 9, center_y - 1);
        let builder_start = map.tile_center(build_tile.0 + 1, build_tile.1 - 1);
        let city_centre_pos = map.tile_center(start_tile.0, center_y - 8);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }

        let mut entities = EntityStore::new();
        entities
            .spawn_building(
                1,
                EntityKind::CityCentre,
                city_centre_pos.0,
                city_centre_pos.1,
                true,
            )
            .ok_or_else(|| "failed to spawn prerequisite City Centre".to_string())?;
        let mover = entities
            .spawn_unit(1, EntityKind::Worker, mover_start.0, mover_start.1)
            .ok_or_else(|| "failed to spawn moving worker".to_string())?;
        let builder = entities
            .spawn_unit(1, EntityKind::Worker, builder_start.0, builder_start.1)
            .ok_or_else(|| "failed to spawn building worker".to_string())?;

        let player_id = 1;
        let mut game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:dynamic_construction_path_block",
        );
        if let Some(player) = game.state.players.iter_mut().find(|p| p.id == player_id) {
            player.refund_resources(1_000, 0);
        }
        if let Some(loadout) = game
            .state
            .starting_loadouts
            .iter_mut()
            .find(|loadout| loadout.player_id == player_id)
        {
            loadout.starting_steel = 1_000;
        }
        // The watcher driver adds the mover's order on tick zero. Keeping the build queued in the
        // checkpoint makes both orders authoritative in the same tick: pathing sees open ground,
        // then construction materializes a Barracks directly across that already-issued route.
        game.enqueue(
            player_id,
            crate::game::command::SimCommand::Build {
                units: vec![builder],
                building: EntityKind::Barracks,
                tile_x: build_tile.0,
                tile_y: build_tile.1,
                queued: false,
            },
        );

        DevScenarioSetup {
            game,
            player_id,
            units: vec![mover],
            goal,
            issue_after_ticks: 0,
        }
        .checkpoint_backed("dev:dynamic_construction_path_block")
    }
}
