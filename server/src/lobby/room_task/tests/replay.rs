use super::support::*;

#[test]
fn room_task_tick_control_preserves_current_intervals_by_mode() {
    let base = Duration::from_millis(config::TICK_MS);

    let normal = RoomTask::new(
        "tick-normal".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    assert_eq!(normal.current_tick_interval(), base);

    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let mut replay = ReplaySession::new(artifact).unwrap();
    replay.set_speed(99, 2.0);
    let mut replay_task = RoomTask::new(
        "tick-replay".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut replay_task, 99, true);
    replay_task.phase = Phase::ReplayViewer(Box::new(replay));
    assert_eq!(replay_task.current_tick_interval(), base.div_f32(2.0));

    replay_task.on_set_room_time_speed(99, 0.0);
    assert_eq!(replay_task.current_tick_interval(), base);

    let mut dev = RoomTask::new(
        "tick-dev".to_string(),
        RoomMode::DevScenario(DevScenarioConfig {
            id: DevScenarioId::VehicleCornerWall,
            unit: EntityKind::Tank,
            count: 1,
            blocker: None,
            case: None,
        }),
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut dev, 99, true);
    dev.on_set_room_time_speed(99, 2.0);
    assert_eq!(dev.current_tick_interval(), base.div_f32(2.0));
    dev.on_set_room_time_speed(99, 0.0);
    assert_eq!(dev.current_tick_interval(), base);

    let seed = replay_branch_test_seed(&players, 1);
    let mut branch = RoomTask::new(
        "tick-branch".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    branch.room_time_speed = 4.0;
    branch.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
    assert_eq!(branch.current_tick_interval(), base);
}

#[test]
fn replay_start_payload_capabilities_survive_initial_and_seek_resends() {
    let initial = replay_start_payload_after("replay-start-caps-initial", |task| {
        task.send_replay_start_to(99)
    });
    let relative = replay_start_payload_after("replay-start-caps-relative", |task| {
        task.on_seek_room_time(99, 1)
    });
    let absolute = replay_start_payload_after("replay-start-caps-absolute", |task| {
        task.on_seek_room_time_to(99, 1)
    });

    for payload in [&initial, &relative, &absolute] {
        assert_eq!(payload.player_id, 99);
        assert!(payload.spectator);
        assert!(payload.replay.is_some());
        assert!(payload.capabilities.room_time.available);
        assert!(payload.capabilities.room_time.set_speed);
        assert!(payload.capabilities.room_time.pause);
        assert!(payload.capabilities.room_time.seek_relative);
        assert!(payload.capabilities.room_time.seek_absolute);
        assert!(payload.capabilities.room_time.timeline);
        assert!(payload.capabilities.visibility.replay_vision);
        assert!(payload.capabilities.actions.replay_branch);
        assert!(!payload.capabilities.commands.gameplay);
        assert!(!payload.capabilities.match_controls.pause);
        assert!(payload.diagnostics.observer_analysis);
        assert_eq!(
            payload.diagnostics.movement_paths,
            MovementPathDiagnosticScope::None
        );
    }

    assert_eq!(relative.capabilities, initial.capabilities);
    assert_eq!(absolute.capabilities, initial.capabilities);
    assert_eq!(relative.diagnostics, initial.diagnostics);
    assert_eq!(absolute.diagnostics, initial.diagnostics);
}

#[test]
fn replay_room_rejects_rapid_seek_without_resetting_viewers() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    let mut task = RoomTask::new(
        "replay-seek-rate-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_seek_room_time(99, 1);
    let first_seek_messages: Vec<_> =
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(first_seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Start(payload)
            if payload.capabilities.room_time.seek_relative
                && payload.capabilities.room_time.seek_absolute
                && payload.capabilities.visibility.replay_vision
    )));
    assert!(first_seek_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::RoomTimeState(_))));

    task.on_seek_room_time(99, 1);
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(msg, ServerMessage::Error { msg } if msg.contains("wait before seeking again"))
    }));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
}

#[test]
fn replay_join_and_seek_emit_authoritative_analysis() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    let mut task = RoomTask::new(
        "replay-analysis-send-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.send_replay_start_to(99);
    task.send_room_time_state_to(99);
    task.send_observer_analysis_to(99);
    let join_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(join_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 3 && analysis.players.len() == 2
    )));

    task.on_seek_room_time_to(99, 1);
    let seek_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Start(payload)
            if payload.capabilities.room_time.seek_relative
                && payload.capabilities.room_time.seek_absolute
                && payload.capabilities.visibility.replay_vision
    )));
    assert!(seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 1 && analysis.players.len() == 2
    )));
}

#[test]
fn rapid_replay_vision_changes_remain_per_viewer() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "replay-vision-stress-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let writer_a = add_test_room_player(&mut task, 100, true);
    let writer_b = add_test_room_player(&mut task, 101, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    for _ in 0..8 {
        task.on_set_replay_vision(
            100,
            ReplayVisionRequest::Player {
                player_id: players[0].id,
            },
        );
        task.on_set_replay_vision(
            101,
            ReplayVisionRequest::Player {
                player_id: players[1].id,
            },
        );
    }
    task.on_tick_replay_viewer(TokioInstant::now());

    let snapshot_a = writer_a.snapshots.take().expect("viewer A snapshot");
    let snapshot_b = writer_b.snapshots.take().expect("viewer B snapshot");
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("replay phase should remain active");
    };
    let expected_a = session.game.snapshot_for_spectator(&[players[0].id]);
    let expected_b = session.game.snapshot_for_spectator(&[players[1].id]);

    assert_eq!(snapshot_a.visible_tiles, expected_a.visible_tiles);
    assert_eq!(snapshot_b.visible_tiles, expected_b.visible_tiles);
    assert_ne!(
        snapshot_a.visible_tiles, snapshot_b.visible_tiles,
        "test setup should exercise different fog perspectives"
    );
}

#[test]
fn persisted_replay_room_join_prompts_before_playback() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let mut task = RoomTask::new(
        "persisted-replay-test".to_string(),
        RoomMode::Replay { artifact },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), false, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(false));
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(!task.players.contains_key(&99));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::JoinReplayPrompt { room } if room == "persisted-replay-test"
    ));
}

#[test]
fn persisted_replay_room_confirmed_join_starts_replay_viewer() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let mut task = RoomTask::new(
        "persisted-replay-confirmed-test".to_string(),
        RoomMode::Replay { artifact },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), false, true, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    assert!(task.players.get(&99).is_some_and(|p| p.spectator));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Start(payload)
            if payload.spectator
                && payload.replay.is_some()
                && payload.diagnostics.observer_analysis
    ));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(_)
    ));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::ObserverAnalysis(analysis)
            if analysis.tick == 0 && analysis.players.len() == players.len()
    ));

    task.on_tick_replay_viewer(TokioInstant::now());
    let snapshot = writer.snapshots.take().expect("replay viewer snapshot");
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("confirmed replay join should keep replay viewer active");
    };
    let visible_players = players.iter().map(|player| player.id).collect::<Vec<_>>();
    let expected = session.game.snapshot_for_spectator(&visible_players);
    assert_eq!(snapshot.tick, expected.tick);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    let tick_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(tick_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::ObserverAnalysis(analysis)
            if analysis.tick == expected.tick && analysis.players.len() == players.len()
    )));
}

#[test]
fn saved_artifact_replay_join_uses_replay_viewer_runtime() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let artifact_name = format!("room_task_saved_selfplay_{}", std::process::id());
    let artifact_dir = write_selfplay_replay_test_artifact(&artifact_name, &artifact);
    let mut task = RoomTask::new(
        "saved-artifact-replay-test".to_string(),
        RoomMode::ReplayArtifact {
            artifact: artifact_name,
        },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), false, true, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("saved artifact replay should start the shared replay viewer runtime");
    };
    assert_eq!(session.artifact.command_log, artifact.command_log);
    assert_eq!(session.vision_player_ids_for(99), vec![1, 2]);
    assert!(task.players.get(&99).is_some_and(|p| p.spectator));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Start(payload)
            if payload.spectator
                && payload.replay.is_some()
                && payload.diagnostics.observer_analysis
    ));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(_)
    ));

    let _ = std::fs::remove_dir_all(artifact_dir);
}

#[test]
fn post_match_replay_join_prompts_before_attaching_viewer() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "post-match-replay-prompt-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.phase = Phase::ReplayViewer(Box::new(replay));
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), false, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(false));
    assert!(!task.players.contains_key(&99));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::JoinReplayPrompt { room } if room == "post-match-replay-prompt-test"
    ));
}

#[test]
fn replay_viewer_return_detaches_only_requesting_viewer() {
    let players = replay_test_players(2);
    let (game, _artifact) = replay_test_artifact(&players, 1);
    let mut task = RoomTask::new(
        "post-match-lobby-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_test_room_player(&mut task, players[0].id, true);
    let writer_b = add_test_room_player(&mut task, players[1].id, true);
    task.match_player_count = 2;
    task.match_human_count = 2;

    task.end_match(Some(players[0].id), game.scores(), Some(&game));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));

    task.on_return_to_lobby(players[0].id);

    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    assert!(!task.players.contains_key(&players[0].id));
    assert!(task.players.contains_key(&players[1].id));
    assert_eq!(task.host_id, Some(players[1].id));

    task.on_tick_replay_viewer(TokioInstant::now());
    assert!(
        writer_b.snapshots.take().is_some(),
        "remaining viewers should keep receiving replay snapshots"
    );
}

#[test]
fn replay_viewer_return_resets_room_when_last_viewer_leaves() {
    let players = replay_test_players(2);
    let (game, _artifact) = replay_test_artifact(&players, 1);
    let mut task = RoomTask::new(
        "post-match-empty-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_test_room_player(&mut task, players[0].id, true);
    let _writer_b = add_test_room_player(&mut task, players[1].id, true);
    task.match_player_count = 2;
    task.match_human_count = 2;

    task.end_match(Some(players[0].id), game.scores(), Some(&game));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));

    task.on_return_to_lobby(players[0].id);
    task.on_return_to_lobby(players[1].id);

    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.players.is_empty());
    assert_eq!(task.host_id, None);
    assert_eq!(task.match_player_count, 0);
    assert_eq!(task.match_human_count, 0);
}
