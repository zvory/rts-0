use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
    terrain, Command, LabCheckpointScenarioV1, LabScenarioEntityIdRemap, LabSpawnEntitySpec,
    LabUpdateSpec, LabVisionMode, TeamId,
};

pub const LAB_REPLAY_ARTIFACT_SCHEMA: &str = "rts.labReplay";
pub const LAB_REPLAY_ARTIFACT_KIND: &str = "labReplay";
pub const LAB_REPLAY_ARTIFACT_SCHEMA_VERSION: u32 = 1;
pub const LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS: u32 = 2_000;

pub const LAB_REPLAY_MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
pub const LAB_REPLAY_MAX_OPERATIONS: usize = 50_000;
pub const LAB_REPLAY_MAX_OPERATION_JSON_BYTES: usize = 64 * 1024;
pub const LAB_REPLAY_MAX_CHECKPOINT_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
pub const LAB_REPLAY_MAX_AUTHORING_NAME_BYTES: usize = 120;
pub const LAB_REPLAY_MAX_AUTHORING_AUTHOR_BYTES: usize = 80;
pub const LAB_REPLAY_MAX_AUTHORING_DESCRIPTION_BYTES: usize = 2_000;
pub const LAB_REPLAY_MAX_AUTHORING_TAGS: usize = 16;
pub const LAB_REPLAY_MAX_AUTHORING_TAG_BYTES: usize = 32;
pub const LAB_REPLAY_MAX_MAP_TILES: usize = 1_000_000;
pub const LAB_REPLAY_MAX_MAP_STARTS: usize = 8;
pub const LAB_REPLAY_MAX_MAP_BASE_SITES: usize = 64;
pub const LAB_REPLAY_MAX_UNITS_PER_COMMAND: usize = 256;
pub const LAB_REPLAY_LAB_MAX_UNITS_PER_COMMAND: usize = 4_096;
pub const LAB_REPLAY_MAX_MUTATION_BATCH: usize = 400;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabReplayArtifactV1 {
    pub schema: String,
    pub schema_version: u32,
    pub kind: String,
    pub server_build_sha: String,
    pub authoring: LabReplayAuthoringMetadata,
    pub initial_setup: LabCheckpointScenarioV1,
    pub timeline: LabReplayTimelineMetadata,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operations: Vec<LabReplayOperationEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabReplayAuthoringMetadata {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabReplayTimelineMetadata {
    pub initial_tick: u32,
    pub duration_ticks: u32,
    pub keyframe_interval_ticks: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LabReplayOperationEntry {
    pub sequence: u64,
    pub tick: u32,
    pub request_id: u32,
    pub operator_id: u32,
    pub op: LabReplayOperation,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum LabReplayOperation {
    SpawnEntities {
        spawns: Vec<LabSpawnEntitySpec>,
    },
    ApplyUpdates {
        updates: Vec<LabUpdateSpec>,
    },
    DeleteEntities {
        entity_ids: Vec<u32>,
    },
    SpawnEntity {
        owner: u32,
        kind: String,
        x: f32,
        y: f32,
        #[serde(default)]
        completed: bool,
    },
    DeleteEntity {
        entity_id: u32,
    },
    MoveEntity {
        entity_id: u32,
        x: f32,
        y: f32,
    },
    SetEntityOwner {
        entity_id: u32,
        owner: u32,
    },
    SetPlayerResources {
        player_id: u32,
        steel: u32,
        oil: u32,
    },
    SetPlayerGodMode {
        player_id: u32,
        enabled: bool,
    },
    SetCompletedResearch {
        player_id: u32,
        upgrade: String,
        completed: bool,
    },
    IssueCommandAs {
        player_id: u32,
        cmd: Command,
        #[serde(default, skip_serializing_if = "is_false")]
        ignore_command_limits: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabReplayValidationError {
    message: String,
}

impl LabReplayValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for LabReplayValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for LabReplayValidationError {}

#[derive(Debug, Clone)]
struct CheckpointFacts {
    tick: u32,
    player_ids: HashSet<u32>,
    entity_ids: HashSet<u32>,
    next_entity_id: u32,
}

#[derive(Debug, Clone)]
struct ValidationState {
    player_ids: HashSet<u32>,
    entity_ids: HashSet<u32>,
    next_entity_id: u32,
    entity_ids_precise: bool,
    command_effects_pending: bool,
}

pub fn lab_replay_artifact_from_slice(
    bytes: &[u8],
) -> Result<LabReplayArtifactV1, LabReplayValidationError> {
    if bytes.len() > LAB_REPLAY_MAX_ARTIFACT_BYTES {
        return Err(invalid(format!(
            "lab replay artifact must be at most {LAB_REPLAY_MAX_ARTIFACT_BYTES} bytes"
        )));
    }
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|err| invalid(format!("invalid lab replay JSON: {err}")))?;
    validate_raw_operation_stream(&value)?;
    reject_unsupported_session_ops(&value)?;
    let artifact: LabReplayArtifactV1 = serde_json::from_value(value)
        .map_err(|err| invalid(format!("invalid lab replay artifact shape: {err}")))?;
    validate_lab_replay_artifact(&artifact)?;
    Ok(artifact)
}

pub fn validate_lab_replay_artifact(
    artifact: &LabReplayArtifactV1,
) -> Result<(), LabReplayValidationError> {
    validate_artifact_header(artifact)?;
    validate_authoring(&artifact.authoring)?;

    let initial = checkpoint_facts("initialSetup", &artifact.initial_setup)?;
    if artifact.timeline.initial_tick != initial.tick {
        return Err(invalid(
            "lab replay timeline initialTick must match initialSetup exportedTick",
        ));
    }
    if artifact.timeline.duration_ticks < artifact.timeline.initial_tick {
        return Err(invalid(
            "lab replay timeline durationTicks must be at least initialTick",
        ));
    }
    if artifact.timeline.keyframe_interval_ticks != LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS {
        return Err(invalid(format!(
            "lab replay timeline keyframeIntervalTicks must be {LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS}"
        )));
    }
    if artifact.operations.len() > LAB_REPLAY_MAX_OPERATIONS {
        return Err(invalid(format!(
            "lab replay operation count must be at most {LAB_REPLAY_MAX_OPERATIONS}"
        )));
    }

    let mut state = ValidationState {
        player_ids: initial.player_ids,
        entity_ids: initial.entity_ids,
        next_entity_id: initial.next_entity_id,
        entity_ids_precise: true,
        command_effects_pending: false,
    };
    let mut last_tick = artifact.timeline.initial_tick;
    for (index, entry) in artifact.operations.iter().enumerate() {
        validate_entry(
            entry,
            index,
            artifact.timeline.initial_tick,
            artifact.timeline.duration_ticks,
            &mut last_tick,
            &mut state,
        )?;
    }
    Ok(())
}

fn validate_artifact_header(
    artifact: &LabReplayArtifactV1,
) -> Result<(), LabReplayValidationError> {
    if artifact.schema != LAB_REPLAY_ARTIFACT_SCHEMA {
        return Err(invalid(format!(
            "lab replay schema must be {LAB_REPLAY_ARTIFACT_SCHEMA}"
        )));
    }
    if artifact.schema_version != LAB_REPLAY_ARTIFACT_SCHEMA_VERSION {
        return Err(invalid(format!(
            "unsupported lab replay schemaVersion {}; expected {LAB_REPLAY_ARTIFACT_SCHEMA_VERSION}",
            artifact.schema_version
        )));
    }
    if artifact.kind != LAB_REPLAY_ARTIFACT_KIND {
        return Err(invalid(format!(
            "lab replay kind must be {LAB_REPLAY_ARTIFACT_KIND}"
        )));
    }
    if artifact.server_build_sha.trim().is_empty() || artifact.server_build_sha.len() > 128 {
        return Err(invalid(
            "lab replay serverBuildSha must be non-empty and at most 128 bytes",
        ));
    }
    Ok(())
}

fn validate_authoring(
    metadata: &LabReplayAuthoringMetadata,
) -> Result<(), LabReplayValidationError> {
    validate_non_empty_len(
        "authoring.name",
        &metadata.name,
        LAB_REPLAY_MAX_AUTHORING_NAME_BYTES,
    )?;
    if let Some(author) = &metadata.author {
        validate_optional_len(
            "authoring.author",
            author,
            LAB_REPLAY_MAX_AUTHORING_AUTHOR_BYTES,
        )?;
    }
    if let Some(description) = &metadata.description {
        validate_optional_len(
            "authoring.description",
            description,
            LAB_REPLAY_MAX_AUTHORING_DESCRIPTION_BYTES,
        )?;
    }
    if metadata.tags.len() > LAB_REPLAY_MAX_AUTHORING_TAGS {
        return Err(invalid(format!(
            "authoring.tags must contain at most {LAB_REPLAY_MAX_AUTHORING_TAGS} entries"
        )));
    }
    for tag in &metadata.tags {
        validate_non_empty_len("authoring.tags[]", tag, LAB_REPLAY_MAX_AUTHORING_TAG_BYTES)?;
        if !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(invalid(
                "authoring.tags[] must contain only ASCII letters, numbers, '-' or '_'",
            ));
        }
    }
    Ok(())
}

fn validate_entry(
    entry: &LabReplayOperationEntry,
    index: usize,
    initial_tick: u32,
    duration_ticks: u32,
    last_tick: &mut u32,
    state: &mut ValidationState,
) -> Result<(), LabReplayValidationError> {
    if entry.sequence != index as u64 {
        return Err(invalid(format!(
            "lab replay operation sequence must be contiguous from 0; entry {index} has {}",
            entry.sequence
        )));
    }
    if entry.tick < initial_tick || entry.tick > duration_ticks {
        return Err(invalid(format!(
            "lab replay operation {} tick must be within timeline bounds",
            entry.sequence
        )));
    }
    if entry.tick < *last_tick {
        return Err(invalid(format!(
            "lab replay operation {} tick is out of order",
            entry.sequence
        )));
    }
    // Once command-driven simulation has advanced, only the authoritative replay rebuild can know
    // which entities were produced, killed, or otherwise removed by normal systems.
    if entry.tick > *last_tick && state.command_effects_pending {
        state.entity_ids_precise = false;
    }
    if entry.request_id == 0 {
        return Err(invalid(format!(
            "lab replay operation {} requestId must be nonzero",
            entry.sequence
        )));
    }
    if entry.operator_id == 0 {
        return Err(invalid(format!(
            "lab replay operation {} operatorId must be nonzero",
            entry.sequence
        )));
    }
    validate_operation(&entry.op, state)?;
    let op_bytes = serde_json::to_vec(&entry.op)
        .map_err(|err| invalid(format!("failed to measure lab replay operation: {err}")))?;
    if op_bytes.len() > LAB_REPLAY_MAX_OPERATION_JSON_BYTES {
        return Err(invalid(format!(
            "lab replay operation payload must be at most {LAB_REPLAY_MAX_OPERATION_JSON_BYTES} bytes"
        )));
    }
    if matches!(entry.op, LabReplayOperation::IssueCommandAs { .. }) {
        state.command_effects_pending = true;
    }
    *last_tick = entry.tick;
    Ok(())
}

fn validate_operation(
    op: &LabReplayOperation,
    state: &mut ValidationState,
) -> Result<(), LabReplayValidationError> {
    match op {
        LabReplayOperation::SpawnEntities { spawns } => {
            validate_replay_batch_len("spawnEntities.spawns", spawns.len())?;
            for spawn in spawns {
                validate_player_id(spawn.owner, "spawnEntities.spawns[].owner", state)?;
                validate_non_empty_len("spawnEntities.spawns[].kind", &spawn.kind, 64)?;
                validate_finite_point(spawn.x, spawn.y, "spawnEntities.spawns[]")?;
                allocate_replay_entity_id(state, "spawnEntities")?;
            }
        }
        LabReplayOperation::ApplyUpdates { updates } => {
            validate_replay_batch_len("applyUpdates.updates", updates.len())?;
            let mut entity_ids = HashSet::new();
            let mut player_fields = HashSet::new();
            for update in updates {
                match update {
                    LabUpdateSpec::Move { entity_id, x, y } => {
                        if !entity_ids.insert(*entity_id) {
                            return Err(invalid("applyUpdates contains a duplicate entity update"));
                        }
                        validate_entity_id(*entity_id, "applyUpdates.entityId", state)?;
                        validate_finite_point(*x, *y, "applyUpdates.move")?;
                    }
                    LabUpdateSpec::Reassign { entity_id, owner } => {
                        if !entity_ids.insert(*entity_id) {
                            return Err(invalid("applyUpdates contains a duplicate entity update"));
                        }
                        validate_entity_id(*entity_id, "applyUpdates.entityId", state)?;
                        validate_player_id(*owner, "applyUpdates.owner", state)?;
                    }
                    LabUpdateSpec::Resources { player_id, .. } => {
                        validate_player_id(*player_id, "applyUpdates.playerId", state)?;
                        if !player_fields.insert((*player_id, "resources".to_string())) {
                            return Err(invalid("applyUpdates contains a duplicate player field"));
                        }
                    }
                    LabUpdateSpec::Research {
                        player_id, upgrade, ..
                    } => {
                        validate_player_id(*player_id, "applyUpdates.playerId", state)?;
                        validate_non_empty_len("applyUpdates.upgrade", upgrade, 64)?;
                        if !player_fields.insert((*player_id, format!("research:{upgrade}"))) {
                            return Err(invalid("applyUpdates contains a duplicate player field"));
                        }
                    }
                    LabUpdateSpec::GodMode { player_id, .. } => {
                        validate_player_id(*player_id, "applyUpdates.playerId", state)?;
                        if !player_fields.insert((*player_id, "godMode".to_string())) {
                            return Err(invalid("applyUpdates contains a duplicate player field"));
                        }
                    }
                }
            }
        }
        LabReplayOperation::DeleteEntities { entity_ids } => {
            validate_replay_batch_len("deleteEntities.entityIds", entity_ids.len())?;
            let mut seen = HashSet::new();
            for entity_id in entity_ids {
                if !seen.insert(*entity_id) {
                    return Err(invalid("deleteEntities contains a duplicate entity id"));
                }
                validate_entity_id(*entity_id, "deleteEntities.entityIds[]", state)?;
                if state.entity_ids_precise {
                    state.entity_ids.remove(entity_id);
                }
            }
        }
        LabReplayOperation::SpawnEntity {
            owner, kind, x, y, ..
        } => {
            validate_player_id(*owner, "spawnEntity.owner", state)?;
            validate_non_empty_len("spawnEntity.kind", kind, 64)?;
            validate_finite_point(*x, *y, "spawnEntity")?;
            allocate_replay_entity_id(state, "spawnEntity")?;
        }
        LabReplayOperation::DeleteEntity { entity_id } => {
            validate_entity_id(*entity_id, "deleteEntity.entityId", state)?;
            if state.entity_ids_precise {
                state.entity_ids.remove(entity_id);
            }
        }
        LabReplayOperation::MoveEntity { entity_id, x, y } => {
            validate_entity_id(*entity_id, "moveEntity.entityId", state)?;
            validate_finite_point(*x, *y, "moveEntity")?;
        }
        LabReplayOperation::SetEntityOwner { entity_id, owner } => {
            validate_entity_id(*entity_id, "setEntityOwner.entityId", state)?;
            validate_player_id(*owner, "setEntityOwner.owner", state)?;
        }
        LabReplayOperation::SetPlayerResources { player_id, .. }
        | LabReplayOperation::SetPlayerGodMode { player_id, .. } => {
            validate_player_id(*player_id, "lab replay playerId", state)?;
        }
        LabReplayOperation::SetCompletedResearch {
            player_id, upgrade, ..
        } => {
            validate_player_id(*player_id, "setCompletedResearch.playerId", state)?;
            validate_non_empty_len("setCompletedResearch.upgrade", upgrade, 64)?;
        }
        LabReplayOperation::IssueCommandAs {
            player_id,
            cmd,
            ignore_command_limits,
        } => {
            validate_player_id(*player_id, "issueCommandAs.playerId", state)?;
            validate_command(cmd, *ignore_command_limits, state)?;
        }
    }
    Ok(())
}

fn validate_replay_batch_len(label: &str, len: usize) -> Result<(), LabReplayValidationError> {
    if (1..=LAB_REPLAY_MAX_MUTATION_BATCH).contains(&len) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} must contain 1 to {LAB_REPLAY_MAX_MUTATION_BATCH} items"
        )))
    }
}

fn allocate_replay_entity_id(
    state: &mut ValidationState,
    label: &str,
) -> Result<(), LabReplayValidationError> {
    if !state.entity_ids_precise {
        return Ok(());
    }
    if state.next_entity_id == 0 {
        return Err(invalid(
            "checkpoint entity allocator nextId must be nonzero",
        ));
    }
    if !state.entity_ids.insert(state.next_entity_id) {
        return Err(invalid(format!(
            "{label} would reuse existing entity id {}",
            state.next_entity_id
        )));
    }
    state.next_entity_id = state.next_entity_id.checked_add(1).ok_or_else(|| {
        invalid(format!(
            "{label} would overflow the artifact entity id allocator"
        ))
    })?;
    Ok(())
}

fn validate_command(
    command: &Command,
    ignore_command_limits: bool,
    state: &ValidationState,
) -> Result<(), LabReplayValidationError> {
    let unit_cap = if ignore_command_limits {
        LAB_REPLAY_LAB_MAX_UNITS_PER_COMMAND
    } else {
        LAB_REPLAY_MAX_UNITS_PER_COMMAND
    };
    match command {
        Command::Move { units, x, y, .. }
        | Command::AttackMove { units, x, y, .. }
        | Command::SetupAntiTankGuns { units, x, y, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
            validate_finite_point(*x, *y, "command")?;
        }
        Command::Attack { units, target, .. } | Command::Deconstruct { units, target, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
            validate_entity_id(*target, "command.target", state)?;
        }
        Command::TearDownAntiTankGuns { units }
        | Command::Charge { units }
        | Command::Stop { units }
        | Command::HoldPosition { units }
        | Command::SetAutocast { units, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
        }
        Command::UseAbility { units, x, y, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
            if let (Some(x), Some(y)) = (x, y) {
                validate_finite_point(*x, *y, "command.useAbility")?;
            }
        }
        Command::RecastAbility { units, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
        }
        Command::Gather { units, node, .. } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
            validate_entity_id(*node, "command.node", state)?;
        }
        Command::Build {
            units,
            tile_x,
            tile_y,
            ..
        }
        | Command::QueueBuild {
            units,
            tile_x,
            tile_y,
            ..
        } => {
            validate_unit_list(units, unit_cap, "command.units", state)?;
            if *tile_x == u32::MAX || *tile_y == u32::MAX {
                return Err(invalid("command build tile coordinates are out of range"));
            }
        }
        Command::Train { building, .. }
        | Command::Research { building, .. }
        | Command::QueueResearch { building, .. }
        | Command::Cancel { building } => {
            validate_entity_id(*building, "command.building", state)?;
        }
        Command::QueueTrain {
            building, quantity, ..
        } => {
            validate_entity_id(*building, "command.building", state)?;
            if *quantity == 0 || *quantity > 1_000 {
                return Err(invalid("command queue quantity is out of range"));
            }
        }
        Command::SetRally { building, x, y, .. } => {
            validate_entity_id(*building, "command.building", state)?;
            validate_finite_point(*x, *y, "command.setRally")?;
        }
    }
    Ok(())
}

fn checkpoint_facts(
    label: &str,
    scenario: &LabCheckpointScenarioV1,
) -> Result<CheckpointFacts, LabReplayValidationError> {
    if scenario.schema_version != 1 {
        return Err(invalid(format!(
            "{label} schemaVersion must be 1 for LabCheckpointScenarioV1"
        )));
    }
    if scenario.kind != "labCheckpointScenario" {
        return Err(invalid(format!(
            "{label} kind must be labCheckpointScenario"
        )));
    }
    validate_non_empty_len(&format!("{label}.name"), &scenario.name, 80)?;
    validate_map_container(label, scenario)?;
    if scenario.checkpoint_payload.len() > LAB_REPLAY_MAX_CHECKPOINT_PAYLOAD_BYTES {
        return Err(invalid(format!(
            "{label}.checkpointPayload must be at most {LAB_REPLAY_MAX_CHECKPOINT_PAYLOAD_BYTES} bytes"
        )));
    }
    let checkpoint: serde_json::Value = serde_json::from_str(&scenario.checkpoint_payload)
        .map_err(|err| invalid(format!("{label}.checkpointPayload is invalid JSON: {err}")))?;
    validate_checkpoint_header(label, &checkpoint)?;
    validate_checkpoint_map_binding(label, scenario, &checkpoint)?;

    let seed = required_u32(&checkpoint, "seed", label)?;
    if seed != scenario.seed {
        return Err(invalid(format!("{label} seed must match checkpoint seed")));
    }
    let tick = required_u32(&checkpoint, "tick", label)?;
    if tick != scenario.metadata.exported_tick {
        return Err(invalid(format!(
            "{label} metadata.exportedTick must match checkpoint tick"
        )));
    }
    let (player_ids, team_ids) = checkpoint_players(label, &checkpoint)?;
    validate_lab_metadata(
        label,
        &scenario.metadata.lab.vision,
        &scenario.metadata.lab.god_mode_players,
        &player_ids,
        &team_ids,
    )?;
    let (entity_ids, next_entity_id) = checkpoint_entities(label, &checkpoint)?;
    validate_source_entity_id_map(label, &scenario.metadata.source_entity_id_map, &entity_ids)?;

    Ok(CheckpointFacts {
        tick,
        player_ids,
        entity_ids,
        next_entity_id,
    })
}

fn validate_map_container(
    label: &str,
    scenario: &LabCheckpointScenarioV1,
) -> Result<(), LabReplayValidationError> {
    validate_non_empty_len(&format!("{label}.map.name"), &scenario.map.name, 120)?;
    validate_non_empty_len(
        &format!("{label}.map.contentHash"),
        &scenario.map.content_hash,
        128,
    )?;
    validate_non_empty_len(
        &format!("{label}.map.materializedHash"),
        &scenario.map.materialized_hash,
        128,
    )?;
    let size = scenario.map.data.size;
    let tile_count = size
        .checked_mul(size)
        .map(|count| count as usize)
        .ok_or_else(|| invalid(format!("{label}.map.data.size overflows")))?;
    if size == 0
        || tile_count != scenario.map.data.terrain.len()
        || tile_count > LAB_REPLAY_MAX_MAP_TILES
    {
        return Err(invalid(format!(
            "{label}.map.data terrain length must match size*size and fit the cap"
        )));
    }
    for &tile in &scenario.map.data.terrain {
        if !matches!(tile, terrain::GRASS | terrain::ROCK | terrain::WATER) {
            return Err(invalid(format!(
                "{label}.map.data.terrain contains an unknown terrain code"
            )));
        }
    }
    if scenario.map.data.starts.is_empty()
        || scenario.map.data.starts.len() > LAB_REPLAY_MAX_MAP_STARTS
    {
        return Err(invalid(format!("{label}.map.data.starts count is invalid")));
    }
    if scenario.map.data.base_sites.len() > LAB_REPLAY_MAX_MAP_BASE_SITES {
        return Err(invalid(format!(
            "{label}.map.data.baseSites count is invalid"
        )));
    }
    for site in scenario
        .map
        .data
        .starts
        .iter()
        .chain(scenario.map.data.base_sites.iter())
    {
        if site.x >= size || site.y >= size {
            return Err(invalid(format!(
                "{label}.map.data contains an out-of-bounds site"
            )));
        }
    }
    Ok(())
}

fn validate_checkpoint_header(
    label: &str,
    checkpoint: &serde_json::Value,
) -> Result<(), LabReplayValidationError> {
    let schema = required_str(checkpoint, "schema", label)?;
    if schema != "rts.gameCheckpoint" {
        return Err(invalid(format!(
            "{label}.checkpointPayload schema must be rts.gameCheckpoint"
        )));
    }
    let version = required_u32(checkpoint, "version", label)?;
    if version != 1 {
        return Err(invalid(format!(
            "{label}.checkpointPayload version must be 1"
        )));
    }
    Ok(())
}

fn validate_checkpoint_map_binding(
    label: &str,
    scenario: &LabCheckpointScenarioV1,
    checkpoint: &serde_json::Value,
) -> Result<(), LabReplayValidationError> {
    let binding = checkpoint
        .get("mapBinding")
        .ok_or_else(|| invalid(format!("{label}.checkpointPayload missing mapBinding")))?;
    let name = required_str(binding, "name", label)?;
    let schema_version = required_u32(binding, "schemaVersion", label)?;
    let content_hash = required_str(binding, "contentHash", label)?;
    let materialized_hash = required_str(binding, "materializedMapHash", label)?;
    let size = required_u32(binding, "size", label)?;
    if name != scenario.map.name {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.name mismatch"
        )));
    }
    if schema_version != scenario.map.schema_version {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.schemaVersion mismatch"
        )));
    }
    if content_hash != scenario.map.content_hash {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.contentHash mismatch"
        )));
    }
    if materialized_hash != scenario.map.materialized_hash {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.materializedMapHash mismatch"
        )));
    }
    if size != scenario.map.data.size {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.size mismatch"
        )));
    }
    let player_count = required_u32(binding, "playerCount", label)?;
    let players_len = checkpoint
        .get("players")
        .and_then(serde_json::Value::as_array)
        .map(|players| players.len() as u32)
        .ok_or_else(|| {
            invalid(format!(
                "{label}.checkpointPayload players must be an array"
            ))
        })?;
    if player_count != players_len {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.playerCount mismatch"
        )));
    }
    if player_count as usize != scenario.map.data.starts.len() {
        return Err(invalid(format!(
            "{label} checkpoint mapBinding.playerCount must match map starts"
        )));
    }
    Ok(())
}

fn checkpoint_players(
    label: &str,
    checkpoint: &serde_json::Value,
) -> Result<(HashSet<u32>, HashSet<TeamId>), LabReplayValidationError> {
    let players = checkpoint
        .get("players")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            invalid(format!(
                "{label}.checkpointPayload players must be an array"
            ))
        })?;
    if players.is_empty() {
        return Err(invalid(format!(
            "{label}.checkpointPayload players must not be empty"
        )));
    }
    let mut player_ids = HashSet::new();
    let mut team_ids = HashSet::new();
    for player in players {
        let player_id = required_u32(player, "id", label)?;
        if player_id == 0 || !player_ids.insert(player_id) {
            return Err(invalid(format!(
                "{label}.checkpointPayload players contain duplicate or zero id"
            )));
        }
        let team_id = required_u32(player, "teamId", label)?;
        if team_id == 0 {
            return Err(invalid(format!(
                "{label}.checkpointPayload players contain zero teamId"
            )));
        }
        team_ids.insert(team_id);
    }
    Ok((player_ids, team_ids))
}

fn checkpoint_entities(
    label: &str,
    checkpoint: &serde_json::Value,
) -> Result<(HashSet<u32>, u32), LabReplayValidationError> {
    let store = checkpoint
        .get("entities")
        .ok_or_else(|| invalid(format!("{label}.checkpointPayload missing entities")))?;
    let next_entity_id = required_u32(store, "nextId", label)?;
    if next_entity_id == 0 {
        return Err(invalid(format!(
            "{label}.checkpointPayload entities.nextId must be nonzero"
        )));
    }
    let entities = store
        .get("entities")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            invalid(format!(
                "{label}.checkpointPayload entities.entities must be an array"
            ))
        })?;
    let mut entity_ids = HashSet::new();
    for entity in entities {
        let entity_id = required_u32(entity, "id", label)?;
        if entity_id == 0 || !entity_ids.insert(entity_id) {
            return Err(invalid(format!(
                "{label}.checkpointPayload entities contain duplicate or zero id"
            )));
        }
        if entity_id >= next_entity_id {
            return Err(invalid(format!(
                "{label}.checkpointPayload entity id must be below entities.nextId"
            )));
        }
    }
    Ok((entity_ids, next_entity_id))
}

fn validate_lab_metadata(
    label: &str,
    vision: &LabVisionMode,
    god_mode_players: &[u32],
    player_ids: &HashSet<u32>,
    team_ids: &HashSet<TeamId>,
) -> Result<(), LabReplayValidationError> {
    match vision {
        LabVisionMode::FullWorld => {}
        LabVisionMode::Team { team_id } => {
            if !team_ids.contains(team_id) {
                return Err(invalid(format!(
                    "{label}.metadata.lab.vision has unknown teamId"
                )));
            }
        }
        LabVisionMode::Teams { team_ids: ids } => {
            if ids.is_empty() {
                return Err(invalid(format!(
                    "{label}.metadata.lab.vision teamIds must not be empty"
                )));
            }
            let mut seen = HashSet::new();
            for team_id in ids {
                if !seen.insert(*team_id) {
                    return Err(invalid(format!(
                        "{label}.metadata.lab.vision teamIds must not contain duplicates"
                    )));
                }
                if !team_ids.contains(team_id) {
                    return Err(invalid(format!(
                        "{label}.metadata.lab.vision has unknown teamId"
                    )));
                }
            }
        }
    }
    let mut seen = HashSet::new();
    for player_id in god_mode_players {
        if !seen.insert(*player_id) {
            return Err(invalid(format!(
                "{label}.metadata.lab.godModePlayers must not contain duplicates"
            )));
        }
        if !player_ids.contains(player_id) {
            return Err(invalid(format!(
                "{label}.metadata.lab.godModePlayers contains unknown player id"
            )));
        }
    }
    Ok(())
}

fn validate_source_entity_id_map(
    label: &str,
    id_map: &[LabScenarioEntityIdRemap],
    entity_ids: &HashSet<u32>,
) -> Result<(), LabReplayValidationError> {
    if id_map.len() > entity_ids.len() {
        return Err(invalid(format!(
            "{label}.metadata.sourceEntityIdMap has too many entries"
        )));
    }
    let mut old_ids = HashSet::new();
    let mut new_ids = HashSet::new();
    for remap in id_map {
        if remap.old_id == 0 || !old_ids.insert(remap.old_id) {
            return Err(invalid(format!(
                "{label}.metadata.sourceEntityIdMap contains duplicate or zero oldId"
            )));
        }
        if remap.new_id == 0 || !new_ids.insert(remap.new_id) {
            return Err(invalid(format!(
                "{label}.metadata.sourceEntityIdMap contains duplicate or zero newId"
            )));
        }
        if !entity_ids.contains(&remap.new_id) {
            return Err(invalid(format!(
                "{label}.metadata.sourceEntityIdMap newId must reference a checkpoint entity"
            )));
        }
    }
    Ok(())
}

fn validate_raw_operation_stream(
    value: &serde_json::Value,
) -> Result<(), LabReplayValidationError> {
    let Some(operations) = value
        .get("operations")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    if operations.len() > LAB_REPLAY_MAX_OPERATIONS {
        return Err(invalid(format!(
            "lab replay operation count must be at most {LAB_REPLAY_MAX_OPERATIONS}"
        )));
    }
    for entry in operations {
        let Some(op) = entry.get("op") else {
            continue;
        };
        let op_bytes = serde_json::to_vec(op).map_err(|err| {
            invalid(format!(
                "failed to measure raw lab replay operation payload: {err}"
            ))
        })?;
        if op_bytes.len() > LAB_REPLAY_MAX_OPERATION_JSON_BYTES {
            return Err(invalid(format!(
                "lab replay operation payload must be at most {LAB_REPLAY_MAX_OPERATION_JSON_BYTES} bytes"
            )));
        }
    }
    Ok(())
}

fn reject_unsupported_session_ops(
    value: &serde_json::Value,
) -> Result<(), LabReplayValidationError> {
    let Some(operations) = value
        .get("operations")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    for entry in operations {
        let Some(op_name) = entry
            .get("op")
            .and_then(|op| op.get("op"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        match op_name {
            "setVision" => {
                return Err(invalid(
                    "setVision is session projection metadata and is not part of the durable lab replay operation stream",
                ));
            }
            "importScenario" | "importCheckpointScenario" => {
                return Err(invalid(
                    "checkpoint setup imports must rebase initialSetup and clear prior lab replay operations to avoid ambiguous entity id remaps",
                ));
            }
            "exportScenario" | "validateScenario" | "submitScenario" => {
                return Err(invalid(
                    "lab scenario authoring/export requests are UI control-plane operations, not durable lab replay operations",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_unit_list(
    units: &[u32],
    cap: usize,
    label: &str,
    state: &ValidationState,
) -> Result<(), LabReplayValidationError> {
    if units.is_empty() {
        return Err(invalid(format!("{label} must not be empty")));
    }
    if units.len() > cap {
        return Err(invalid(format!("{label} must contain at most {cap} ids")));
    }
    for &entity_id in units {
        validate_entity_id(entity_id, label, state)?;
    }
    Ok(())
}

fn validate_player_id(
    player_id: u32,
    label: &str,
    state: &ValidationState,
) -> Result<(), LabReplayValidationError> {
    if !state.player_ids.contains(&player_id) {
        return Err(invalid(format!(
            "{label} references unknown player id {player_id}"
        )));
    }
    Ok(())
}

fn validate_entity_id(
    entity_id: u32,
    label: &str,
    state: &ValidationState,
) -> Result<(), LabReplayValidationError> {
    if !state.entity_ids_precise {
        if entity_id == 0 {
            return Err(invalid(format!("{label} must be nonzero")));
        }
        return Ok(());
    }
    if !state.entity_ids.contains(&entity_id) {
        return Err(invalid(format!(
            "{label} references stale entity id {entity_id}"
        )));
    }
    Ok(())
}

fn validate_finite_point(x: f32, y: f32, label: &str) -> Result<(), LabReplayValidationError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(invalid(format!("{label} coordinates must be finite")));
    }
    Ok(())
}

fn validate_non_empty_len(
    label: &str,
    value: &str,
    max: usize,
) -> Result<(), LabReplayValidationError> {
    if value.trim().is_empty() || value.len() > max {
        return Err(invalid(format!(
            "{label} must be non-empty and at most {max} bytes"
        )));
    }
    Ok(())
}

fn validate_optional_len(
    label: &str,
    value: &str,
    max: usize,
) -> Result<(), LabReplayValidationError> {
    if value.len() > max {
        return Err(invalid(format!("{label} must be at most {max} bytes")));
    }
    Ok(())
}

fn required_str<'a>(
    object: &'a serde_json::Value,
    field: &str,
    label: &str,
) -> Result<&'a str, LabReplayValidationError> {
    object
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| invalid(format!("{label}.{field} must be a string")))
}

fn required_u32(
    object: &serde_json::Value,
    field: &str,
    label: &str,
) -> Result<u32, LabReplayValidationError> {
    let value = object
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| invalid(format!("{label}.{field} must be a u32")))?;
    u32::try_from(value).map_err(|_| invalid(format!("{label}.{field} must be a u32")))
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn invalid(message: impl Into<String>) -> LabReplayValidationError {
    LabReplayValidationError::new(message)
}

#[cfg(test)]
mod tests;
