use crate::game::Game;

use super::scenario::{validate_lab_checkpoint_scenario_shape, LAB_CHECKPOINT_SCENARIO_KIND};
use super::{
    LabCheckpointScenarioMap, LabCheckpointScenarioMetadata, LabCheckpointScenarioSource,
    LabCheckpointScenarioV1, LabEntityIdRemap, LabError, LabScenarioV1,
    LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION,
};

impl Game {
    pub fn export_lab_checkpoint_scenario(
        &self,
        server_build_sha: &str,
    ) -> Result<LabCheckpointScenarioV1, LabError> {
        self.export_lab_checkpoint_scenario_with_metadata(
            "Untitled lab scenario".to_string(),
            self.tick_count(),
            None,
            Vec::new(),
            server_build_sha,
        )
    }

    pub fn lab_checkpoint_scenario_from_v1(
        scenario: LabScenarioV1,
        server_build_sha: &str,
    ) -> Result<LabCheckpointScenarioV1, LabError> {
        let name = scenario.name.clone();
        let exported_tick = scenario.metadata.exported_tick;
        let source = LabCheckpointScenarioSource {
            kind: scenario.kind.clone(),
            schema_version: scenario.schema_version,
        };
        let (game, restore) = Self::lab_game_from_scenario(scenario)?;
        game.export_lab_checkpoint_scenario_with_metadata(
            name,
            exported_tick,
            Some(source),
            restore.entity_id_map,
            server_build_sha,
        )
    }

    pub fn restore_lab_checkpoint_scenario(
        scenario: LabCheckpointScenarioV1,
    ) -> Result<Game, LabError> {
        validate_lab_checkpoint_scenario_shape(&scenario)?;
        let seed = scenario.seed;
        let (map, map_metadata) = scenario.map.into_map()?;
        let game = Game::restore_checkpoint_payload_text(
            &scenario.checkpoint_payload,
            map,
            map_metadata,
        )
        .map_err(|err| LabError::InvalidScenario {
            reason: format!("checkpoint scenario payload is invalid: {err}"),
        })?;
        if game.seed() != seed {
            return Err(LabError::InvalidScenario {
                reason: "checkpoint scenario seed does not match payload seed".to_string(),
            });
        }
        Ok(game)
    }

    fn export_lab_checkpoint_scenario_with_metadata(
        &self,
        name: String,
        exported_tick: u32,
        source_scenario: Option<LabCheckpointScenarioSource>,
        source_entity_id_map: Vec<LabEntityIdRemap>,
        server_build_sha: &str,
    ) -> Result<LabCheckpointScenarioV1, LabError> {
        let checkpoint_payload = self
            .checkpoint_payload_text_for_container("lab", server_build_sha)
            .map_err(|err| LabError::InvalidScenario {
                reason: format!("checkpoint scenario payload export failed: {err}"),
            })?;
        Ok(LabCheckpointScenarioV1 {
            schema_version: LAB_CHECKPOINT_SCENARIO_V1_SCHEMA_VERSION,
            kind: LAB_CHECKPOINT_SCENARIO_KIND.to_string(),
            name,
            seed: self.state.seed,
            map: LabCheckpointScenarioMap::from_map(&self.state.map, &self.state.map_metadata),
            metadata: LabCheckpointScenarioMetadata {
                exported_tick,
                source_scenario,
                source_entity_id_map,
            },
            checkpoint_payload,
        })
    }
}
