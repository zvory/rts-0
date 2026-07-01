use crate::game::Game;

use super::orders::{lab_entity_active_order, lab_entity_queued_orders};
use super::orientation::{
    lab_entity_facing, lab_entity_is_set_up, lab_entity_setup_facing, lab_entity_setup_target,
    lab_entity_weapon_facing,
};
use super::scenario::LAB_SCENARIO_KIND;
use super::{
    LabScenarioEntity, LabScenarioMap, LabScenarioMetadata, LabScenarioPlayer, LabScenarioResearch,
    LabScenarioResources, LabScenarioV1, LAB_SCENARIO_V1_SCHEMA_VERSION,
};

impl Game {
    pub fn export_lab_scenario(&self) -> LabScenarioV1 {
        let players = self
            .state
            .players
            .iter()
            .map(|player| LabScenarioPlayer {
                id: player.id,
                team_id: player.team_id,
                faction_id: player.faction_id.clone(),
                name: player.name.clone(),
                color: player.color.clone(),
                is_ai: player.is_ai,
                resources: LabScenarioResources {
                    steel: player.steel,
                    oil: player.oil,
                },
                research: LabScenarioResearch {
                    completed: player
                        .upgrades
                        .iter()
                        .map(|upgrade| upgrade.to_protocol_str().to_string())
                        .collect(),
                },
            })
            .collect();

        let entities = self
            .state
            .entities
            .iter()
            .map(|entity| LabScenarioEntity {
                id: entity.id,
                owner: entity.owner,
                kind: entity.kind.to_string(),
                x: entity.pos_x,
                y: entity.pos_y,
                hp: entity.hp,
                completed: !entity.under_construction(),
                construction_progress: entity.construction.as_ref().map(|state| state.progress),
                construction_total: entity.construction.as_ref().map(|state| state.total),
                resource_remaining: entity.remaining(),
                facing: lab_entity_facing(entity),
                weapon_facing: lab_entity_weapon_facing(entity),
                set_up: lab_entity_is_set_up(entity),
                setup_facing: lab_entity_setup_facing(entity),
                setup_target: lab_entity_setup_target(&self.state.map, entity),
                order: lab_entity_active_order(entity),
                queued_orders: lab_entity_queued_orders(entity),
            })
            .collect();

        LabScenarioV1 {
            schema_version: LAB_SCENARIO_V1_SCHEMA_VERSION,
            kind: LAB_SCENARIO_KIND.to_string(),
            name: "Untitled lab scenario".to_string(),
            seed: self.state.seed,
            map: LabScenarioMap {
                name: self.state.map_metadata.name.clone(),
                schema_version: self.state.map_metadata.schema_version,
                content_hash: self.state.map_metadata.content_hash.clone(),
            },
            players,
            entities,
            metadata: LabScenarioMetadata {
                exported_tick: self.tick_count(),
            },
        }
    }
}
