use super::*;

#[derive(Clone, Copy)]
enum DynamicConstructionPathBlockCase {
    HeadOn,
    SlightAngle,
    MajorAngle,
}

impl DynamicConstructionPathBlockCase {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "head_on" => Some(Self::HeadOn),
            "slight_angle" => Some(Self::SlightAngle),
            "major_angle" => Some(Self::MajorAngle),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::HeadOn => "head_on",
            Self::SlightAngle => "slight_angle",
            Self::MajorAngle => "major_angle",
        }
    }
}

impl Game {
    pub fn new_dynamic_construction_path_block_scenario(
        scenario_case: Option<&str>,
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Worker || unit_count != 1 {
            return Err(format!(
                "unsupported dynamic-construction path-block launch {unit} x{unit_count}"
            ));
        }
        let scenario_case = scenario_case
            .and_then(DynamicConstructionPathBlockCase::parse)
            .ok_or_else(|| {
                "missing or unsupported dynamic-construction path-block case".to_string()
            })?;

        let mut map = flat_dev_map(1);
        let center_y = map.size / 2;
        let start_x = map.size / 2 - 20;
        let goal = map.tile_center(start_x + 20, center_y);
        let (start_tile, mover_start, build_tile) = match scenario_case {
            DynamicConstructionPathBlockCase::HeadOn => {
                let tile = (start_x, center_y);
                (
                    tile,
                    map.tile_center(tile.0, tile.1),
                    (start_x + 6, center_y - 1),
                )
            }
            DynamicConstructionPathBlockCase::SlightAngle => {
                let tile = (start_x, center_y);
                let mut start = map.tile_center(tile.0, tile.1);
                start.1 -= 4.0;
                (tile, start, (start_x + 6, center_y - 1))
            }
            DynamicConstructionPathBlockCase::MajorAngle => {
                let tile = (start_x, center_y - 6);
                (
                    tile,
                    map.tile_center(tile.0, tile.1),
                    (start_x + 6, center_y - 5),
                )
            }
        };
        let builder_start = map.tile_center(build_tile.0 + 1, build_tile.1 - 1);
        let city_centre_pos = map.tile_center(start_x, center_y + 10);
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
            &format!("dev:dynamic_construction_path_block:{}", scenario_case.id()),
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
            attack_move: false,
        }
        .checkpoint_backed(&format!(
            "dev:dynamic_construction_path_block:{}",
            scenario_case.id()
        ))
    }
}
