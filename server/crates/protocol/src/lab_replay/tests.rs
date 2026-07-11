use super::*;
use crate::{
    LabCheckpointScenarioMap, LabCheckpointScenarioMapData, LabCheckpointScenarioMetadata,
    LabScenarioLabMetadata, LabScenarioTile,
};
use serde_json::json;

fn checkpoint_payload(
    content_hash: &str,
    materialized_hash: &str,
    seed: u32,
    tick: u32,
    players: &[(u32, TeamId)],
    entity_ids: &[u32],
    next_id: u32,
) -> String {
    serde_json::to_string(&json!({
        "schema": "rts.gameCheckpoint",
        "version": 1,
        "mapBinding": {
            "name": "Default",
            "schemaVersion": 2,
            "contentHash": content_hash,
            "materializedMapHash": materialized_hash,
            "size": 2,
            "playerCount": players.len(),
        },
        "seed": seed,
        "tick": tick,
        "players": players.iter().map(|(id, team_id)| {
            json!({ "id": id, "teamId": team_id })
        }).collect::<Vec<_>>(),
        "entities": {
            "nextId": next_id,
            "entities": entity_ids.iter().map(|id| json!({ "id": id })).collect::<Vec<_>>(),
        },
    }))
    .unwrap()
}

fn checkpoint_scenario(entity_ids: &[u32], next_id: u32) -> LabCheckpointScenarioV1 {
    let content_hash = "content-hash";
    let materialized_hash = "materialized-hash";
    let seed = 1234;
    let tick = 0;
    let players = [(1, 1), (2, 2)];
    LabCheckpointScenarioV1 {
        schema_version: 1,
        kind: "labCheckpointScenario".to_string(),
        name: "Initial lab setup".to_string(),
        seed,
        map: LabCheckpointScenarioMap {
            name: "Default".to_string(),
            schema_version: 2,
            content_hash: content_hash.to_string(),
            materialized_hash: materialized_hash.to_string(),
            data: LabCheckpointScenarioMapData {
                size: 2,
                terrain: vec![terrain::GRASS; 4],
                starts: vec![
                    LabScenarioTile { x: 0, y: 0 },
                    LabScenarioTile { x: 1, y: 1 },
                ],
                expansion_sites: Vec::new(),
            },
        },
        metadata: LabCheckpointScenarioMetadata {
            exported_tick: tick,
            lab: LabScenarioLabMetadata {
                vision: LabVisionMode::FullWorld,
                god_mode_players: Vec::new(),
                initial_camera: None,
            },
            source_scenario: None,
            source_entity_id_map: entity_ids
                .iter()
                .map(|id| LabScenarioEntityIdRemap {
                    old_id: *id,
                    new_id: *id,
                })
                .collect(),
        },
        checkpoint_payload: checkpoint_payload(
            content_hash,
            materialized_hash,
            seed,
            tick,
            &players,
            entity_ids,
            next_id,
        ),
    }
}

fn valid_artifact() -> LabReplayArtifactV1 {
    LabReplayArtifactV1 {
        schema: LAB_REPLAY_ARTIFACT_SCHEMA.to_string(),
        schema_version: LAB_REPLAY_ARTIFACT_SCHEMA_VERSION,
        kind: LAB_REPLAY_ARTIFACT_KIND.to_string(),
        server_build_sha: "test-build".to_string(),
        authoring: LabReplayAuthoringMetadata {
            name: "Portable lab replay".to_string(),
            author: Some("Contract test".to_string()),
            created_at_unix_ms: Some(1_700_000_000_000),
            description: Some("Exercise the durable lab replay artifact contract.".to_string()),
            tags: vec!["test".to_string()],
        },
        initial_setup: checkpoint_scenario(&[1, 2], 3),
        timeline: LabReplayTimelineMetadata {
            initial_tick: 0,
            duration_ticks: 30,
            keyframe_interval_ticks: LAB_REPLAY_TIMELINE_KEYFRAME_INTERVAL_TICKS,
        },
        operations: vec![
            LabReplayOperationEntry {
                sequence: 0,
                tick: 0,
                request_id: 1,
                operator_id: 100,
                op: LabReplayOperation::SetPlayerResources {
                    player_id: 1,
                    steel: 900,
                    oil: 300,
                },
            },
            LabReplayOperationEntry {
                sequence: 1,
                tick: 5,
                request_id: 2,
                operator_id: 100,
                op: LabReplayOperation::SpawnEntity {
                    owner: 1,
                    kind: "rifleman".to_string(),
                    x: 64.0,
                    y: 96.0,
                    completed: true,
                },
            },
            LabReplayOperationEntry {
                sequence: 2,
                tick: 6,
                request_id: 3,
                operator_id: 100,
                op: LabReplayOperation::MoveEntity {
                    entity_id: 3,
                    x: 128.0,
                    y: 160.0,
                },
            },
            LabReplayOperationEntry {
                sequence: 3,
                tick: 7,
                request_id: 4,
                operator_id: 100,
                op: LabReplayOperation::IssueCommandAs {
                    player_id: 1,
                    cmd: Command::Move {
                        units: vec![3],
                        x: 192.0,
                        y: 224.0,
                        queued: false,
                    },
                    ignore_command_limits: false,
                },
            },
        ],
    }
}

fn validate_value(
    value: serde_json::Value,
) -> Result<LabReplayArtifactV1, LabReplayValidationError> {
    let bytes = serde_json::to_vec(&value).unwrap();
    lab_replay_artifact_from_slice(&bytes)
}

#[test]
fn lab_replay_artifact_round_trips_and_validates() {
    let artifact = valid_artifact();
    let bytes = serde_json::to_vec(&artifact).unwrap();

    let parsed = lab_replay_artifact_from_slice(&bytes).expect("valid artifact");

    assert_eq!(parsed, artifact);
}

#[test]
fn lab_replay_artifact_rejects_malformed_kind() {
    let mut artifact = valid_artifact();
    artifact.kind = "labScenario".to_string();

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("wrong artifact kind should fail");

    assert!(err.to_string().contains("kind must be labReplay"));
}

#[test]
fn lab_replay_artifact_rejects_excessive_whole_artifact_size() {
    let bytes = vec![b' '; LAB_REPLAY_MAX_ARTIFACT_BYTES + 1];

    let err = lab_replay_artifact_from_slice(&bytes)
        .expect_err("oversized artifact should fail before parse");

    assert!(err.to_string().contains("at most"));
}

#[test]
fn lab_replay_artifact_rejects_too_many_entries() {
    let mut artifact = valid_artifact();
    artifact.operations = (0..=LAB_REPLAY_MAX_OPERATIONS)
        .map(|index| LabReplayOperationEntry {
            sequence: index as u64,
            tick: 0,
            request_id: index as u32 + 1,
            operator_id: 100,
            op: LabReplayOperation::SetPlayerGodMode {
                player_id: 1,
                enabled: index % 2 == 0,
            },
        })
        .collect();

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("entry cap should fail");

    assert!(err.to_string().contains("operation count"));
}

#[test]
fn lab_replay_artifact_rejects_oversized_nested_checkpoint_payload() {
    let mut artifact = valid_artifact();
    artifact.initial_setup.checkpoint_payload =
        " ".repeat(LAB_REPLAY_MAX_CHECKPOINT_PAYLOAD_BYTES + 1);

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("nested checkpoint cap should fail");

    assert!(err.to_string().contains("checkpointPayload"));
}

#[test]
fn lab_replay_artifact_rejects_oversized_raw_operation_payload() {
    let mut value = serde_json::to_value(valid_artifact()).unwrap();
    value["operations"][0]["op"]["ignoredPayload"] =
        json!("x".repeat(LAB_REPLAY_MAX_OPERATION_JSON_BYTES + 1));

    let err = validate_value(value).expect_err("raw entry cap should fail");

    assert!(err.to_string().contains("operation payload"));
}

#[test]
fn lab_replay_artifact_rejects_unknown_operation_fields() {
    let mut value = serde_json::to_value(valid_artifact()).unwrap();
    value["operations"][0]["op"]["ignoredPayload"] = json!("small");

    let err = validate_value(value).expect_err("unknown op fields should fail");

    assert!(err.to_string().contains("unknown field"));
}

#[test]
fn lab_replay_artifact_rejects_stale_entity_ids() {
    let mut artifact = valid_artifact();
    artifact.operations[0].op = LabReplayOperation::MoveEntity {
        entity_id: 999,
        x: 1.0,
        y: 1.0,
    };

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("stale entity should fail");

    assert!(err.to_string().contains("stale entity id 999"));
}

#[test]
fn lab_replay_artifact_allows_command_created_entity_references_after_ticks() {
    let mut artifact = valid_artifact();
    artifact.initial_setup = checkpoint_scenario(&[1], 2);
    artifact.timeline.duration_ticks = 12;
    artifact.operations = vec![
        LabReplayOperationEntry {
            sequence: 0,
            tick: 0,
            request_id: 1,
            operator_id: 100,
            op: LabReplayOperation::IssueCommandAs {
                player_id: 1,
                cmd: Command::Train {
                    building: 1,
                    unit: "rifleman".to_string(),
                },
                ignore_command_limits: false,
            },
        },
        LabReplayOperationEntry {
            sequence: 1,
            tick: 12,
            request_id: 2,
            operator_id: 100,
            op: LabReplayOperation::IssueCommandAs {
                player_id: 1,
                cmd: Command::Move {
                    units: vec![2],
                    x: 64.0,
                    y: 64.0,
                    queued: false,
                },
                ignore_command_limits: false,
            },
        },
    ];

    let parsed = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect("command-created entity references after simulated ticks should validate");

    assert_eq!(parsed.operations.len(), 2);
}

#[test]
fn lab_replay_artifact_still_rejects_same_tick_unknown_entity_ids() {
    let mut artifact = valid_artifact();
    artifact.initial_setup = checkpoint_scenario(&[1], 2);
    artifact.operations = vec![
        LabReplayOperationEntry {
            sequence: 0,
            tick: 0,
            request_id: 1,
            operator_id: 100,
            op: LabReplayOperation::IssueCommandAs {
                player_id: 1,
                cmd: Command::Train {
                    building: 1,
                    unit: "rifleman".to_string(),
                },
                ignore_command_limits: false,
            },
        },
        LabReplayOperationEntry {
            sequence: 1,
            tick: 0,
            request_id: 2,
            operator_id: 100,
            op: LabReplayOperation::MoveEntity {
                entity_id: 2,
                x: 64.0,
                y: 64.0,
            },
        },
    ];

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("same-tick unknown entity should still fail");

    assert!(err.to_string().contains("stale entity id 2"));
}

#[test]
fn lab_replay_artifact_rejects_bad_player_ids() {
    let mut artifact = valid_artifact();
    artifact.operations[0].op = LabReplayOperation::SetPlayerResources {
        player_id: 99,
        steel: 1,
        oil: 1,
    };

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("bad player id should fail");

    assert!(err.to_string().contains("unknown player id 99"));
}

#[test]
fn lab_replay_artifact_rejects_map_checkpoint_mismatch() {
    let mut artifact = valid_artifact();
    let mut checkpoint: serde_json::Value =
        serde_json::from_str(&artifact.initial_setup.checkpoint_payload).unwrap();
    checkpoint["mapBinding"]["contentHash"] = json!("wrong-hash");
    artifact.initial_setup.checkpoint_payload = serde_json::to_string(&checkpoint).unwrap();

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("map binding mismatch should fail");

    assert!(err.to_string().contains("contentHash mismatch"));
}

#[test]
fn lab_replay_artifact_rejects_checkpoint_player_count_start_mismatch() {
    let mut artifact = valid_artifact();
    artifact.initial_setup.map.data.starts.pop();

    let err = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect_err("map starts must match checkpoint player count");

    assert!(err
        .to_string()
        .contains("playerCount must match map starts"));
}

#[test]
fn lab_replay_artifact_rejects_nonfinite_set_rally_coordinates() {
    let mut artifact = valid_artifact();
    artifact.operations[0].op = LabReplayOperation::IssueCommandAs {
        player_id: 1,
        cmd: Command::SetRally {
            building: 1,
            x: f32::NAN,
            y: 10.0,
            kind: Some("move".to_string()),
            queued: false,
        },
        ignore_command_limits: false,
    };

    let err = validate_lab_replay_artifact(&artifact).expect_err("NaN rally should fail");

    assert!(err.to_string().contains("command.setRally coordinates"));
}

#[test]
fn lab_replay_artifact_rejects_unsupported_vision_metadata_operation() {
    let mut value = serde_json::to_value(valid_artifact()).unwrap();
    value["operations"][0]["op"] = json!({
        "op": "setVision",
        "vision": { "mode": "fullWorld" },
    });

    let err = validate_value(value).expect_err("setVision should be excluded");

    assert!(err
        .to_string()
        .contains("setVision is session projection metadata"));
}

#[test]
fn lab_replay_artifact_rejects_setup_import_id_remap_ambiguity() {
    let mut value = serde_json::to_value(valid_artifact()).unwrap();
    let setup = value["initialSetup"].clone();
    value["operations"][0]["op"] = json!({
        "op": "importCheckpointScenario",
        "scenario": setup,
        "entityIdMap": [],
    });

    let err = validate_value(value).expect_err("setup imports must rebase");

    assert!(err.to_string().contains("rebase initialSetup"));
}

#[test]
fn plural_lab_replay_operations_round_trip_and_legacy_singular_remains_readable() {
    let mut artifact = valid_artifact();
    artifact.initial_setup = checkpoint_scenario(&[1, 2], 3);
    artifact.operations = vec![
        LabReplayOperationEntry {
            sequence: 0,
            tick: 0,
            request_id: 1,
            operator_id: 100,
            op: LabReplayOperation::SpawnEntities {
                spawns: vec![LabSpawnEntitySpec {
                    owner: 1,
                    kind: "rifleman".to_string(),
                    x: 100.0,
                    y: 100.0,
                    completed: true,
                }],
            },
        },
        LabReplayOperationEntry {
            sequence: 1,
            tick: 0,
            request_id: 2,
            operator_id: 100,
            op: LabReplayOperation::ApplyUpdates {
                updates: vec![LabUpdateSpec::Move {
                    entity_id: 3,
                    x: 120.0,
                    y: 100.0,
                }],
            },
        },
        LabReplayOperationEntry {
            sequence: 2,
            tick: 0,
            request_id: 3,
            operator_id: 100,
            op: LabReplayOperation::DeleteEntities {
                entity_ids: vec![3],
            },
        },
        LabReplayOperationEntry {
            sequence: 3,
            tick: 0,
            request_id: 4,
            operator_id: 100,
            op: LabReplayOperation::MoveEntity {
                entity_id: 1,
                x: 64.0,
                y: 64.0,
            },
        },
    ];

    let parsed = lab_replay_artifact_from_slice(&serde_json::to_vec(&artifact).unwrap())
        .expect("plural and legacy singular operations should parse together");
    assert!(matches!(
        parsed.operations[0].op,
        LabReplayOperation::SpawnEntities { .. }
    ));
    assert!(matches!(
        parsed.operations[3].op,
        LabReplayOperation::MoveEntity { .. }
    ));
}

#[test]
fn plural_lab_replay_operations_enforce_400_item_limit() {
    let mut artifact = valid_artifact();
    artifact.operations[0].op = LabReplayOperation::SpawnEntities {
        spawns: (0..401)
            .map(|_| LabSpawnEntitySpec {
                owner: 1,
                kind: "rifleman".to_string(),
                x: 100.0,
                y: 100.0,
                completed: true,
            })
            .collect(),
    };
    let err = validate_lab_replay_artifact(&artifact).expect_err("401-item replay batch");
    assert!(err.to_string().contains("1 to 400"));
}
