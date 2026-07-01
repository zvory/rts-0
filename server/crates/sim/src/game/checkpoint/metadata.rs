use serde::{Deserialize, Serialize};

use super::super::map::Map;
use super::super::replay::CommandLogEntry;
use super::super::state::{GameState, TrackedRng};
use super::super::MapMetadata;
use super::{
    CheckpointPayloadError, MAX_RNG_DRAWS_CONSUMED, PROTOCOL_VERSION, RNG_ALGORITHM, RULES_VERSION,
    SIM_SCHEMA_VERSION,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CheckpointCompatibilityV1 {
    created_by: String,
    server_build_sha: String,
    sim_schema_version: u32,
    rules_version: u32,
    protocol_version: u32,
    required_features: Vec<String>,
    optional_features: Vec<String>,
}

impl CheckpointCompatibilityV1 {
    pub(super) fn debug_default() -> Self {
        Self {
            created_by: "debug".to_string(),
            server_build_sha: option_env!("GIT_SHA").unwrap_or("unknown").to_string(),
            sim_schema_version: SIM_SCHEMA_VERSION,
            rules_version: RULES_VERSION,
            protocol_version: PROTOCOL_VERSION,
            required_features: Vec::new(),
            optional_features: Vec::new(),
        }
    }

    pub(super) fn validate(&self) -> Result<(), CheckpointPayloadError> {
        if self.sim_schema_version != SIM_SCHEMA_VERSION {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "compatibility.simSchemaVersion",
            });
        }
        if self.rules_version != RULES_VERSION {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "compatibility.rulesVersion",
            });
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "compatibility.protocolVersion",
            });
        }
        if let Some(feature) = self.required_features.first() {
            return Err(CheckpointPayloadError::UnsupportedRequiredFeature {
                feature: feature.clone(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct MapBindingV1 {
    name: String,
    schema_version: u32,
    content_hash: String,
    materialized_map_hash: String,
    size: u32,
    player_count: u32,
}

impl MapBindingV1 {
    pub(super) fn from_state(state: &GameState) -> Self {
        Self {
            name: state.map_metadata.name.clone(),
            schema_version: state.map_metadata.schema_version,
            content_hash: state.map_metadata.content_hash.clone(),
            materialized_map_hash: state.map.materialized_hash(),
            size: state.map.size,
            player_count: state.players.len() as u32,
        }
    }

    pub(super) fn validate_against(
        &self,
        map: &Map,
        map_metadata: &MapMetadata,
    ) -> Result<(), CheckpointPayloadError> {
        if self.name != map_metadata.name {
            return Err(CheckpointPayloadError::MapBindingMismatch { field: "name" });
        }
        if self.schema_version != map_metadata.schema_version {
            return Err(CheckpointPayloadError::MapBindingMismatch {
                field: "schemaVersion",
            });
        }
        if self.content_hash != map_metadata.content_hash {
            return Err(CheckpointPayloadError::MapBindingMismatch {
                field: "contentHash",
            });
        }
        if self.materialized_map_hash != map.materialized_hash() {
            return Err(CheckpointPayloadError::MapBindingMismatch {
                field: "materializedMapHash",
            });
        }
        if self.size != map.size {
            return Err(CheckpointPayloadError::MapBindingMismatch { field: "size" });
        }
        if self.player_count as usize != map.starts.len() {
            return Err(CheckpointPayloadError::MapBindingMismatch {
                field: "playerCount",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RngDescriptorV1 {
    algorithm: String,
    pub(super) seed: u64,
    pub(super) draws_consumed: u64,
}

impl RngDescriptorV1 {
    pub(super) fn from_rng(rng: &TrackedRng) -> Self {
        Self {
            algorithm: RNG_ALGORITHM.to_string(),
            seed: rng.seed(),
            draws_consumed: rng.draws_consumed(),
        }
    }

    pub(super) fn validate(&self, match_seed: u32) -> Result<(), CheckpointPayloadError> {
        if self.algorithm != RNG_ALGORITHM {
            return Err(CheckpointPayloadError::IncompatibleRngAlgorithm {
                found: self.algorithm.clone(),
            });
        }
        if self.seed != match_seed as u64 {
            return Err(CheckpointPayloadError::InvalidValue { field: "rng.seed" });
        }
        if self.draws_consumed > MAX_RNG_DRAWS_CONSUMED {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "rng.drawsConsumed",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CommandLogMetadataV1 {
    protocol_version: u32,
    first_tick: Option<u32>,
    last_tick: Option<u32>,
    complete_from_tick_zero: bool,
    replay_base_tick: Option<u32>,
}

impl CommandLogMetadataV1 {
    pub(super) fn from_command_log(command_log: &[CommandLogEntry]) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            first_tick: command_log.first().map(|entry| entry.tick),
            last_tick: command_log.last().map(|entry| entry.tick),
            complete_from_tick_zero: true,
            replay_base_tick: None,
        }
    }

    pub(super) fn validate_against(
        &self,
        command_log: &[CommandLogEntry],
    ) -> Result<(), CheckpointPayloadError> {
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLogMetadata.protocolVersion",
            });
        }
        if self.first_tick != command_log.first().map(|entry| entry.tick) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLogMetadata.firstTick",
            });
        }
        if self.last_tick != command_log.last().map(|entry| entry.tick) {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLogMetadata.lastTick",
            });
        }
        if !self.complete_from_tick_zero {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLogMetadata.completeFromTickZero",
            });
        }
        if self.replay_base_tick.is_some() {
            return Err(CheckpointPayloadError::InvalidValue {
                field: "commandLogMetadata.replayBaseTick",
            });
        }
        Ok(())
    }
}
