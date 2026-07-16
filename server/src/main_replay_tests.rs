use super::*;
use rts_rules::faction::{DEFAULT_FACTION_ID, EMPTY_FIXTURE_FACTION_ID};

#[test]
fn observation_run_id_validation_is_bounded_and_uses_log_safe_characters() {
    assert!(valid_observation_run_id("ai-selfplay-1740000000000-000001"));
    assert!(valid_observation_run_id("observer_run-42"));
    assert!(!valid_observation_run_id(""));
    assert!(!valid_observation_run_id("has space"));
    assert!(!valid_observation_run_id("has/slash"));
    assert!(!valid_observation_run_id(&"a".repeat(97)));
}

fn replay_summary_for(
    meta: Option<rts_server::db::ReplaySummaryMetadata>,
) -> rts_server::db::MatchSummary {
    rts_server::db::MatchSummary {
        id: 1,
        replay_number: None,
        match_run_id: None,
        started_at: chrono::Utc::now(),
        ended_at: chrono::Utc::now(),
        duration_ms: 1_000,
        map_name: "Chokes".to_string(),
        winner_name: Some("Alpha".to_string()),
        outcome: "win".to_string(),
        participants: vec!["Alpha".to_string(), "Bravo".to_string()],
        score_screen: serde_json::Value::Array(Vec::new()),
        human_count: 2,
        debug_mode: false,
        local_only: false,
        replay_available: meta.is_some(),
        replay_unavailable_reason: None,
        replay_metadata: meta,
    }
}

fn replay_artifact_for_faction(faction_id: &str) -> ReplayArtifactV1 {
    let players = vec![rts_sim::game::PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: faction_id.to_string(),
        name: "Replay".to_string(),
        color: "#ffffff".to_string(),
        is_ai: false,
    }];
    let game = rts_sim::game::Game::new(&players, 0x5150_030d);
    let replay_start = replay::ReplayStartComposition::capture(&game, "current-build").unwrap();
    replay_start.finalize(&game, None, game.scores())
}

#[test]
fn replay_summary_marks_current_build_and_map_available() {
    let map = Map::metadata_for_name("Chokes").unwrap();
    let mut row = replay_summary_for(Some(rts_server::db::ReplaySummaryMetadata {
        artifact_schema_version: replay::REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION as i32,
        build_sha: "current-build".to_string(),
        map_name: map.name,
        map_schema_version: map.schema_version as i32,
        map_hash: map.content_hash,
    }));

    apply_replay_summary_compatibility(&mut row, "current-build");

    assert!(row.replay_available);
    assert_eq!(row.replay_unavailable_reason, None);
}

#[test]
fn replay_summary_warns_but_allows_incompatible_build() {
    let map = Map::metadata_for_name("Chokes").unwrap();
    let mut row = replay_summary_for(Some(rts_server::db::ReplaySummaryMetadata {
        artifact_schema_version: replay::REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION as i32,
        build_sha: "old-build".to_string(),
        map_name: map.name,
        map_schema_version: map.schema_version as i32,
        map_hash: map.content_hash,
    }));

    apply_replay_summary_compatibility(&mut row, "new-build");

    assert!(row.replay_available);
    assert_eq!(
        row.replay_unavailable_reason.as_deref(),
        Some(
            "Replay Potentially Incompatible With Current Server (server: new-build, replay: old-build)"
        )
    );
}

#[test]
fn replay_summary_rejects_map_drift_before_build_warning() {
    let map = Map::metadata_for_name("Chokes").unwrap();
    let mut row = replay_summary_for(Some(rts_server::db::ReplaySummaryMetadata {
        artifact_schema_version: replay::REPLAY_ARTIFACT_CURRENT_SCHEMA_VERSION as i32,
        build_sha: "old-build".to_string(),
        map_name: map.name,
        map_schema_version: map.schema_version as i32,
        map_hash: "changed-map-hash".to_string(),
    }));

    apply_replay_summary_compatibility(&mut row, "new-build");

    assert!(!row.replay_available);
    assert_eq!(
        row.replay_unavailable_reason.as_deref(),
        Some("Replay map \"Chokes\" has changed on this server.")
    );
}

#[test]
fn replay_summary_explains_missing_artifact() {
    let mut row = replay_summary_for(None);

    apply_replay_summary_compatibility(&mut row, "current-build");

    assert!(!row.replay_available);
    assert_eq!(
        row.replay_unavailable_reason.as_deref(),
        Some("Replay was not recorded for this match.")
    );
}

#[test]
fn match_history_replay_launch_accepts_checkpoint_backed_kriegsia_factions() {
    let artifact = replay_artifact_for_faction(DEFAULT_FACTION_ID);

    assert_eq!(
        replay_incompatibility_reason(&artifact, "current-build"),
        None
    );
}

#[test]
fn match_history_replay_launch_rejects_invalid_checkpoint_start() {
    let artifact = replay_artifact_for_faction(DEFAULT_FACTION_ID);
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

    let err = replay_incompatibility_reason(&artifact, "current-build")
        .expect("invalid checkpoint start should reject persisted replay launch");

    assert!(
        err.contains("checkpoint-backed replay start state is invalid"),
        "unexpected reject: {err}"
    );
}

#[test]
fn match_history_replay_launch_rejects_unsupported_or_fixture_factions() {
    let mut unknown = replay_artifact_for_faction(DEFAULT_FACTION_ID);
    unknown.players[0].faction_id = "unknown-faction".to_string();
    let err = replay_incompatibility_reason(&unknown, "current-build")
        .expect("unsupported future faction should reject");
    assert!(err.contains("unknown faction"), "unexpected reject: {err}");

    let fixture = replay_artifact_for_faction(EMPTY_FIXTURE_FACTION_ID);
    let err = replay_incompatibility_reason(&fixture, "current-build")
        .expect("fixture faction should reject persisted replay launch");
    assert!(err.contains("fixture-only"), "unexpected reject: {err}");
}

#[test]
fn match_history_replay_launch_rejects_unknown_player_loadout() {
    let mut artifact = replay_artifact_for_faction(DEFAULT_FACTION_ID);
    let mut extra_loadout = artifact.player_loadouts[0].clone();
    extra_loadout.player_id = 999;
    artifact.player_loadouts.push(extra_loadout);

    let err = replay_incompatibility_reason(&artifact, "current-build")
        .expect("unknown-player replay loadout should reject");
    assert!(
        err.contains("unknown player 999"),
        "unexpected reject: {err}"
    );
}
