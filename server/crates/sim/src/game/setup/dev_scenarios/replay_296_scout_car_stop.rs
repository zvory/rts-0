use super::*;

const REPLAY_SEED: u32 = 0x6cf2_d1b2;
const REPLAY_TICK: u32 = 8_242;
const GOAL: (f32, f32) = (48.0, 1_552.0);

impl Game {
    pub fn new_replay_296_scout_car_stop_scenario(
        unit: EntityKind,
        unit_count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::ScoutCar || unit_count != 5 {
            return Err(format!(
                "replay-296 Scout Car stop requires five Scout Cars, got {unit_count} {unit}"
            ));
        }

        let map = Map::load("Schone Tage", 2, REPLAY_SEED)
            .map_err(|error| format!("failed to load replay-296 map: {error}"))?;
        let mut entities = EntityStore::new();
        let player_base = map.tile_center(map.starts[0].0, map.starts[0].1);
        let opponent_base = map.tile_center(map.starts[1].0, map.starts[1].1);
        for (owner, (x, y)) in [(1, player_base), (2, opponent_base)] {
            entities
                .spawn_building(owner, EntityKind::ResourceDepot, x, y, true)
                .ok_or_else(|| format!("failed to spawn replay-296 player {owner} base"))?;
        }
        let specs = [
            (273.12808, 2_106.571, -2.014_803),
            (272.43066, 1_892.032_8, -2.031_696_6),
            // The current Schone Tage revision added forest over these two recorded hull
            // positions. Shift them one tile east while preserving the replay's relative lanes
            // closely enough to keep this a stable west-edge formation regression.
            (253.18274, 1_914.671_3, -2.023_099_7),
            (251.92749, 1_975.598_1, -2.038_910_2),
            (286.04916, 2_009.069, -1.991_067_9),
        ];
        let mut units = Vec::with_capacity(specs.len());
        for (x, y, facing) in specs {
            let id = entities
                .spawn_unit(1, EntityKind::ScoutCar, x, y)
                .ok_or_else(|| "failed to spawn replay-296 Scout Car".to_string())?;
            if let Some(entity) = entities.get_mut(id) {
                entity.set_facing(facing);
            }
            units.push(id);
        }

        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(1, 1), (2, 2)],
            1,
            (8, 61),
            REPLAY_SEED,
            "dev:replay_296_scout_car_stop",
        );
        game.state.tick = REPLAY_TICK;
        game.state.rng = TrackedRng::seed_from_match_seed(REPLAY_SEED);
        game.state.ground_decals.begin_tick(REPLAY_TICK);
        if let Some(player) = game.state.players.iter_mut().find(|player| player.id == 1) {
            player.set_resources(1_213, 39);
        }

        DevScenarioSetup {
            game,
            player_id: 1,
            units,
            goal: GOAL,
            issue_after_ticks: REPLAY_TICK,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:replay_296_scout_car_stop")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_geometry_routes_all_five_scout_cars_after_edge_fix() {
        let mut setup =
            Game::new_replay_296_scout_car_stop_scenario(EntityKind::ScoutCar, 5, REPLAY_SEED)
                .expect("scenario");
        setup.game.enqueue(setup.player_id, setup.command());
        let start = setup
            .units
            .iter()
            .map(|id| {
                let entity = setup.game.state.entities.get(*id).expect("Scout Car");
                (entity.pos_x, entity.pos_y)
            })
            .collect::<Vec<_>>();
        for _ in 0..30 {
            setup.game.tick();
        }
        let snapshot = setup.game.snapshot_full_for_with_options(
            setup.player_id,
            SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: true,
            },
        );
        let moved = setup
            .units
            .iter()
            .enumerate()
            .map(|(index, id)| {
                let entity = snapshot
                    .entities
                    .iter()
                    .find(|entity| entity.id == *id)
                    .expect("Scout Car snapshot");
                (entity.x - start[index].0).hypot(entity.y - start[index].1)
            })
            .collect::<Vec<_>>();
        assert!(
            moved.iter().all(|distance| *distance > 1.0),
            "every Scout Car should make progress, got {moved:?}"
        );
        for id in &setup.units {
            let entity = snapshot
                .entities
                .iter()
                .find(|entity| entity.id == *id)
                .expect("Scout Car snapshot");
            assert_eq!(entity.state, "move");
            assert!(
                entity.debug_path.is_some(),
                "every moving car must have a route"
            );
        }
        assert_eq!(
            snapshot.oil, 39,
            "replay oil is present but irrelevant to movement"
        );
    }

    #[test]
    fn moving_the_group_one_tile_inward_gives_every_scout_car_a_route() {
        let mut setup =
            Game::new_replay_296_scout_car_stop_scenario(EntityKind::ScoutCar, 5, REPLAY_SEED)
                .expect("scenario");
        setup.goal = (80.0, GOAL.1);
        setup.game.enqueue(setup.player_id, setup.command());
        for _ in 0..30 {
            setup.game.tick();
        }
        let snapshot = setup.game.snapshot_full_for_with_options(
            setup.player_id,
            SnapshotOptions {
                include_movement_paths: true,
                movement_paths_for_all_projected: true,
            },
        );
        assert!(setup.units.iter().all(|id| snapshot
            .entities
            .iter()
            .find(|entity| entity.id == *id)
            .and_then(|entity| entity.debug_path.as_ref())
            .is_some()));
    }
}
