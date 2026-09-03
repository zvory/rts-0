use super::*;
use crate::game::entity::Entity;

const REPLAY_SEED: u32 = 772_415_158;
const REPLAY_START_TICK: u32 = 15_275;
const NEXT_ENTITY_ID: u32 = 412;
const ALEX: u32 = 5;
const SOUPMAN: u32 = 6;
const SCOUT_CAR: u32 = 403;
const GROUP: [u32; 7] = [411, 322, 338, 365, SCOUT_CAR, 396, 379];
const ORDERS: [(u32, (f32, f32)); 6] = [
    (15_283, (2_391.229_5, 4_210.128_4)),
    (15_290, (2_593.229_5, 4_194.128_4)),
    (15_294, (2_625.229_5, 4_195.128_4)),
    (15_299, (2_663.229_5, 4_146.128_4)),
    (15_303, (2_719.229_5, 4_110.128_4)),
    (15_308, (2_843.229_5, 4_106.128_4)),
];

impl Game {
    pub fn new_replay_303_scout_car_forest_lock_scenario(
        unit: EntityKind,
        unit_count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::ScoutCar || unit_count != 1 {
            return Err(format!(
                "replay-303 forest lock requires one highlighted Scout Car, got {unit_count} {unit}"
            ));
        }

        let map = replay_forest_patch()?;
        let replay_entities: Vec<Entity> = serde_json::from_str(include_str!(
            "fixtures/replay_303_tick_15275_group_entities.json"
        ))
        .map_err(|error| format!("invalid replay-303 group fixture: {error}"))?;
        let entities = EntityStore::from_checkpoint_entities(NEXT_ENTITY_ID, replay_entities);
        if let Some(missing) = GROUP.iter().find(|id| !entities.contains(**id)) {
            return Err(format!(
                "replay-303 group fixture is missing actor {missing}"
            ));
        }

        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            [(ALEX, 1), (SOUPMAN, 2)],
            SOUPMAN,
            (67, 124),
            REPLAY_SEED,
            "dev:replay_303_scout_car_forest_lock",
        );
        game.state.tick = REPLAY_START_TICK;
        game.state.rng = TrackedRng::seed_from_match_seed(REPLAY_SEED);
        game.state.ground_decals.begin_tick(REPLAY_START_TICK);

        DevScenarioSetup {
            game,
            player_id: SOUPMAN,
            units: GROUP.to_vec(),
            goal: ORDERS[0].1,
            issue_after_ticks: ORDERS[0].0,
            order: DevScenarioOrder::MoveSequence(&ORDERS),
        }
        .checkpoint_backed("dev:replay_303_scout_car_forest_lock")
    }
}

fn replay_forest_patch() -> Result<Map, String> {
    let mut map = Map::load("Schone Tage", 2, REPLAY_SEED)
        .map_err(|error| format!("failed to load replay-303 map: {error}"))?;
    map.terrain.fill(crate::protocol::terrain::GRASS);
    map.elevation.fill(0);
    map.sun = None;
    map.base_sites.clear();
    map.base_resource_counts.clear();

    let is_replay_forest =
        |&(x, y): &(u32, u32)| matches!((y, x), (119 | 123, 67..=69) | (120..=122, 66..=70));
    map.concealment_tiles.retain(is_replay_forest);
    map.no_vehicle_tiles.retain(is_replay_forest);
    map.no_building_tiles.retain(is_replay_forest);
    map.no_entrenchment_tiles.retain(is_replay_forest);
    map.damage_reduction_tiles.retain(is_replay_forest);
    map.slow_movement_tiles.retain(is_replay_forest);
    map.doodads.retain(|doodad| {
        (2_080..=2_272).contains(&doodad.x) && (3_776..=4_000).contains(&doodad.y)
    });
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduced_replay_group_recovers_from_scout_car_forest_lock() {
        let mut setup = Game::new_replay_303_scout_car_forest_lock_scenario(
            EntityKind::ScoutCar,
            1,
            REPLAY_SEED,
        )
        .expect("scenario");
        assert_eq!(setup.game.tick_count(), REPLAY_START_TICK);
        assert_eq!(setup.game.state.entities.iter().count(), GROUP.len());
        assert_eq!(setup.game.state.map.no_vehicle_tiles.len(), 21);

        let schedule = setup.scheduled_commands();
        let mut next_command = 0;
        let mut former_lock_position = None;
        let tank_start = position(&setup.game, 322);
        while setup.game.tick_count() < 15_360 {
            while schedule
                .get(next_command)
                .is_some_and(|(tick, _)| *tick == setup.game.tick_count())
            {
                setup
                    .game
                    .enqueue(setup.player_id, schedule[next_command].1.clone());
                next_command += 1;
            }
            setup.game.tick();
            if setup.game.tick_count() == 15_290 {
                former_lock_position = Some(position(&setup.game, SCOUT_CAR));
            }
        }

        let former_lock = former_lock_position.expect("former lock tick");
        assert_eq!(former_lock.0.to_bits(), 2_148.724_6_f32.to_bits());
        assert_eq!(former_lock.1.to_bits(), 3_983.101_6_f32.to_bits());
        assert!(distance(former_lock, position(&setup.game, SCOUT_CAR)) > 10.0);
        assert!(distance(tank_start, position(&setup.game, 322)) > 10.0);
        let scout = setup.game.state.entities.get(SCOUT_CAR).expect("Scout Car");
        assert!(matches!(
            scout.movement.as_ref().map(|movement| &movement.order),
            Some(crate::game::entity::Order::Move(_))
        ));
        assert!(!matches!(
            scout.move_phase(),
            Some(crate::game::entity::MovePhase::PathFailed)
        ));
    }

    fn position(game: &Game, id: u32) -> (f32, f32) {
        let entity = game.state.entities.get(id).expect("replay actor");
        (entity.pos_x, entity.pos_y)
    }

    fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
        (b.0 - a.0).hypot(b.1 - a.1)
    }
}
