use super::*;

const LAKE_SIZE_TILES: u32 = 15;
const CORNER_OFFSET_TILES: u32 = 20;

impl Game {
    pub fn new_scout_car_lake_reverse_l_path_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::ScoutCar {
            return Err(format!("unsupported lake reverse L-path unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported lake reverse L-path unit count {unit_count}"
            ));
        }

        let mut map = flat_dev_map(1);
        let lake_min_x = map.size / 2 - LAKE_SIZE_TILES / 2;
        let lake_min_y = map.size / 2 - LAKE_SIZE_TILES / 2;
        let lake_max_x = lake_min_x + LAKE_SIZE_TILES - 1;
        let lake_max_y = lake_min_y + LAKE_SIZE_TILES - 1;
        for ty in lake_min_y..=lake_max_y {
            for tx in lake_min_x..=lake_max_x {
                let index = map.index(tx, ty);
                map.terrain[index] = crate::protocol::terrain::WATER;
            }
        }

        let start_tile = (
            lake_max_x + CORNER_OFFSET_TILES,
            lake_min_y - CORNER_OFFSET_TILES,
        );
        let goal_tile = (
            lake_min_x - CORNER_OFFSET_TILES,
            lake_max_y + CORNER_OFFSET_TILES,
        );
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
            "dev:scout_car_lake_reverse_l_path",
        );

        DevScenarioSetup {
            game,
            player_id,
            units: vec![unit_id],
            goal,
            issue_after_ticks: config::TICK_HZ * 5,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:scout_car_lake_reverse_l_path")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::Order;
    use crate::game::services::geometry::{
        tile_rect, unit_body_for_entity, unit_body_intersects_rect,
    };

    #[test]
    fn scenario_authors_centered_lake_and_wrong_facing_cross_map_order() {
        let setup =
            Game::new_scout_car_lake_reverse_l_path_scenario(EntityKind::ScoutCar, 1, 0x5150_0724)
                .expect("scenario setup should succeed");
        let map = &setup.game.state.map;
        let lake_min_x = map.size / 2 - LAKE_SIZE_TILES / 2;
        let lake_min_y = map.size / 2 - LAKE_SIZE_TILES / 2;
        let lake_max_x = lake_min_x + LAKE_SIZE_TILES - 1;
        let lake_max_y = lake_min_y + LAKE_SIZE_TILES - 1;
        let scout = setup
            .game
            .state
            .entities
            .get(setup.units[0])
            .expect("scenario scout car should exist");
        let tile_size = config::TILE_SIZE as f32;
        let start_tile = (
            (scout.pos_x / tile_size).floor() as u32,
            (scout.pos_y / tile_size).floor() as u32,
        );
        let goal_tile = (
            (setup.goal.0 / tile_size).floor() as u32,
            (setup.goal.1 / tile_size).floor() as u32,
        );

        assert_eq!(setup.issue_after_ticks, config::TICK_HZ * 5);
        assert_eq!(
            start_tile,
            (
                lake_max_x + CORNER_OFFSET_TILES,
                lake_min_y - CORNER_OFFSET_TILES
            )
        );
        assert_eq!(
            goal_tile,
            (
                lake_min_x - CORNER_OFFSET_TILES,
                lake_max_y + CORNER_OFFSET_TILES
            )
        );
        assert!(scout.facing().abs() <= 0.001);
        assert!(matches!(scout.order(), Order::Idle));

        let water_tiles = map
            .terrain
            .iter()
            .filter(|&&terrain| terrain == crate::protocol::terrain::WATER)
            .count();
        assert_eq!(water_tiles, (LAKE_SIZE_TILES * LAKE_SIZE_TILES) as usize);
        for ty in lake_min_y..=lake_max_y {
            for tx in lake_min_x..=lake_max_x {
                assert_eq!(
                    map.terrain[map.index(tx, ty)],
                    crate::protocol::terrain::WATER
                );
            }
        }
    }

    #[test]
    fn scenario_scout_car_routes_around_lake_and_arrives() {
        let setup =
            Game::new_scout_car_lake_reverse_l_path_scenario(EntityKind::ScoutCar, 1, 0x5150_0724)
                .expect("scenario setup should succeed");
        let scout_id = setup.units[0];
        let goal = setup.goal;
        let command = setup.command();
        let mut game = setup.game;
        game.enqueue(setup.player_id, command);

        let mut arrival_tick = None;
        for tick in 1..=config::TICK_HZ * 60 {
            game.tick();
            let scout = game
                .state
                .entities
                .get(scout_id)
                .expect("scenario scout car should remain");

            let body = unit_body_for_entity(scout).expect("scout car should have a body");
            for ty in 0..game.state.map.size {
                for tx in 0..game.state.map.size {
                    let index = game.state.map.index(tx, ty);
                    if game.state.map.terrain[index] == crate::protocol::terrain::WATER {
                        assert!(
                            !unit_body_intersects_rect(body, tile_rect(tx as i32, ty as i32)),
                            "scout car body should not overlap lake tile {tx},{ty} at tick {tick}"
                        );
                    }
                }
            }

            if matches!(scout.order(), Order::Idle) && scout.path_is_empty() {
                arrival_tick = Some(tick);
                break;
            }
        }

        let arrival_tick =
            arrival_tick.expect("scout car should route around the lake and arrive within 60s");
        let scout = game
            .state
            .entities
            .get(scout_id)
            .expect("scenario scout car should remain");
        assert!(
            (scout.pos_x - goal.0).abs() <= 0.001 && (scout.pos_y - goal.1).abs() <= 0.001,
            "scout car should finish at the exact goal by tick {arrival_tick}, got ({:.3}, {:.3})",
            scout.pos_x,
            scout.pos_y
        );
    }
}
