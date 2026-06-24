use super::support::*;

#[test]
fn paused_replay_viewer_does_not_advance_on_scheduled_tick() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "replay-pause-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_set_room_time_speed(99, 0.0);
    assert_eq!(
        task.current_tick_interval(),
        Duration::from_millis(config::TICK_MS)
    );
    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 0);

    task.on_set_room_time_speed(99, 1.0);
    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 1);
}

#[test]
fn live_spectator_receives_observer_analysis_but_active_players_do_not() {
    let mut task = RoomTask::new(
        "live-spectator-analysis-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_test_room_player(&mut task, 1, true);
    let mut writer_b = add_test_room_player(&mut task, 2, true);
    let mut writer_spectator = add_test_room_spectator(&mut task, 99);

    task.start_match();
    while writer_a.reliable_rx.try_recv().is_ok() {}
    while writer_b.reliable_rx.try_recv().is_ok() {}
    while writer_spectator.reliable_rx.try_recv().is_ok() {}

    task.on_tick(TokioInstant::now());

    let spectator_messages: Vec<_> =
        std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
    assert!(spectator_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::ObserverAnalysis(analysis) if analysis.tick == 1 && analysis.players.len() == 2
    )));
    let mut active_messages: Vec<_> =
        std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
    active_messages.extend(std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()));
    assert!(!active_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::ObserverAnalysis(_))));
}

#[test]
fn normal_live_player_commands_use_connection_authority_and_ack_sequence() {
    let mut task = RoomTask::new(
        "live-command-authority-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 1, true);

    task.start_match();
    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_command(
        1,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );

    assert_eq!(task.players.get(&1).unwrap().last_received_client_seq, 1);
    assert_eq!(task.pending_client_command_acks.len(), 1);
    assert_eq!(task.pending_client_command_acks[0].connection_id, 1);
    assert_eq!(task.pending_client_command_acks[0].client_seq, 1);
    assert!(
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
            matches!(
                msg,
                ServerMessage::CommandReceipt {
                    client_seq: 1,
                    accepted: true,
                    ..
                }
            )
        })
    );

    task.on_tick(TokioInstant::now());

    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    assert_eq!(game.command_log().len(), 1);
    assert_eq!(game.command_log()[0].player_id, 1);
    assert!(task.pending_client_command_acks.is_empty());
    assert_eq!(
        task.players.get(&1).unwrap().last_sim_consumed_client_seq,
        1
    );
}

#[test]
fn live_pause_authorizes_active_players_and_tracks_limit() {
    let mut task = RoomTask::new(
        "live-pause-authority-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_test_room_player(&mut task, 1, true);
    let mut writer_b = add_test_room_player(&mut task, 2, true);
    let mut writer_spectator = add_test_room_spectator(&mut task, 99);

    task.start_match();
    while writer_a.reliable_rx.try_recv().is_ok() {}
    while writer_b.reliable_rx.try_recv().is_ok() {}
    while writer_spectator.reliable_rx.try_recv().is_ok() {}

    task.on_pause_game(99);
    assert!(!task.live_paused, "spectators must not pause live matches");

    task.on_pause_game(1);
    assert!(task.live_paused);
    assert_eq!(task.live_paused_by, Some(1));
    assert_eq!(task.live_pause_counts.get(&1), Some(&1));
    let active_state = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
        .find_map(|msg| match msg {
            ServerMessage::LivePauseState(state) => Some(state),
            _ => None,
        })
        .expect("active pause state");
    assert_eq!(active_state.pauses_remaining, Some(2));
    assert!(!active_state.can_pause);
    assert!(active_state.can_unpause);
    let spectator_state = std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok())
        .find_map(|msg| match msg {
            ServerMessage::LivePauseState(state) => Some(state),
            _ => None,
        })
        .expect("spectator pause state");
    assert_eq!(spectator_state.pauses_remaining, None);
    assert!(!spectator_state.can_unpause);

    task.on_pause_game(1);
    assert_eq!(
        task.live_pause_counts.get(&1),
        Some(&1),
        "repeated pause while paused must not spend another charge"
    );

    for expected_used in 1..=3 {
        if !task.live_paused {
            task.on_pause_game(1);
        }
        assert_eq!(task.live_pause_counts.get(&1), Some(&expected_used));
        task.on_unpause_game(2);
        assert!(!task.live_paused, "any active player can unpause");
    }

    task.on_pause_game(1);
    assert!(
        !task.live_paused,
        "fourth successful pause by one player is denied"
    );
    assert_eq!(task.live_pause_counts.get(&1), Some(&3));
    let denied_state = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
        .filter_map(|msg| match msg {
            ServerMessage::LivePauseState(state) => Some(state),
            _ => None,
        })
        .last()
        .expect("denied pause state");
    assert_eq!(denied_state.pauses_remaining, Some(0));
    assert!(!denied_state.can_pause);
    drop(writer_b);
}

#[test]
fn live_pause_skips_live_tick_work_until_unpaused() {
    let mut task = RoomTask::new(
        "live-pause-tick-skip-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 1, true);
    add_test_room_player(&mut task, 2, true);

    task.start_match();
    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_command(
        1,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );
    task.on_pause_game(1);
    task.on_tick(TokioInstant::now());
    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    assert_eq!(
        game.tick_count(),
        0,
        "paused scheduled tick must not advance sim"
    );
    assert_eq!(
        task.pending_client_command_acks.len(),
        1,
        "paused scheduled tick must not consume command acks"
    );

    task.on_unpause_game(2);
    task.on_tick(TokioInstant::now());
    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    assert_eq!(game.tick_count(), 1);
    assert!(task.pending_client_command_acks.is_empty());
}

#[test]
fn defeated_live_players_cannot_issue_more_commands() {
    let mut task = RoomTask::new(
        "defeated-command-authority-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut task, 1, true);
    add_test_room_player(&mut task, 2, true);

    task.start_match();
    task.outcome_sent.insert(1);
    task.on_command(
        1,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );

    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    assert!(game.command_log().is_empty());
    assert!(task.pending_client_command_acks.is_empty());
    assert_eq!(task.players.get(&1).unwrap().last_received_client_seq, 0);
}

#[test]
fn normal_live_start_payloads_stamp_active_players_and_spectators() {
    let mut task = RoomTask::new(
        "live-start-payload-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_player = add_test_room_player(&mut task, 1, true);
    let mut writer_spectator = add_test_room_spectator(&mut task, 99);

    task.start_match();

    let player_starts = start_payloads(&mut writer_player);
    assert_eq!(player_starts.len(), 1);
    let player_payload = &player_starts[0];
    assert_eq!(player_payload.player_id, 1);
    assert!(!player_payload.spectator);
    assert!(player_payload.prediction_build_id.is_some());
    assert_eq!(
        player_payload.prediction_version,
        PREDICTION_PROTOCOL_VERSION
    );
    assert!(player_payload.capabilities.commands.gameplay);
    assert!(player_payload.capabilities.match_controls.pause);
    assert!(!player_payload.capabilities.room_time.available);
    assert!(!player_payload.capabilities.visibility.replay_vision);
    assert!(!player_payload.capabilities.actions.replay_branch);
    assert!(player_payload.replay.is_none());
    assert!(player_payload.lab.is_none());
    assert!(player_payload.diagnostics.is_empty());

    let spectator_starts = start_payloads(&mut writer_spectator);
    assert_eq!(spectator_starts.len(), 1);
    let spectator_payload = &spectator_starts[0];
    assert_eq!(spectator_payload.player_id, 99);
    assert!(spectator_payload.spectator);
    assert!(spectator_payload.prediction_build_id.is_none());
    assert_eq!(spectator_payload.prediction_version, 0);
    assert!(!spectator_payload.capabilities.commands.gameplay);
    assert!(!spectator_payload.capabilities.match_controls.pause);
    assert!(!spectator_payload.capabilities.room_time.available);
    assert!(!spectator_payload.capabilities.visibility.replay_vision);
    assert!(!spectator_payload.capabilities.actions.replay_branch);
    assert!(spectator_payload.replay.is_none());
    assert!(spectator_payload.lab.is_none());
    assert_eq!(
        spectator_payload.diagnostics.movement_paths,
        MovementPathDiagnosticScope::None
    );
    assert!(spectator_payload.diagnostics.observer_analysis);
}

#[test]
fn ai_only_live_start_payload_advertises_speed_controls_without_seek() {
    let mut task = RoomTask::new(
        "ai-only-live-speed-start-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_spectator(&mut task, 1);
    task.host_id = Some(1);
    task.on_add_ai(1, Some(1), None);
    task.on_add_ai(1, Some(2), None);
    while writer.reliable_rx.try_recv().is_ok() {}

    task.start_match();

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    let payload = messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .expect("AI-only live start payload");
    assert!(payload.spectator);
    assert!(payload.prediction_build_id.is_none());
    assert_eq!(payload.prediction_version, 0);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);
    assert!(payload.capabilities.room_time.available);
    assert!(payload.capabilities.room_time.set_speed);
    assert!(payload.capabilities.room_time.pause);
    assert!(!payload.capabilities.room_time.step);
    assert!(!payload.capabilities.room_time.seek_relative);
    assert!(!payload.capabilities.room_time.seek_absolute);
    assert!(!payload.capabilities.room_time.timeline);
    assert_eq!(
        payload.diagnostics.movement_paths,
        MovementPathDiagnosticScope::None
    );
    assert!(payload.diagnostics.observer_analysis);

    let state = messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::RoomTimeState(state) => Some(state),
            _ => None,
        })
        .expect("AI-only live room-time state");
    assert_eq!(state.current_tick, 0);
    assert_eq!(state.duration_ticks, 0);
    assert!(state.keyframe_ticks.is_empty());
    assert_eq!(state.speed, 1.0);
    assert!(!state.paused);
    assert!(!state.ended);
}

#[test]
fn ai_only_live_room_time_speed_and_pause_control_tick_rate_without_seek() {
    let mut task = RoomTask::new(
        "ai-only-live-speed-control-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_spectator(&mut task, 1);
    task.host_id = Some(1);
    task.on_add_ai(1, Some(1), None);
    task.on_add_ai(1, Some(2), None);
    while writer.reliable_rx.try_recv().is_ok() {}
    task.start_match();
    while writer.reliable_rx.try_recv().is_ok() {}

    task.on_set_room_time_speed(1, 4.0);
    assert_eq!(
        task.current_tick_interval(),
        Duration::from_millis(config::TICK_MS).div_f32(4.0)
    );
    let speed_states = room_time_states(&mut writer);
    let speed_state = speed_states.last().expect("speed state");
    assert_eq!(speed_state.speed, 4.0);
    assert!(!speed_state.paused);

    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 1);

    task.on_set_room_time_speed(1, 0.0);
    let paused_states = room_time_states(&mut writer);
    let paused_state = paused_states.last().expect("paused state");
    assert_eq!(paused_state.current_tick, 1);
    assert_eq!(paused_state.speed, 0.0);
    assert!(paused_state.paused);
    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 1);

    task.on_step_room_time(1);
    task.on_seek_room_time(1, u32::MAX);
    task.on_seek_room_time_to(1, 0);
    assert_eq!(
        in_game_tick(&task),
        1,
        "AI-only live exposes speed/pause but not step or seek"
    );

    task.on_set_room_time_speed(1, 2.0);
    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 2);
}

#[test]
fn normal_live_spectator_start_payload_is_read_only() {
    let mut task = RoomTask::new(
        "live-spectator-readonly-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_player = add_test_room_player(&mut task, 1, true);
    let mut writer_spectator = add_test_room_spectator(&mut task, 99);

    task.start_match();
    let start_messages: Vec<_> =
        std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
    assert!(start_messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::Start(payload)
                if payload.player_id == 99
                    && payload.spectator
                    && payload.prediction_build_id.is_none()
                    && payload.prediction_version == 0
                    && payload.replay.is_none()
        )
    }));

    task.on_command(
        99,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );
    task.on_tick(TokioInstant::now());

    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    assert!(game.command_log().is_empty());
    assert!(task.pending_client_command_acks.is_empty());
    assert_eq!(task.players.get(&99).unwrap().last_received_client_seq, 0);
    let snapshot = writer_spectator
        .snapshots
        .take()
        .expect("spectator snapshot");
    assert_eq!(snapshot.net_status.prediction_version, 0);
    assert_eq!(snapshot.net_status.last_sim_consumed_client_seq, 0);
}

#[test]
fn late_spectator_join_gets_read_only_live_start_and_snapshot() {
    let mut task = RoomTask::new(
        "late-spectator-live-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_player = add_test_room_player(&mut task, 1, true);
    task.start_match();
    task.on_tick(TokioInstant::now());
    let current_tick = in_game_tick(&task);

    let (msg_tx, mut writer_spectator) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Late Spectator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    let player = task.players.get(&99).expect("late spectator inserted");
    assert!(player.spectator);
    assert!(player.ready);
    assert_eq!(player.color, "#6f8fa8");
    assert!(!task.human_team_assignments.contains_key(&99));
    assert!(!task.human_faction_assignments.contains_key(&99));
    assert_eq!(task.match_player_count, 1);
    assert_eq!(task.active_human_count(), 1);

    let payload = start_payloads(&mut writer_spectator)
        .pop()
        .expect("late spectator start payload");
    assert_eq!(payload.player_id, 99);
    assert!(payload.spectator);
    assert!(payload.prediction_build_id.is_none());
    assert_eq!(payload.prediction_version, 0);
    assert_eq!(payload.tick, current_tick);
    assert_eq!(payload.players.len(), 1);
    assert_eq!(payload.players[0].id, 1);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);
    assert!(payload.diagnostics.observer_analysis);

    task.on_tick(TokioInstant::now());
    let snapshot = writer_spectator
        .snapshots
        .take()
        .expect("late spectator snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("normal live match should remain active");
    };
    let mut expected = game.snapshot_for_spectator(&[1]);
    compact_snapshot_for_wire(&mut expected);
    assert_eq!(snapshot.tick, expected.tick);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    assert_eq!(snapshot.player_resources, expected.player_resources);
    assert_eq!(snapshot.net_status.prediction_version, 0);
    assert_eq!(snapshot.net_status.last_sim_consumed_client_seq, 0);
    let tick_messages: Vec<_> =
        std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
    assert!(tick_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::ObserverAnalysis(analysis)
            if analysis.tick == expected.tick && !analysis.players.is_empty()
    )));
}

#[test]
fn late_spectator_notice_targets_existing_recipients_once() {
    let mut task = RoomTask::new(
        "late-spectator-notice-targeting-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_active_one = add_test_room_player(&mut task, 1, true);
    let mut writer_active_two = add_test_room_player(&mut task, 2, true);
    let mut writer_existing_spectator = add_test_room_spectator(&mut task, 50);

    task.start_match();
    let _ = start_payloads(&mut writer_active_one);
    let _ = start_payloads(&mut writer_active_two);
    let _ = start_payloads(&mut writer_existing_spectator);
    task.on_tick(TokioInstant::now());
    let _ = writer_active_one.snapshots.take();
    let _ = writer_active_two.snapshots.take();
    let _ = writer_existing_spectator.snapshots.take();

    let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Late Scout".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(task.match_player_count, 2);
    assert_eq!(task.match_human_count, 2);
    assert_eq!(
        task.match_participants,
        vec!["Player 1".to_string(), "Player 2".to_string()]
    );
    let summary = task
        .lobby_summary()
        .expect("live room should stay in the public browser");
    assert_eq!(summary.spectator_count, 2);

    task.on_tick(TokioInstant::now());
    let expected = "Late Scout has joined the match as a spectator";
    assert_single_late_spectator_notice(&mut writer_active_one, expected);
    assert_single_late_spectator_notice(&mut writer_active_two, expected);
    assert_single_late_spectator_notice(&mut writer_existing_spectator, expected);
    assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
    assert!(task.pending_recipient_notices.is_empty());

    task.on_tick(TokioInstant::now());
    assert_no_late_spectator_notice(&mut writer_active_one, expected);
    assert_no_late_spectator_notice(&mut writer_active_two, expected);
    assert_no_late_spectator_notice(&mut writer_existing_spectator, expected);
    assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
}

#[test]
fn late_spectator_notice_uses_commander_for_blank_or_control_name() {
    let mut task = RoomTask::new(
        "late-spectator-notice-commander-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_active = add_test_room_player(&mut task, 1, true);
    task.start_match();
    let _ = start_payloads(&mut writer_active);
    task.on_tick(TokioInstant::now());
    let _ = writer_active.snapshots.take();

    let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, " \n\u{0007}\t ".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    task.on_tick(TokioInstant::now());
    let expected = "Commander has joined the match as a spectator";
    assert_single_late_spectator_notice(&mut writer_active, expected);
    assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
}

#[test]
fn late_spectator_notice_is_not_emitted_for_rejected_active_join() {
    let mut task = RoomTask::new(
        "late-spectator-notice-active-reject-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_active = add_test_room_player(&mut task, 1, true);
    task.start_match();
    let _ = start_payloads(&mut writer_active);
    task.on_tick(TokioInstant::now());
    let _ = writer_active.snapshots.take();

    let (msg_tx, mut writer_rejected) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Late Active".to_string(), false, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(false));
    assert!(!task.players.contains_key(&99));
    assert!(task.pending_recipient_notices.is_empty());
    assert!(matches!(
        writer_rejected.reliable_rx.try_recv().unwrap(),
        ServerMessage::Error { msg } if msg.contains("join as a spectator")
    ));

    task.on_tick(TokioInstant::now());
    assert_no_late_spectator_notice(
        &mut writer_active,
        "Late Active has joined the match as a spectator",
    );
}

#[test]
fn late_spectator_notice_queues_while_live_paused() {
    let mut task = RoomTask::new(
        "late-spectator-notice-live-pause-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_active_one = add_test_room_player(&mut task, 1, true);
    let mut writer_active_two = add_test_room_player(&mut task, 2, true);

    task.start_match();
    let _ = start_payloads(&mut writer_active_one);
    let _ = start_payloads(&mut writer_active_two);
    task.on_tick(TokioInstant::now());
    let _ = writer_active_one.snapshots.take();
    let _ = writer_active_two.snapshots.take();
    task.on_pause_game(1);
    assert!(task.live_paused);

    let (msg_tx, mut writer_new_spectator) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Paused Scout".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(!task.pending_recipient_notices.is_empty());
    task.on_tick(TokioInstant::now());
    assert!(
        writer_active_one.snapshots.take().is_none(),
        "paused live ticks should not fan out snapshots"
    );
    assert!(
        writer_active_two.snapshots.take().is_none(),
        "paused live ticks should not fan out snapshots"
    );

    task.on_unpause_game(2);
    task.on_tick(TokioInstant::now());
    let expected = "Paused Scout has joined the match as a spectator";
    assert_single_late_spectator_notice(&mut writer_active_one, expected);
    assert_single_late_spectator_notice(&mut writer_active_two, expected);
    assert_no_late_spectator_notice(&mut writer_new_spectator, expected);
    assert!(task.pending_recipient_notices.is_empty());
}

#[test]
fn late_spectator_notice_lifecycle_keeps_active_match_counts() {
    let mut task = RoomTask::new(
        "late-spectator-notice-lifecycle-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_active_one = add_test_room_player(&mut task, 1, true);
    let mut writer_active_two = add_test_room_player(&mut task, 2, true);
    task.start_match();
    let _ = start_payloads(&mut writer_active_one);
    let _ = start_payloads(&mut writer_active_two);
    task.on_tick(TokioInstant::now());
    let _ = writer_active_one.snapshots.take();
    let _ = writer_active_two.snapshots.take();

    let (msg_tx, _writer_late_spectator) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Lifecycle Scout".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(task.match_player_count, 2);
    assert_eq!(task.active_human_count(), 2);
    assert_eq!(
        task.match_participants,
        vec!["Player 1".to_string(), "Player 2".to_string()]
    );

    let before_alive = match &task.phase {
        Phase::InGame(game) => game.alive_players(),
        _ => panic!("expected live match"),
    };
    assert_eq!(before_alive.len(), 2);

    task.on_leave(99);
    let summary = task
        .lobby_summary()
        .expect("live room should stay in the public browser after spectator leaves");
    assert_eq!(summary.spectator_count, 0);
    assert_eq!(task.match_player_count, 2);
    assert_eq!(task.active_human_count(), 2);
    let after_alive = match &task.phase {
        Phase::InGame(game) => game.alive_players(),
        _ => panic!("expected live match"),
    };
    assert_eq!(after_alive, before_alive);
}

#[test]
fn late_spectator_phase_rejects_active_joins_without_claiming_socket() {
    let mut task = RoomTask::new(
        "late-spectator-active-reject-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_player = add_test_room_player(&mut task, 1, true);
    task.start_match();

    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Late Active".to_string(), false, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(false));
    assert!(!task.players.contains_key(&99));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Error { msg } if msg.contains("join as a spectator")
    ));

    let mut other = RoomTask::new(
        "late-spectator-retry-room".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let (retry_tx, _retry_writer) = ConnectionSink::new();
    let (retry_ack, mut retry_ack_rx) = tokio::sync::oneshot::channel();
    other.on_join(
        99,
        "Late Active".to_string(),
        false,
        false,
        retry_tx,
        retry_ack,
    );

    assert_eq!(retry_ack_rx.try_recv(), Ok(true));
    assert!(other.players.contains_key(&99));
}

#[test]
fn replay_phase_ignores_gameplay_commands() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 0);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "replay-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_command(
        players[0].id,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );

    let Phase::ReplayViewer(replay) = &task.phase else {
        panic!("replay phase should remain active");
    };
    assert!(replay.game.command_log().is_empty());
}
