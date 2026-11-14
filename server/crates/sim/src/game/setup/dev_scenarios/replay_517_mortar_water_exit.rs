use super::*;
use crate::game::{entity::Entity, replay::CommandLogEntry};

const SEED: u32 = 654_567_149;
const ALEX: u32 = 4;
const MORTAR: u32 = 265;

impl Game {
    pub(super) fn new_replay_517_mortar_water_exit_scenario(
        unit: EntityKind,
        count: usize,
        _seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::MortarTeam || count != 1 {
            return Err("replay-517 water exit requires one Mortar Team".to_string());
        }
        let entities: Vec<Entity> =
            serde_json::from_str(include_str!("fixtures/replay_517_gunworks.json"))
                .map_err(|e| format!("invalid replay-517 producer: {e}"))?;
        let orders: Vec<CommandLogEntry> =
            serde_json::from_str(include_str!("fixtures/replay_517_mortar_orders.json"))
                .map_err(|e| format!("invalid replay-517 orders: {e}"))?;
        let mut map = Map::load("Classic", 2, SEED)?;
        let keep = |&(x, y): &(u32, u32)| (38..=52).contains(&x) && (26..=40).contains(&y);
        for y in 0..map.height {
            for x in 0..map.width {
                if !keep(&(x, y)) {
                    let index = map.index(x, y);
                    map.terrain[index] = crate::protocol::terrain::GRASS;
                }
            }
        }
        map.elevation.fill(0);
        map.base_sites.clear();
        map.base_resource_counts.clear();
        map.concealment_tiles.retain(keep);
        map.no_vehicle_tiles.retain(keep);
        map.no_building_tiles.retain(keep);
        map.no_entrenchment_tiles.retain(keep);
        map.damage_reduction_tiles.retain(keep);
        map.slow_movement_tiles.retain(keep);
        map.doodads
            .retain(|d| (1216..1696).contains(&d.x) && (832..1312).contains(&d.y));
        let mut game = build_dev_scenario_game_with_teams(
            map,
            EntityStore::from_checkpoint_entities(MORTAR, entities),
            [(3, 2), (ALEX, 3)],
            ALEX,
            (43, 33),
            SEED,
            "dev:replay_517_mortar_water_exit",
        );
        game.state.tick = 4125;
        game.state.ground_decals.begin_tick(4125);
        if let Some(player) = game.state.players.iter_mut().find(|p| p.id == ALEX) {
            player.name = "Alex".to_string();
            player.color = "#0072b2".to_string();
        }
        DevScenarioSetup {
            game,
            player_id: ALEX,
            units: vec![MORTAR],
            goal: (1979.9789, 908.0946),
            issue_after_ticks: 4161,
            // The room driver schedules before tick(); log entries label the applied tick.
            order: DevScenarioOrder::RecordedCommands(
                orders
                    .into_iter()
                    .map(|entry| (entry.tick - 1, SimCommand::from_protocol(entry.command)))
                    .collect(),
            ),
        }
        .checkpoint_backed("dev:replay_517_mortar_water_exit")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_517_mortar_production_and_recorded_moves_match_verified_replay() {
        let mut setup =
            Game::new_replay_517_mortar_water_exit_scenario(EntityKind::MortarTeam, 1, 0).unwrap();
        assert_eq!(setup.game.state.entities.iter().count(), 1);
        assert!(setup.game.state.entities.get(MORTAR).is_none());
        let expected: Vec<serde_json::Value> =
            serde_json::from_str(include_str!("fixtures/replay_517_mortar_positions.json"))
                .unwrap();
        assert_eq!(expected.len(), 100);
        let commands = setup.scheduled_commands();
        assert_eq!(commands.len(), 6);
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
            let mortar = setup.game.state.entities.get(MORTAR).unwrap();
            assert_eq!((mortar.kind, mortar.hp), (EntityKind::MortarTeam, 75));
            assert_eq!(
                (mortar.pos_x(), mortar.pos_y()),
                (
                    frame["x"].as_f64().unwrap() as f32,
                    frame["y"].as_f64().unwrap() as f32
                ),
                "tick {tick}"
            );
        }
        assert_eq!(next, commands.len());
        let mortar = setup.game.state.entities.get(MORTAR).unwrap();
        assert!(
            mortar.pos_x() > 1600.0,
            "the verified source mortar leaves the producer"
        );
        assert_eq!(setup.game.state.entities.iter().count(), 2);
    }
}
