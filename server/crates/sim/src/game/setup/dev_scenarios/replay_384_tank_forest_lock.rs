use super::*;
use crate::game::{entity::Entity, replay::CommandLogEntry};

const SEED: u32 = 1_583_372_308;
const START_TICK: u32 = 9_707;
const TANK: u32 = 245;
const ALEX: u32 = 2;

impl Game {
    pub(super) fn new_replay_384_tank_forest_lock_scenario(
        unit: EntityKind,
        count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Tank || count != 1 {
            return Err("replay-384 forest lock requires one Tank".to_string());
        }
        let entities: Vec<Entity> =
            serde_json::from_str(include_str!("fixtures/replay_384_tick_9707_tank.json"))
                .map_err(|e| format!("invalid replay-384 tank: {e}"))?;
        let orders: Vec<CommandLogEntry> =
            serde_json::from_str(include_str!("fixtures/replay_384_tank_orders.json"))
                .map_err(|e| format!("invalid replay-384 orders: {e}"))?;
        if orders.is_empty() {
            return Err("replay-384 requires recorded orders".to_string());
        }
        // The room driver enqueues before tick(), whereas replay logs label the applied tick.
        let commands = orders
            .into_iter()
            .map(|entry| {
                (
                    entry.tick.saturating_sub(1),
                    SimCommand::from_protocol(entry.command),
                )
            })
            .collect();
        let mut map = Map::load("Wald des Todes", 2, SEED)?;
        for y in 0..map.height {
            for x in 0..map.width {
                if !(55..=75).contains(&x) || !(35..=58).contains(&y) {
                    let index = map.index(x, y);
                    map.terrain[index] = crate::protocol::terrain::GRASS;
                }
            }
        }
        map.elevation.fill(0);
        map.base_sites.clear();
        map.base_resource_counts.clear();
        let keep = |&(x, y): &(u32, u32)| (55..=75).contains(&x) && (35..=58).contains(&y);
        map.concealment_tiles.retain(keep);
        map.no_vehicle_tiles.retain(keep);
        map.no_building_tiles.retain(keep);
        map.no_entrenchment_tiles.retain(keep);
        map.damage_reduction_tiles.retain(keep);
        map.slow_movement_tiles.retain(keep);
        map.doodads
            .retain(|d| (1760..2432).contains(&d.x) && (1120..1888).contains(&d.y));
        let mut game = build_dev_scenario_game_with_teams(
            map,
            EntityStore::from_checkpoint_entities(259, entities),
            [(1, 1), (ALEX, 2)],
            ALEX,
            (66, 40),
            SEED,
            "dev:replay_384_tank_forest_lock",
        );
        // Seven seconds of idle inspection before the recorded approach begins.
        game.state.tick = START_TICK - 210;
        game.state.ground_decals.begin_tick(START_TICK - 210);
        if let Some(player) = game.state.players.iter_mut().find(|p| p.id == ALEX) {
            player.name = "alex".to_string();
            player.color = "#d55e00".to_string();
        }
        DevScenarioSetup {
            game,
            player_id: ALEX,
            units: vec![TANK],
            goal: (2128.0, 1264.0),
            issue_after_ticks: START_TICK,
            order: DevScenarioOrder::RecordedCommands(commands),
        }
        .checkpoint_backed("dev:replay_384_tank_forest_lock")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_384_isolated_tank_matches_recorded_approach_and_failed_escape_orders() {
        let mut setup =
            Game::new_replay_384_tank_forest_lock_scenario(EntityKind::Tank, 1, 0).unwrap();
        let expected: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/replay_384_tank_positions.json")).unwrap();
        let commands = setup.scheduled_commands();
        let mut next = 0;
        for frame in expected {
            let tick = frame["tick"].as_u64().unwrap() as u32;
            while setup.game.tick_count() < tick {
                while commands
                    .get(next)
                    .is_some_and(|(t, _)| *t <= setup.game.tick_count())
                {
                    setup.game.enqueue(ALEX, commands[next].1.clone());
                    next += 1;
                }
                setup.game.tick();
            }
            let tank = setup.game.state.entities.get(TANK).unwrap();
            assert_eq!((tank.hp, tank.max_hp, tank.units_killed()), (292, 292, 2));
            let actual = (tank.pos_x, tank.pos_y);
            let expected = (
                frame["x"].as_f64().unwrap() as f32,
                frame["y"].as_f64().unwrap() as f32,
            );
            assert_eq!(actual, expected, "position at tick {tick}");
            assert_eq!(
                tank.facing(),
                frame["facing"].as_f64().unwrap() as f32,
                "facing at tick {tick}"
            );
        }
        assert_eq!(next, commands.len());
    }
}
