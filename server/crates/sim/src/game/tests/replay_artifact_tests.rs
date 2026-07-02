use super::*;
use crate::game::replay::{
    replay_commands, ReplayArtifactV1, ReplayStartComposition, ReplayValidationError,
    REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION,
};
use crate::protocol::DEFAULT_FACTION_ID;

fn players() -> [PlayerInit; 1] {
    [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: DEFAULT_FACTION_ID.to_string(),
        name: "Replay".into(),
        color: "#fff".into(),
        is_ai: false,
    }]
}

fn start_checkpoint_payload(artifact: &ReplayArtifactV1) -> String {
    serde_json::to_value(artifact).unwrap()["startState"]["checkpointPayload"]
        .as_str()
        .expect("schema 3 artifact should contain startState.checkpointPayload")
        .to_string()
}

#[test]
fn replay_commands_preserves_explicit_player_loadout_resources() {
    let players = players();
    let starting_loadouts = [PlayerStartingLoadout {
        player_id: 1,
        faction_id: DEFAULT_FACTION_ID.to_string(),
        loadout_id: "kriegsia.standard".to_string(),
        starting_steel: 99_999,
        starting_oil: 88_888,
    }];
    let outcome = replay_commands(&players, &[], 0, 0x1234_5678, &starting_loadouts)
        .expect("replay should succeed");
    let snapshot = &outcome.final_snapshots[0].snapshot;

    assert_eq!(snapshot.steel, 99_999);
    assert_eq!(snapshot.oil, 88_888);
}

#[test]
fn replay_artifact_captures_live_game_contract() {
    let players = players();
    let mut game = Game::new_with_starting_resources(&players, 777, 333, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, "test-sha")
        .expect("tick-zero replay start should export");
    game.tick();
    let scores = game.scores();

    let artifact = replay_start.finalize(&game, Some(1), scores);

    assert_eq!(
        artifact.artifact_schema_version,
        REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION
    );
    assert_eq!(artifact.server_build_sha, "test-sha");
    assert_eq!(artifact.map_name, "Default");
    assert_eq!(artifact.seed, 0x1234_5678);
    assert_eq!(artifact.player_loadouts.len(), 1);
    assert_eq!(artifact.player_loadouts[0].starting_steel, 777);
    assert_eq!(artifact.player_loadouts[0].starting_oil, 333);
    assert_eq!(artifact.player_loadouts[0].loadout_id, "kriegsia.standard");
    assert_eq!(artifact.players, players);
    assert_eq!(artifact.players[0].faction_id, DEFAULT_FACTION_ID);
    assert_eq!(artifact.duration_ticks, 1);
    let checkpoint: serde_json::Value =
        serde_json::from_str(&start_checkpoint_payload(&artifact)).expect("valid checkpoint JSON");
    assert_eq!(checkpoint["schema"], "rts.gameCheckpoint");
    assert_eq!(checkpoint["version"], 1);
    assert_eq!(checkpoint["compatibility"]["createdBy"], "replay");
    assert_eq!(checkpoint["tick"], 0);
    assert_eq!(
        checkpoint["commandLog"].as_array().map(Vec::len),
        Some(0),
        "launch-time checkpoint must not be exported from the final game command log"
    );
    assert_eq!(artifact.winner_id, Some(1));
    assert_eq!(artifact.winner_team_id, Some(1));
    assert!(!artifact.final_scores.is_empty());
    assert_eq!(artifact.start_metadata().duration_ticks, 1);
}

#[test]
fn replay_start_capture_rejects_pending_commands() {
    let players = players();
    let mut game = Game::new(&players, 0x1234_5678);
    game.enqueue(1, Command::Stop { units: vec![1] });

    let err = ReplayStartComposition::capture(&game, "test-sha")
        .expect_err("pending commands should reject replay start capture");

    assert!(
        err.contains("pending commands"),
        "unexpected replay start capture error: {err}"
    );
}

#[test]
fn replay_artifact_requires_faction_schema_and_defaults_missing_winner_team() {
    let json = serde_json::json!({
        "artifactSchemaVersion": 3,
        "serverBuildSha": "test-sha",
        "mapName": "Default",
        "mapSchemaVersion": 1,
        "mapContentHash": "hash",
        "seed": 1,
        "playerLoadouts": [{
            "playerId": 1,
            "factionId": DEFAULT_FACTION_ID,
            "loadoutId": "kriegsia.standard",
            "startingSteel": 75,
            "startingOil": 0
        }],
        "players": [{
            "id": 1,
            "team_id": 1,
            "faction_id": DEFAULT_FACTION_ID,
            "name": "Replay",
            "color": "#fff",
            "is_ai": false
        }],
        "durationTicks": 0,
        "commandLog": [],
        "winnerId": 1,
        "finalScores": []
    });

    let artifact: ReplayArtifactV1 = serde_json::from_value(json).unwrap();

    assert_eq!(artifact.players[0].team_id, 1);
    assert_eq!(artifact.players[0].faction_id, DEFAULT_FACTION_ID);
    assert_eq!(artifact.winner_id, Some(1));
    assert_eq!(artifact.winner_team_id, None);

    let old_json = serde_json::json!({
        "artifactSchemaVersion": 1,
        "serverBuildSha": "test-sha",
        "mapName": "Default",
        "mapSchemaVersion": 1,
        "mapContentHash": "hash",
        "seed": 1,
        "players": [{
            "id": 1,
            "team_id": 1,
            "name": "Replay",
            "color": "#fff",
            "is_ai": false
        }],
        "durationTicks": 0,
        "commandLog": [],
        "winnerId": 1,
        "finalScores": []
    });
    assert!(
        serde_json::from_value::<ReplayArtifactV1>(old_json).is_err(),
        "pre-faction replay artifacts without player factionId should not load"
    );
}

#[test]
fn replay_validation_rejects_incompatible_build_and_map_metadata() {
    let players = players();
    let game = Game::new(&players, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, "sha-a").unwrap();
    let artifact = replay_start.finalize(&game, None, game.scores());
    let map = game.map_metadata().clone();

    assert!(matches!(
        artifact.validate_against("sha-b", &map),
        Err(ReplayValidationError::BuildShaMismatch { .. })
    ));

    let mut missing = map.clone();
    missing.name = "Other".to_string();
    assert!(matches!(
        artifact.validate_against("sha-a", &missing),
        Err(ReplayValidationError::MapMissing { .. })
    ));

    let mut wrong_schema = map.clone();
    wrong_schema.schema_version = wrong_schema.schema_version.saturating_add(1);
    assert!(matches!(
        artifact.validate_against("sha-a", &wrong_schema),
        Err(ReplayValidationError::MapSchemaMismatch { .. })
    ));

    let mut wrong_hash = map;
    wrong_hash.content_hash.push_str("-changed");
    assert!(matches!(
        artifact.validate_against("sha-a", &wrong_hash),
        Err(ReplayValidationError::MapHashMismatch { .. })
    ));

    let mut wrong_hash_and_build = wrong_hash.clone();
    wrong_hash_and_build.content_hash.push_str("-again");
    assert!(matches!(
        artifact.validate_against("sha-b", &wrong_hash_and_build),
        Err(ReplayValidationError::MapHashMismatch { .. })
    ));
}

#[test]
fn replay_artifact_schema_two_is_intentionally_rejected() {
    let players = players();
    let game = Game::new(&players, 0x1234_5678);
    let map = game.map_metadata().clone();
    let json = serde_json::json!({
        "artifactSchemaVersion": 2,
        "serverBuildSha": "sha-a",
        "mapName": map.name,
        "mapSchemaVersion": map.schema_version,
        "mapContentHash": map.content_hash,
        "seed": game.seed(),
        "playerLoadouts": game.starting_loadouts(),
        "players": game.player_inits(),
        "durationTicks": game.tick_count(),
        "commandLog": game.command_log(),
        "winnerId": null,
        "winnerTeamId": null,
        "finalScores": game.scores(),
    });
    let artifact: ReplayArtifactV1 = serde_json::from_value(json).unwrap();

    assert!(matches!(
        artifact.validate_against("sha-a", game.map_metadata()),
        Err(ReplayValidationError::UnsupportedArtifactSchema {
            found: 2,
            ..
        })
    ));
    assert!(serde_json::to_value(&artifact).unwrap().get("startState").is_none());
}

#[test]
fn checkpoint_backed_replay_rejects_wrong_materialized_map_binding() {
    let players = players();
    let game = Game::new(&players, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, "sha-a").unwrap();
    let artifact = replay_start.finalize(&game, None, game.scores());
    let mut artifact_json = serde_json::to_value(&artifact).unwrap();
    let checkpoint_payload = artifact_json["startState"]["checkpointPayload"]
        .as_str()
        .unwrap();
    let mut checkpoint: serde_json::Value = serde_json::from_str(checkpoint_payload).unwrap();
    checkpoint["mapBinding"]["materializedMapHash"] =
        serde_json::Value::String("wrong-map".to_string());
    artifact_json["startState"]["checkpointPayload"] =
        serde_json::Value::String(serde_json::to_string(&checkpoint).unwrap());
    let artifact: ReplayArtifactV1 = serde_json::from_value(artifact_json).unwrap();

    let err = match artifact.restore_start_game(game.state.map.clone(), game.map_metadata().clone())
    {
        Ok(_) => panic!("wrong materialized map hash should reject replay restore"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        ReplayValidationError::CheckpointStartInvalid { .. }
    ));
}

#[test]
fn checkpoint_backed_replay_rejects_top_level_start_metadata_mismatch() {
    let players = players();
    let game = Game::new(&players, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, "sha-a").unwrap();
    let mut artifact = replay_start.finalize(&game, None, game.scores());
    artifact.players[0].name = "Tampered".to_string();

    let err = match artifact.restore_start_game(game.state.map.clone(), game.map_metadata().clone())
    {
        Ok(_) => panic!("mismatched top-level player metadata should reject replay restore"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        ReplayValidationError::StartStateMismatch { field: "players" }
    ));
}

#[test]
fn checkpoint_backed_replay_rejects_checkpoint_seed_mismatch() {
    let players = players();
    let game = Game::new(&players, 0x1234_5678);
    let replay_start = ReplayStartComposition::capture(&game, "sha-a").unwrap();
    let artifact = replay_start.finalize(&game, None, game.scores());
    let mut artifact_json = serde_json::to_value(&artifact).unwrap();
    let checkpoint_payload = artifact_json["startState"]["checkpointPayload"]
        .as_str()
        .unwrap();
    let mut checkpoint: serde_json::Value = serde_json::from_str(checkpoint_payload).unwrap();
    checkpoint["seed"] = serde_json::Value::Number(0x8765_4321u64.into());
    checkpoint["rng"]["seed"] = serde_json::Value::Number(0x8765_4321u64.into());
    artifact_json["startState"]["checkpointPayload"] =
        serde_json::Value::String(serde_json::to_string(&checkpoint).unwrap());
    let artifact: ReplayArtifactV1 = serde_json::from_value(artifact_json).unwrap();

    let err = match artifact.restore_start_game(game.state.map.clone(), game.map_metadata().clone())
    {
        Ok(_) => panic!("checkpoint seed drift should reject replay restore"),
        Err(err) => err,
    };

    assert!(matches!(
        err,
        ReplayValidationError::StartStateMismatch {
            field: "checkpointSeed"
        }
    ));
}
