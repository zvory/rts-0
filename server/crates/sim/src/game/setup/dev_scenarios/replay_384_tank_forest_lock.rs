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
    fn replay_384_isolated_tank_remains_legal_and_escapes_historical_forest_lock() {
        let mut setup =
            Game::new_replay_384_tank_forest_lock_scenario(EntityKind::Tank, 1, 0).unwrap();
        let historical: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/replay_384_tank_positions.json")).unwrap();
        let commands = setup.scheduled_commands();
        let mut next = 0;
        // Keep the exact recorded start and orders. The old trace contains the permanent
        // forest lock, so matching every historical position would require preserving the bug.
        while setup.game.tick_count() < 11_200 {
            while commands
                .get(next)
                .is_some_and(|(tick, _)| *tick <= setup.game.tick_count())
            {
                setup.game.enqueue(ALEX, commands[next].1.clone());
                next += 1;
            }
            setup.game.tick();
            let tank = setup.game.state.entities.get(TANK).unwrap();
            assert_eq!((tank.hp, tank.max_hp, tank.units_killed()), (292, 292, 2));
            let occ = crate::game::services::occupancy::Occupancy::build(
                &setup.game.state.map,
                &setup.game.state.entities,
            );
            assert!(
                crate::game::services::standability::unit_static_standable_with_facing(
                    &setup.game.state.map,
                    &occ,
                    tank.kind,
                    tank.pos_x(),
                    tank.pos_y(),
                    tank.facing()
                ),
                "illegal hull at {}",
                setup.game.tick_count()
            );
            if setup.game.tick_count() == START_TICK {
                let first = &historical[0];
                assert!((tank.pos_x() - first["x"].as_f64().unwrap() as f32).abs() < 0.1);
                assert!((tank.pos_y() - first["y"].as_f64().unwrap() as f32).abs() < 0.1);
                assert!((tank.facing() - first["facing"].as_f64().unwrap() as f32).abs() < 0.001);
            }
        }
        assert_eq!(next, commands.len());
        let tank = setup.game.state.entities.get(TANK).unwrap();
        let lock = historical.last().unwrap();
        let dx = tank.pos_x() - lock["x"].as_f64().unwrap() as f32;
        let dy = tank.pos_y() - lock["y"].as_f64().unwrap() as f32;
        assert!(
            dx * dx + dy * dy > 32.0 * 32.0,
            "tank must leave historical lock: {:?}",
            (tank.pos_x(), tank.pos_y())
        );
        let goal = tank.path_goal().unwrap_or((1910.2272, 1283.0035));
        let dx = tank.pos_x() - goal.0;
        let dy = tank.pos_y() - goal.1;
        assert!(
            dx * dx + dy * dy < 96.0 * 96.0,
            "tank must resume travel toward its final order: {:?} goal {:?}",
            (tank.pos_x(), tank.pos_y()),
            goal
        );
    }
}
