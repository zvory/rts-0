use super::support::*;
use crate::lab_scenario_submission::{
    LabScenarioPrRequest, LabScenarioSubmissionService, LabScenarioSubmissionSuccess,
    ScenarioPrBackend, ScenarioPrFuture, LAB_SCENARIO_SUBMISSION_MANIFEST_PATH,
};
use rts_rules::{faction::CURRENT_CATALOG, EntityKind};
use rts_sim::game::lab::LabOp;
use std::sync::{Arc, Mutex as StdMutex};

#[derive(Clone)]
struct RecordingSubmissionBackend {
    captured: Arc<StdMutex<Vec<LabScenarioPrRequest>>>,
}

impl ScenarioPrBackend for RecordingSubmissionBackend {
    fn create_draft_pr(&self, request: LabScenarioPrRequest) -> ScenarioPrFuture {
        let captured = self.captured.clone();
        Box::pin(async move {
            let scenario_path = request
                .files
                .iter()
                .find(|file| file.path != LAB_SCENARIO_SUBMISSION_MANIFEST_PATH)
                .map(|file| file.path.clone())
                .unwrap_or_default();
            let success = LabScenarioSubmissionSuccess {
                pr_url: "https://github.com/example/rts/pull/17".to_string(),
                branch_name: request.branch_name.clone(),
                scenario_path,
                manifest_path: LAB_SCENARIO_SUBMISSION_MANIFEST_PATH.to_string(),
            };
            captured.lock().unwrap().push(request);
            Ok(success)
        })
    }
}

fn authoring_metadata(slug: &str) -> crate::protocol::LabScenarioAuthoringMetadata {
    crate::protocol::LabScenarioAuthoringMetadata {
        slug: slug.to_string(),
        name: "Submitted Lab Scenario".to_string(),
        title: "Submitted Lab Scenario".to_string(),
        description: "A submitted lab scenario for room-task tests.".to_string(),
        tags: vec!["test".to_string()],
        review_notes: Some("Verify the submitted state.".to_string()),
    }
}

#[test]
fn lab_start_payload_initial_operator_uses_policy_metadata() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(drain.active_matches(), 1);
    assert!(task.match_tracked_for_drain);
    assert_eq!(task.match_player_count, 2);
    assert_eq!(task.match_human_count, 0);
    assert!(!task.session_policy().allows_match_history());
    let session = task.lab_session.as_ref().expect("lab session");
    assert_eq!(session.operator_id, 99);
    assert_eq!(session.role_for(99), LabStartRole::Operator);

    let starts = start_payloads(&mut writer);
    assert_eq!(starts.len(), 1);
    let payload = &starts[0];
    assert_eq!(payload.player_id, LAB_PLAYER_ONE_ID);
    assert!(payload.spectator);
    assert!(payload.prediction_build_id.is_none());
    assert_eq!(payload.prediction_version, 0);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);
    assert!(payload.capabilities.room_time.available);
    assert!(payload.capabilities.room_time.set_speed);
    assert!(payload.capabilities.room_time.pause);
    assert!(payload.capabilities.room_time.step);
    assert!(payload.capabilities.room_time.seek_relative);
    assert!(payload.capabilities.room_time.seek_absolute);
    assert!(payload.capabilities.room_time.timeline);
    assert!(!payload.capabilities.visibility.vision_selection);
    assert!(!payload.capabilities.actions.branch_from_tick);
    assert!(payload.diagnostics.is_empty());
    assert!(payload.replay.is_none());
    assert_eq!(payload.players.len(), 2);
    assert_eq!(payload.players[0].team_id, 1);
    assert_eq!(payload.players[1].team_id, 2);
    let lab = payload.lab.as_ref().expect("lab metadata");
    assert_eq!(lab.room, "sandbox");
    assert_eq!(lab.operator_id, 99);
    assert_eq!(lab.role, LabStartRole::Operator);
    assert_eq!(lab.vision, LabVisionMode::FullWorld);
    assert!(!lab.dirty);
    assert_eq!(lab.operation_count, 0);
}

#[test]
fn map_editor_handoff_materializes_before_the_first_lab_start_payload() {
    let size = 126;
    let mut config = lab_config();
    config.map_draft = Some(crate::protocol::LabMapDraft {
        name: "Room map".to_string(),
        size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        starts: vec![
            crate::protocol::LabMapTile { x: 16, y: 16 },
            crate::protocol::LabMapTile {
                x: size - 17,
                y: size - 17,
            },
            crate::protocol::LabMapTile {
                x: size - 17,
                y: 16,
            },
            crate::protocol::LabMapTile {
                x: 16,
                y: size - 17,
            },
        ],
        base_sites: vec![crate::protocol::LabMapTile {
            x: size / 2,
            y: size / 2,
        }],
    });
    let mut task = RoomTask::new(
        "__lab__:map-editor:map=Default".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    let starts = start_payloads(&mut writer);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].tick, 0);
    assert_eq!(starts[0].map.width, size);
    assert_eq!(starts[0].players.len(), 4);
    assert_eq!(starts[0].players[0].start_tile_x, 16);
    assert_eq!(starts[0].players[3].name, "Lab Delta");

    task.on_lab_request(99, 701, LabClientOp::ExportMap);
    let exported = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .find_map(|message| match message {
            ServerMessage::LabResult(result) if result.request_id == 701 => Some(result),
            _ => None,
        })
        .expect("map-only Lab export result");
    assert!(exported.ok);
    let map = &exported.outcome.as_ref().expect("export outcome")["map"];
    assert_eq!(map["name"], "Room map");
    assert_eq!(map["starts"].as_array().map(Vec::len), Some(4));
    assert!(map.get("entities").is_none());
}

#[test]
fn lab_start_payload_can_use_bundled_lategame_scenario() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=lategame".to_string(),
        RoomMode::Lab(lategame_lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    let starts = start_payloads(&mut writer);
    assert_eq!(starts.len(), 1);
    assert_eq!(starts[0].players.len(), 2);
    assert_eq!(starts[0].players[0].name, "Lab Alpha");
    assert_eq!(starts[0].players[1].name, "Lab Bravo");
    let lab = starts[0].lab.as_ref().expect("lab metadata");
    assert_eq!(lab.vision, LabVisionMode::FullWorld);
    assert!(!lab.dirty);
    assert_eq!(lab.operation_count, 0);
    let Phase::InGame(game) = &task.phase else {
        panic!("lategame lab should start immediately");
    };
    assert_eq!(game.seed(), 3_566_641_871);
    assert_eq!(game.start_payload().players.len(), 2);
    assert_eq!(game.perf_entity_counts().entities, 227);
    let snapshot = game.snapshot_full_for(1);
    let resources = |player_id| {
        snapshot
            .player_resources
            .iter()
            .find(|resources| resources.id == player_id)
            .expect("player resources should be projected")
    };
    assert_eq!(resources(1).steel, 99_999);
    assert_eq!(resources(1).oil, 99_999);
    assert_eq!(resources(2).steel, 99_999);
    assert_eq!(resources(2).oil, 99_999);
    let mut all_research = CURRENT_CATALOG.researchable_upgrades(EntityKind::TrainingCentre);
    all_research.extend(CURRENT_CATALOG.researchable_upgrades(EntityKind::ResearchComplex));
    all_research.sort_unstable();
    for player_id in [1, 2] {
        let mut completed_research = game.snapshot_full_for(player_id).upgrades;
        completed_research.sort_unstable();
        assert_eq!(
            completed_research, all_research,
            "player {player_id} should have all current Kriegsia research"
        );
    }
    assert_eq!(
        task.lab_timeline
            .as_ref()
            .expect("lab timeline")
            .keyframe_ticks(),
        vec![0]
    );
}

#[test]
fn lab_start_payload_uses_bundled_render_preview_god_mode() {
    let mut config = lab_config();
    config.scenario = Some("render-preview".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=render-preview".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    let starts = start_payloads(&mut writer);
    assert_eq!(starts.len(), 1);
    let lab = starts[0].lab.as_ref().expect("lab metadata");
    assert_eq!(lab.god_mode_players, vec![1, 2]);
    let expected_initial_camera = Some(crate::protocol::InitialCamera {
        center_x: 2016,
        center_y: 2016,
    });
    assert_eq!(lab.initial_camera, expected_initial_camera);
    let artifact = task
        .export_lab_replay_artifact(99, Some("render preview replay"))
        .expect("lab replay export");
    assert_eq!(
        artifact.initial_setup.metadata.lab.initial_camera,
        expected_initial_camera
    );
    let Phase::InGame(game) = &task.phase else {
        panic!("render preview lab should start immediately");
    };
    assert_eq!(game.lab_god_mode_players(), vec![1, 2]);
}

#[test]
fn lab_authoring_validation_returns_repo_preview_without_mutating_lab() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    drain_reliable_messages(&mut writer);

    task.on_lab_request(
        99,
        7,
        LabClientOp::ValidateScenario {
            metadata: crate::protocol::LabScenarioAuthoringMetadata {
                slug: "room-dry-run".to_string(),
                name: "Room Dry Run".to_string(),
                title: "Room Dry Run".to_string(),
                description: "Dry-run validation from the authoritative lab game.".to_string(),
                tags: vec!["test".to_string()],
                review_notes: Some("No branch should be created in phase two.".to_string()),
            },
        },
    );

    let result = lab_results(&mut writer).pop().expect("validation result");
    assert!(result.ok, "validation should succeed: {result:?}");
    assert_eq!(result.op, "validateScenario");
    let preview = result
        .outcome
        .as_ref()
        .and_then(|outcome| outcome.get("preview"))
        .expect("validation preview");
    assert_eq!(
        preview
            .get("scenarioPath")
            .and_then(serde_json::Value::as_str),
        Some("server/assets/lab-scenarios/room-dry-run.json")
    );
    assert!(preview
        .get("scenarioJson")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|json| json.contains("\"name\": \"Room Dry Run\"")));
    let session = task.lab_session.as_ref().expect("lab session");
    assert!(!session.dirty);
    assert!(session.operation_log.is_empty());
}

#[test]
fn lab_scenario_submission_without_credentials_returns_structured_error() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    drain_reliable_messages(&mut writer);

    task.on_lab_request(
        99,
        8,
        LabClientOp::SubmitScenario {
            metadata: authoring_metadata("missing-credentials-submit"),
        },
    );

    let result = lab_results(&mut writer).pop().expect("submission result");
    assert!(!result.ok);
    assert_eq!(result.op, "submitScenario");
    assert!(result
        .error
        .as_deref()
        .is_some_and(|error| error.contains("disabled")));
    assert_eq!(
        result
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("credentialsMissing")
    );
}

#[tokio::test]
async fn lab_scenario_submission_dispatches_authoritative_export_and_rate_limits_room() {
    let captured = Arc::new(StdMutex::new(Vec::new()));
    let service = LabScenarioSubmissionService::enabled_for_test(RecordingSubmissionBackend {
        captured: captured.clone(),
    });
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    )
    .with_lab_scenario_submission(service);
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    drain_reliable_messages(&mut writer);

    task.on_lab_request(
        99,
        9,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 777,
            oil: 66,
        },
    );
    assert!(lab_results(&mut writer)[0].ok);

    task.on_lab_request(
        99,
        10,
        LabClientOp::SubmitScenario {
            metadata: authoring_metadata("successful-submit"),
        },
    );

    let result = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if let ServerMessage::LabResult(result) =
                writer.reliable_rx.recv().await.expect("lab result message")
            {
                break result;
            }
        }
    })
    .await
    .expect("submission result should arrive");
    assert!(result.ok, "submission should succeed: {result:?}");
    assert_eq!(
        result
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("prUrl"))
            .and_then(serde_json::Value::as_str),
        Some("https://github.com/example/rts/pull/17")
    );

    let captured = captured.lock().unwrap();
    assert_eq!(captured.len(), 1);
    let request = &captured[0];
    assert_eq!(
        request.branch_name,
        "zvorygin/lab-scenario-successful-submit"
    );
    let scenario_file = request
        .files
        .iter()
        .find(|file| file.path == "server/assets/lab-scenarios/successful-submit.json")
        .expect("scenario file should be part of PR request");
    let scenario: crate::protocol::LabCheckpointScenarioV1 =
        serde_json::from_str(&scenario_file.contents).expect("submitted checkpoint scenario JSON");
    assert_eq!(scenario.kind, "labCheckpointScenario");
    let checkpoint: serde_json::Value =
        serde_json::from_str(&scenario.checkpoint_payload).expect("embedded checkpoint payload");
    let player = checkpoint["players"]
        .as_array()
        .expect("checkpoint players")
        .iter()
        .find(|player| player["id"].as_u64() == Some(u64::from(LAB_PLAYER_ONE_ID)))
        .expect("lab player one in checkpoint payload");
    assert_eq!(player["steel"].as_u64(), Some(777));
    assert_eq!(player["oil"].as_u64(), Some(66));
    drop(captured);

    task.on_lab_request(
        99,
        11,
        LabClientOp::SubmitScenario {
            metadata: authoring_metadata("second-submit"),
        },
    );
    let rate_limited = lab_results(&mut writer)
        .pop()
        .expect("rate-limit result should be immediate");
    assert!(!rate_limited.ok);
    assert_eq!(
        rate_limited
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("rateLimit")
    );
}

#[test]
fn lab_room_first_join_during_drain_is_rejected_without_starting_session() {
    let drain = DrainHandle::default();
    drain.begin_draining(Duration::from_secs(295));
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(false));
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::Error { msg } if msg == DRAINING_NEW_MATCHES_DISABLED_MSG
        )
    }));
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.players.is_empty());
    assert!(task.order.is_empty());
    assert!(task.lab_session.is_none());
    assert!(!task.match_tracked_for_drain);
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn failed_lab_room_start_does_not_increment_drain_accounting() {
    let drain = DrainHandle::default();
    let mut config = lab_config();
    config.map_name = "MissingLabMap".to_string();
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=MissingLabMap".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.lab_session.is_some());
    assert!(!task.match_tracked_for_drain);
    assert_eq!(drain.active_matches(), 0);
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::Error { msg } if msg.contains("Cannot load lab map")
        )
    }));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
}

#[test]
fn lab_start_payload_additional_joiner_uses_policy_metadata() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, _operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );

    let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
    let (viewer_ack, mut viewer_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Viewer".to_string(),
        true,
        false,
        viewer_tx,
        viewer_ack,
    );

    assert_eq!(viewer_ack_rx.try_recv(), Ok(true));
    let starts = start_payloads(&mut viewer_writer);
    assert_eq!(starts.len(), 1);
    let payload = &starts[0];
    assert_eq!(payload.player_id, LAB_PLAYER_ONE_ID);
    assert!(payload.spectator);
    assert!(payload.prediction_build_id.is_none());
    assert_eq!(payload.prediction_version, 0);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);
    assert!(payload.diagnostics.is_empty());
    assert!(payload.replay.is_none());
    let lab = payload.lab.as_ref().expect("lab metadata");
    assert_eq!(lab.operator_id, 99);
    assert_eq!(lab.role, LabStartRole::Operator);
    assert_eq!(lab.vision, LabVisionMode::FullWorld);
}

#[test]
fn running_lab_room_collaborator_can_join_during_drain() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        drain.clone(),
    );
    let (operator_tx, _operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(drain.active_matches(), 1);
    let notice = drain.begin_draining(Duration::from_secs(295));

    let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
    let (viewer_ack, mut viewer_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Collaborator".to_string(),
        true,
        false,
        viewer_tx,
        viewer_ack,
    );

    assert_eq!(viewer_ack_rx.try_recv(), Ok(true));
    assert_eq!(drain.active_matches(), 1);
    assert!(matches!(task.phase, Phase::InGame(_)));
    let messages: Vec<_> =
        std::iter::from_fn(|| viewer_writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::ShutdownWarning {
                deadline_unix_ms,
                seconds_remaining,
            } if *deadline_unix_ms == notice.deadline_unix_ms && *seconds_remaining == 295
        )
    }));
    let start = messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .expect("collaborator should receive lab start");
    let lab = start.lab.as_ref().expect("lab metadata");
    assert_eq!(lab.operator_id, 99);
    assert_eq!(lab.role, LabStartRole::Operator);
    assert_eq!(
        task.lab_session.as_ref().unwrap().role_for(100),
        LabStartRole::Operator
    );
}

#[test]
fn lab_room_snapshot_uses_full_world_projection() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    while writer.reliable_rx.try_recv().is_ok() {}

    task.on_tick(TokioInstant::now());

    let snapshot = writer.snapshots.take().expect("lab snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should remain live");
    };
    let mut expected = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
    compact_snapshot_for_wire(&mut expected);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    assert!(snapshot.visible_tiles.is_empty());
    assert_eq!(snapshot.entities.len(), expected.entities.len());
    assert_eq!(snapshot.player_resources, expected.player_resources);
    assert_eq!(snapshot.net_status.prediction_version, 0);
}

fn drain_reliable_messages(writer: &mut ConnectionWriter) {
    while writer.reliable_rx.try_recv().is_ok() {}
}

fn spawn_lab_entity(
    task: &mut RoomTask,
    writer: &mut ConnectionWriter,
    request_id: u32,
    owner: u32,
    kind: &str,
    position: (f32, f32),
) -> u32 {
    task.on_lab_request(
        99,
        request_id,
        LabClientOp::SpawnEntity {
            owner,
            kind: kind.to_string(),
            x: position.0,
            y: position.1,
            completed: true,
        },
    );
    let result = lab_results(writer).pop().expect("spawn result");
    assert!(result.ok, "spawn should succeed: {result:?}");
    result
        .outcome
        .as_ref()
        .and_then(|outcome| outcome.get("entityId"))
        .and_then(serde_json::Value::as_u64)
        .expect("spawned entity id") as u32
}

fn launch_event(snapshot: &Snapshot, mortar_id: u32) -> Option<Event> {
    snapshot
        .events
        .iter()
        .find(|event| matches!(event, Event::MortarLaunch { from, .. } if *from == mortar_id))
        .cloned()
}

fn import_lab_checkpoint_with_deployed_entity(
    task: &mut RoomTask,
    entity_id: u32,
    target: (f32, f32),
) -> u32 {
    let Phase::InGame(game) = &mut task.phase else {
        panic!("lab should be running");
    };
    let mut scenario = game
        .export_lab_checkpoint_scenario("deployed test setup".to_string(), "room-task-test")
        .expect("checkpoint setup should export");
    let mut payload: serde_json::Value =
        serde_json::from_str(&scenario.checkpoint_payload).expect("checkpoint payload JSON");
    let entities = payload["entities"]["entities"]
        .as_array_mut()
        .expect("checkpoint entities array");
    let entity = entities
        .iter_mut()
        .find(|entity| entity["id"].as_u64() == Some(entity_id as u64))
        .expect("checkpoint should include spawned entity");
    let x = entity["pos_x"].as_f64().expect("entity x") as f32;
    let y = entity["pos_y"].as_f64().expect("entity y") as f32;
    let facing = (target.1 - y).atan2(target.0 - x);
    let combat = entity["combat"].as_object_mut().expect("entity combat");
    combat.insert("setup".to_string(), serde_json::json!("Deployed"));
    combat.insert("weapon_facing".to_string(), serde_json::json!(facing));
    combat.insert(
        "desired_weapon_facing".to_string(),
        serde_json::json!(facing),
    );
    combat.insert("emplacement_facing".to_string(), serde_json::json!(facing));
    scenario.checkpoint_payload =
        serde_json::to_string(&payload).expect("checkpoint payload should serialize");

    let outcome = game
        .apply_lab_op(LabOp::RestoreCheckpointScenario(Box::new(scenario)))
        .expect("checkpoint restore should succeed");
    let rts_sim::game::lab::LabOpOutcome::ScenarioRestored(restore) = outcome else {
        panic!("checkpoint restore should return entity id map");
    };
    restore
        .entity_id_map
        .iter()
        .find_map(|entry| (entry.old_id == entity_id).then_some(entry.new_id))
        .expect("restore should return remapped mortar id")
}

fn has_mortar_impact(snapshot: &Snapshot) -> bool {
    snapshot
        .events
        .iter()
        .any(|event| matches!(event, Event::MortarImpact { .. }))
}

fn issue_lab_mortar_fire(
    task: &mut RoomTask,
    writer: &mut ConnectionWriter,
    request_id: u32,
    player_id: u32,
    mortar_id: u32,
    target: (f32, f32),
) {
    task.on_lab_request(
        99,
        request_id,
        LabClientOp::IssueCommandAs {
            player_id,
            cmd: Command::UseAbility {
                ability: "mortarFire".to_string(),
                units: vec![mortar_id],
                x: Some(target.0),
                y: Some(target.1),
                queued: false,
            },
            ignore_command_limits: false,
        },
    );
    let result = lab_results(writer).pop().expect("mortar fire result");
    assert!(result.ok, "mortar fire should succeed: {result:?}");
}

fn prepare_player_two_lab_mortar(
    task: &mut RoomTask,
    writer: &mut ConnectionWriter,
    request_id_base: u32,
) -> (u32, (f32, f32)) {
    let mortar_position = lab_tile_center(task, 30, 30);
    let target_position = lab_tile_center(task, 38, 30);
    let mortar_id = spawn_lab_entity(
        task,
        writer,
        request_id_base,
        LAB_PLAYER_TWO_ID,
        crate::protocol::kinds::MORTAR_TEAM,
        mortar_position,
    );
    spawn_lab_entity(
        task,
        writer,
        request_id_base + 1,
        LAB_PLAYER_ONE_ID,
        crate::protocol::kinds::RIFLEMAN,
        target_position,
    );
    let mortar_id = import_lab_checkpoint_with_deployed_entity(task, mortar_id, target_position);
    (mortar_id, target_position)
}

#[test]
fn lab_full_world_operator_receives_player_two_mortar_launch_event() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    drain_reliable_messages(&mut writer);

    let (mortar_id, target_position) = prepare_player_two_lab_mortar(&mut task, &mut writer, 80);
    let _ = writer.snapshots.take();
    issue_lab_mortar_fire(
        &mut task,
        &mut writer,
        83,
        LAB_PLAYER_TWO_ID,
        mortar_id,
        target_position,
    );
    drain_reliable_messages(&mut writer);

    for _ in 0..40 {
        task.on_tick(TokioInstant::now());
        let snapshot = writer.snapshots.take().expect("lab snapshot");
        assert!(
            !has_mortar_impact(&snapshot),
            "impact should not arrive before launch"
        );
        if launch_event(&snapshot, mortar_id).is_some() {
            return;
        }
    }
    panic!("default full-world lab operator should receive P2 mortarLaunch");
}

#[test]
fn multiple_lab_full_world_viewers_receive_same_mortar_launch_event() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, mut operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
    let (viewer_ack, _viewer_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Viewer".to_string(),
        true,
        false,
        viewer_tx,
        viewer_ack,
    );
    drain_reliable_messages(&mut operator_writer);
    drain_reliable_messages(&mut viewer_writer);

    let (mortar_id, target_position) =
        prepare_player_two_lab_mortar(&mut task, &mut operator_writer, 90);
    drain_reliable_messages(&mut viewer_writer);
    let _ = operator_writer.snapshots.take();
    let _ = viewer_writer.snapshots.take();
    issue_lab_mortar_fire(
        &mut task,
        &mut operator_writer,
        93,
        LAB_PLAYER_TWO_ID,
        mortar_id,
        target_position,
    );
    drain_reliable_messages(&mut operator_writer);
    drain_reliable_messages(&mut viewer_writer);

    for _ in 0..40 {
        task.on_tick(TokioInstant::now());
        let operator_snapshot = operator_writer.snapshots.take().expect("operator snapshot");
        let viewer_snapshot = viewer_writer.snapshots.take().expect("viewer snapshot");
        assert!(
            !has_mortar_impact(&operator_snapshot) && !has_mortar_impact(&viewer_snapshot),
            "impact should not arrive before launch"
        );
        let operator_launch = launch_event(&operator_snapshot, mortar_id);
        let viewer_launch = launch_event(&viewer_snapshot, mortar_id);
        if operator_launch.is_some() || viewer_launch.is_some() {
            assert_eq!(
                operator_launch, viewer_launch,
                "full-world lab viewers should receive identical launch events"
            );
            return;
        }
    }
    panic!("both full-world lab viewers should receive P2 mortarLaunch");
}

#[test]
fn lab_team_vision_receives_only_selected_player_mortar_events() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, mut operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
    let (viewer_ack, _viewer_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Viewer".to_string(),
        true,
        false,
        viewer_tx,
        viewer_ack,
    );
    drain_reliable_messages(&mut operator_writer);
    drain_reliable_messages(&mut viewer_writer);

    task.on_lab_request(
        99,
        100,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 2 },
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);
    task.on_lab_request(
        100,
        101,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 1 },
        },
    );
    assert!(lab_results(&mut viewer_writer)[0].ok);
    drain_reliable_messages(&mut operator_writer);
    drain_reliable_messages(&mut viewer_writer);

    task.on_tick(TokioInstant::now());
    let team_two_resource_scope = operator_writer
        .snapshots
        .take()
        .expect("team 2 lab snapshot")
        .player_resources
        .into_iter()
        .map(|resources| resources.id)
        .collect::<Vec<_>>();
    let team_one_resource_scope = viewer_writer
        .snapshots
        .take()
        .expect("team 1 lab snapshot")
        .player_resources
        .into_iter()
        .map(|resources| resources.id)
        .collect::<Vec<_>>();
    assert_eq!(
        team_two_resource_scope,
        vec![LAB_PLAYER_TWO_ID],
        "team 2 lab vision should only expose team 2 resource rows"
    );
    assert_eq!(
        team_one_resource_scope,
        vec![LAB_PLAYER_ONE_ID],
        "team 1 lab vision should only expose team 1 resource rows"
    );

    let (mortar_id, target_position) =
        prepare_player_two_lab_mortar(&mut task, &mut operator_writer, 102);
    drain_reliable_messages(&mut viewer_writer);
    let _ = operator_writer.snapshots.take();
    let _ = viewer_writer.snapshots.take();
    issue_lab_mortar_fire(
        &mut task,
        &mut operator_writer,
        105,
        LAB_PLAYER_TWO_ID,
        mortar_id,
        target_position,
    );
    drain_reliable_messages(&mut operator_writer);
    drain_reliable_messages(&mut viewer_writer);

    for _ in 0..40 {
        task.on_tick(TokioInstant::now());
        let team_two_snapshot = operator_writer
            .snapshots
            .take()
            .expect("team 2 lab snapshot");
        let team_one_snapshot = viewer_writer.snapshots.take().expect("team 1 lab snapshot");
        assert!(
            launch_event(&team_one_snapshot, mortar_id).is_none(),
            "team 1 lab vision should not receive owner-only P2 launch events"
        );
        assert!(
            !has_mortar_impact(&team_two_snapshot) && !has_mortar_impact(&team_one_snapshot),
            "impact should not arrive before launch"
        );
        if launch_event(&team_two_snapshot, mortar_id).is_some() {
            return;
        }
    }
    panic!("team 2 lab vision should receive P2 mortarLaunch");
}

#[test]
fn lab_start_payload_advertises_room_time_controls() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    let starts: Vec<_> = messages
        .iter()
        .filter_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .collect();
    assert_eq!(starts.len(), 1);
    let payload = starts[0];
    assert!(payload.lab.is_some());
    assert!(payload.capabilities.room_time.available);
    assert!(payload.capabilities.room_time.set_speed);
    assert!(payload.capabilities.room_time.pause);
    assert!(payload.capabilities.room_time.step);
    assert!(payload.capabilities.room_time.seek_relative);
    assert!(payload.capabilities.room_time.seek_absolute);
    assert!(payload.capabilities.room_time.timeline);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);

    let states: Vec<_> = messages
        .iter()
        .filter_map(|msg| match msg {
            ServerMessage::RoomTimeState(state) => Some(state),
            _ => None,
        })
        .collect();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].current_tick, 0);
    assert_eq!(states[0].duration_ticks, 0);
    assert_eq!(states[0].keyframe_ticks.as_slice(), &[0]);
    assert_eq!(states[0].speed, 1.0);
    assert!(!states[0].paused);
    assert!(!states[0].ended);
    assert_eq!(states[0].controller_id, None);
}

#[test]
fn paused_lab_room_steps_one_live_tick_and_shares_room_time_state() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, mut operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    let (collab_tx, mut collab_writer) = ConnectionSink::new();
    let (collab_ack, _collab_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Collaborator".to_string(),
        true,
        false,
        collab_tx,
        collab_ack,
    );
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    while collab_writer.reliable_rx.try_recv().is_ok() {}

    assert_eq!(in_game_tick(&task), 0);
    task.on_set_room_time_speed(99, 0.0);

    let operator_pause = room_time_states(&mut operator_writer);
    let collab_pause = room_time_states(&mut collab_writer);
    assert_eq!(operator_pause.len(), 1);
    assert_eq!(collab_pause.len(), 1);
    for state in [&operator_pause[0], &collab_pause[0]] {
        assert_eq!(state.current_tick, 0);
        assert_eq!(state.speed, 0.0);
        assert!(state.paused);
        assert_eq!(state.controller_id, Some(99));
    }

    task.on_tick(TokioInstant::now());
    assert_eq!(
        in_game_tick(&task),
        0,
        "scheduled lab ticks should not advance while paused"
    );

    task.on_step_room_time(100);
    assert_eq!(in_game_tick(&task), 1);
    assert!(operator_writer.snapshots.take().is_some());
    assert!(collab_writer.snapshots.take().is_some());
    let operator_step = room_time_states(&mut operator_writer);
    let collab_step = room_time_states(&mut collab_writer);
    assert_eq!(operator_step.len(), 1);
    assert_eq!(collab_step.len(), 1);
    for state in [&operator_step[0], &collab_step[0]] {
        assert_eq!(state.current_tick, 1);
        assert_eq!(state.speed, 0.0);
        assert!(state.paused);
        assert_eq!(state.controller_id, Some(100));
    }

    task.on_set_room_time_speed(100, 2.0);
    let operator_resume = room_time_states(&mut operator_writer);
    assert_eq!(operator_resume.len(), 1);
    assert_eq!(operator_resume[0].current_tick, 1);
    assert_eq!(operator_resume[0].speed, 2.0);
    assert!(!operator_resume[0].paused);
    assert_eq!(operator_resume[0].controller_id, Some(100));

    task.on_tick(TokioInstant::now());
    assert_eq!(
        in_game_tick(&task),
        2,
        "scheduled lab ticks should resume after selecting a non-zero speed"
    );
}

#[test]
fn lab_operator_mutation_returns_result_broadcasts_state_and_logs() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    while writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        7,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 1234,
            oil: 55,
        },
    );

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    let results: Vec<_> = messages
        .iter()
        .filter_map(|msg| match msg {
            ServerMessage::LabResult(result) => Some(result),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 1);
    assert!(results[0].ok);
    assert_eq!(results[0].request_id, 7);
    assert_eq!(results[0].op, "setPlayerResources");
    let states: Vec<_> = messages
        .iter()
        .filter_map(|msg| match msg {
            ServerMessage::LabState(state) => Some(state),
            _ => None,
        })
        .collect();
    assert_eq!(states.len(), 1);
    assert!(states[0].dirty);
    assert_eq!(states[0].operation_count, 1);
    let session = task.lab_session.as_ref().unwrap();
    assert_eq!(session.operation_log.len(), 1);
    assert_eq!(session.operation_log[0].request_id, 7);
    assert_eq!(session.operation_log[0].operator_id, 99);
    assert_eq!(session.operation_log[0].tick, 0);
    assert_eq!(session.operation_log[0].op, "setPlayerResources");
    assert!(session.operation_log[0].result.contains("playerId"));
}

#[test]
fn lab_collaborators_can_mutate_issue_commands_and_log_requester() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, mut operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    let (collab_tx, mut collab_writer) = ConnectionSink::new();
    let (collab_ack, _collab_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Collaborator".to_string(),
        true,
        false,
        collab_tx,
        collab_ack,
    );
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    while collab_writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        30,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 456,
            oil: 78,
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);

    let Phase::InGame(game) = &task.phase else {
        panic!("lab should be running");
    };
    let worker = game
        .snapshot_full_for(LAB_PLAYER_ONE_ID)
        .entities
        .iter()
        .find(|entity| {
            entity.owner == LAB_PLAYER_ONE_ID && entity.kind == crate::protocol::kinds::WORKER
        })
        .unwrap()
        .id;

    task.on_lab_request(
        100,
        31,
        LabClientOp::IssueCommandAs {
            player_id: LAB_PLAYER_ONE_ID,
            cmd: Command::Stop {
                units: vec![worker],
            },
            ignore_command_limits: false,
        },
    );
    let command_result = lab_results(&mut collab_writer)
        .pop()
        .expect("issueCommandAs result");
    assert!(command_result.ok);
    let command_outcome = command_result.outcome.expect("issueCommandAs outcome");
    assert_eq!(command_outcome["accepted"], true);
    assert_eq!(command_outcome["admission"], "enqueued");
    assert_eq!(command_outcome["playerId"], LAB_PLAYER_ONE_ID);
    assert!(command_outcome["queuedAtTick"].as_u64().is_some());

    let session = task.lab_session.as_ref().unwrap();
    assert_eq!(session.role_for(99), LabStartRole::Operator);
    assert_eq!(session.role_for(100), LabStartRole::Operator);
    assert_eq!(session.operation_log.len(), 2);
    assert_eq!(session.operation_log[0].request_id, 30);
    assert_eq!(session.operation_log[0].operator_id, 99);
    assert_eq!(session.operation_log[0].op, "setPlayerResources");
    assert_eq!(session.operation_log[1].request_id, 31);
    assert_eq!(session.operation_log[1].operator_id, 100);
    assert_eq!(session.operation_log[1].op, "issueCommandAs");
}

#[test]
fn lab_read_only_role_rejects_privileged_ops() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (operator_tx, mut operator_writer) = ConnectionSink::new();
    let (operator_ack, _operator_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        99,
        "Operator".to_string(),
        true,
        false,
        operator_tx,
        operator_ack,
    );
    let (viewer_tx, mut viewer_writer) = ConnectionSink::new();
    let (viewer_ack, _viewer_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        100,
        "Read Only".to_string(),
        true,
        false,
        viewer_tx,
        viewer_ack,
    );
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    while viewer_writer.reliable_rx.try_recv().is_ok() {}
    task.lab_session
        .as_mut()
        .unwrap()
        .viewer_roles
        .insert(100, LabStartRole::ReadOnly);

    task.on_lab_request(
        100,
        32,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 456,
            oil: 78,
        },
    );

    let results = lab_results(&mut viewer_writer);
    assert_eq!(results.len(), 1);
    assert!(!results[0].ok);
    assert_eq!(results[0].request_id, 32);
    assert!(results[0]
        .error
        .as_deref()
        .unwrap()
        .contains("only lab operators"));
    let session = task.lab_session.as_ref().unwrap();
    assert_eq!(session.role_for(100), LabStartRole::ReadOnly);
    assert!(session.operation_log.is_empty());
}

#[test]
fn normal_room_rejects_lab_request() {
    let mut normal = RoomTask::new(
        "normal-lab-reject-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut normal_writer = add_test_room_player(&mut normal, 1, true);
    normal.on_lab_request(
        1,
        9,
        LabClientOp::SetVision {
            vision: LabVisionMode::FullWorld,
        },
    );
    let normal_results = lab_results(&mut normal_writer);
    assert_eq!(normal_results.len(), 1);
    assert!(!normal_results[0].ok);
    assert!(normal_results[0]
        .error
        .as_deref()
        .unwrap()
        .contains("lab rooms"));
}

#[test]
fn replay_viewer_rejects_lab_requests_and_gameplay_commands() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "replay-lab-reject-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_spectator(&mut task, 99);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_lab_request(
        99,
        33,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 999,
            oil: 999,
        },
    );
    task.on_command(
        99,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );

    let results = lab_results(&mut writer);
    assert_eq!(results.len(), 1);
    assert!(!results[0].ok);
    assert_eq!(results[0].request_id, 33);
    assert!(results[0].error.as_deref().unwrap().contains("lab rooms"));
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("replay viewer should remain active");
    };
    assert!(session.game().command_log().is_empty());
    assert_eq!(task.players.get(&99).unwrap().last_received_client_seq, 0);
    assert!(task.lab_session.is_none());
}

#[test]
fn empty_lab_room_resets_session_without_changing_lab_mode() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(drain.active_matches(), 1);
    assert!(task.match_tracked_for_drain);

    task.on_leave(99);

    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.players.is_empty());
    assert!(task.lab_session.is_none());
    assert_eq!(drain.active_matches(), 0);
    assert!(matches!(task.mode, RoomMode::Lab(_)));
}
