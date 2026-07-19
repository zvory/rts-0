use super::*;
use crate::game::entity::{ProdItem, RallyIntent, RallyKind};

impl Game {
    pub fn new_factory_wall_rally_spawn_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !matches!(
            unit,
            EntityKind::ScoutCar | EntityKind::Tank | EntityKind::CommandCar
        ) {
            return Err(format!("unsupported factory-wall-rally unit {unit}"));
        }
        if unit_count != 1 {
            return Err(format!(
                "unsupported factory-wall-rally unit count {unit_count}"
            ));
        }

        let (map, start_tile, factory_pos, _, _, rally) = factory_wall_rally_spawn_map();
        let mut entities = EntityStore::new();
        let factory = entities
            .spawn_building(1, EntityKind::Factory, factory_pos.0, factory_pos.1, true)
            .ok_or_else(|| "failed to spawn factory".to_string())?;
        let producer = entities
            .get_mut(factory)
            .ok_or_else(|| "spawned factory is missing".to_string())?;
        producer.push_production(ProdItem {
            unit,
            progress: 1,
            total: 1,
            paid: true,
        });
        producer.set_rally_point(Some(RallyIntent::new(RallyKind::Move, rally.0, rally.1)));

        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:factory_wall_rally_spawn",
        );

        DevScenarioSetup {
            game,
            player_id,
            units: Vec::new(),
            goal: rally,
            issue_after_ticks: u32::MAX,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:factory_wall_rally_spawn")
    }
}
