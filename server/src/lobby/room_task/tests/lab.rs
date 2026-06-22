use super::support::*;

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
    assert!(!payload.capabilities.visibility.replay_vision);
    assert!(!payload.capabilities.actions.replay_branch);
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
    let scenario = game.export_lab_scenario();
    assert_eq!(scenario.seed, 3_566_641_871);
    assert_eq!(scenario.players.len(), 2);
    assert_eq!(scenario.entities.len(), 227);
    assert_eq!(scenario.players[0].resources.steel, 99_999);
    assert_eq!(scenario.players[0].resources.oil, 99_999);
    assert_eq!(scenario.players[1].resources.steel, 99_999);
    assert_eq!(scenario.players[1].resources.oil, 99_999);
    let all_research = [
        "methamphetamines",
        "anti_tank_gun_unlock",
        "tank_unlock",
        "artillery_unlock",
        "command_car_unlock",
        "mortar_autocast",
    ];
    for player in &scenario.players {
        for upgrade in all_research {
            assert!(
                player
                    .research
                    .completed
                    .iter()
                    .any(|completed| completed == upgrade),
                "player {} should have {upgrade}",
                player.id
            );
        }
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
        },
    );
    assert!(lab_results(&mut collab_writer)[0].ok);

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
