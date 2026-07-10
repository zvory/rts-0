use super::super::match_history::MatchHistoryRecordInput;
use super::support::*;

#[test]
fn match_history_persistence_marks_solo_matches_debug_and_allows_human_ai_matches() {
    let mut solo = RoomTask::new(
        "solo-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    solo.match_player_count = 1;
    solo.match_human_count = 1;
    solo.match_participants = vec!["Player".to_string()];
    assert!(solo.should_persist_match_history());
    assert!(solo.match_history_debug_mode());

    let mut human_ai = RoomTask::new(
        "human-ai-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    human_ai.match_player_count = 2;
    human_ai.match_human_count = 1;
    human_ai.match_participants = vec!["Player".to_string(), "AI 2.1".to_string()];
    assert!(human_ai.should_persist_match_history());
    assert!(!human_ai.match_history_debug_mode());
}

#[test]
fn match_history_persistence_allows_ai_only_but_skips_test_matches() {
    let mut ai_only = RoomTask::new(
        "ai-only-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    ai_only.match_player_count = 2;
    ai_only.match_human_count = 0;
    ai_only.match_participants = vec!["AI 2.1 1".to_string(), "AI 2.1 2".to_string()];
    assert!(ai_only.should_persist_match_history());
    assert!(!ai_only.match_history_debug_mode());

    let mut smoke = RoomTask::new(
        "smoke-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    smoke.match_player_count = 2;
    smoke.match_human_count = 1;
    smoke.match_participants = vec!["smoke".to_string(), "AI 2.1".to_string()];
    assert!(!smoke.should_persist_match_history());

    let mut automated_room = RoomTask::new(
        "itest-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    automated_room.match_player_count = 2;
    automated_room.match_human_count = 2;
    automated_room.match_participants = vec!["Player 1".to_string(), "Player 2".to_string()];
    assert!(!automated_room.should_persist_match_history());
}

#[test]
fn empty_live_room_clears_lifecycle_bookkeeping_and_drain_tracking() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "live-empty-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(1, "Player 1".to_string(), false, false, msg_tx, ack_tx);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    task.on_ready(1, true);
    task.on_start_request(1);

    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(drain.active_matches(), 1);
    assert!(task.match_started_at.is_some());
    assert!(task.match_run_id.is_some());
    assert_eq!(task.match_player_count, 1);
    assert_eq!(task.match_human_count, 1);
    assert!(!task.match_map_name.is_empty());
    assert_eq!(task.match_participants, vec!["Player 1".to_string()]);

    task.on_leave(1);

    assert!(matches!(task.phase, Phase::Lobby));
    assert_eq!(drain.active_matches(), 0);
    assert!(!task.match_tracked_for_drain);
    assert!(task.players.is_empty());
    assert_eq!(task.host_id, None);
    assert_eq!(task.match_player_count, 0);
    assert_eq!(task.match_human_count, 0);
    assert!(task.match_started_at.is_none());
    assert!(task.match_run_id.is_none());
    assert!(task.match_map_name.is_empty());
    assert!(task.match_participants.is_empty());
}

#[test]
fn shutdown_finalize_active_match_marks_drain_finished_before_leave_cleanup() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "shutdown-finalize-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(1, "Player 1".to_string(), false, false, msg_tx, ack_tx);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    task.on_ready(1, true);
    task.on_start_request(1);

    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(drain.active_matches(), 1);

    let result = task.finalize_for_shutdown();

    assert_eq!(
        result,
        ShutdownFinalizeResult {
            had_active_match: true,
            finalized_match: true,
            match_history_allowed: true,
            record_queued: false,
            replay_captured: false,
        }
    );
    assert!(matches!(task.phase, Phase::Lobby));
    assert_eq!(drain.active_matches(), 0);
    assert!(!task.match_tracked_for_drain);
    assert_eq!(task.match_player_count, 0);
    assert_eq!(task.match_human_count, 0);
    assert!(task.match_started_at.is_none());
    assert!(task.match_run_id.is_none());
    assert!(task.match_map_name.is_empty());
    assert!(task.match_participants.is_empty());

    task.on_leave(1);
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn shutdown_finalize_noneligible_active_match_skips_match_history() {
    let drain = DrainHandle::default();
    let mut task = RoomTask::new(
        "itest-history-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(1, "Player 1".to_string(), false, false, msg_tx, ack_tx);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    task.on_ready(1, true);
    task.on_start_request(1);

    let result = task.finalize_for_shutdown();

    assert!(result.had_active_match);
    assert!(result.finalized_match);
    assert!(!result.match_history_allowed);
    assert!(!result.record_queued);
    assert!(!result.replay_captured);
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn aborted_match_history_record_is_winnerless_and_replay_backed() {
    let players = replay_test_players(2);
    let (game, artifact) = replay_test_artifact(&players, 4);
    let mut task = RoomTask::new(
        "aborted-history-record-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.match_player_count = 2;
    task.match_human_count = 2;
    task.match_started_at = Some(chrono::Utc::now() - chrono::Duration::milliseconds(1_500));
    task.match_map_name = "Default".to_string();
    task.match_participants = players.iter().map(|player| player.name.clone()).collect();

    let ended_at = chrono::Utc::now();
    let record = task.build_match_history_record(MatchHistoryRecordInput {
        started_at: task.match_started_at.expect("match start should be set"),
        ended_at,
        duration_ms: 1_500,
        scores: &game.scores(),
        replay_artifact: Some(&artifact),
        outcome: crate::db::MatchOutcome::Aborted,
        winner_name: None,
    });

    assert_eq!(record.winner_name, None);
    assert_eq!(record.outcome, crate::db::MatchOutcome::Aborted);
    assert_eq!(record.duration_ms, 1_500);
    assert_eq!(record.map_name, "Default");
    assert_eq!(
        record.participants,
        vec!["Player 1".to_string(), "Player 2".to_string()]
    );
    assert_eq!(record.human_count, 2);
    assert!(!record.debug_mode);
    assert!(record.score_screen.as_array().is_some());
    let replay = record.replay.expect("aborted record should include replay");
    assert_eq!(
        replay.duration_ticks,
        i32::try_from(artifact.duration_ticks).unwrap()
    );
    assert_eq!(replay.map_name, artifact.map_name);
    assert_eq!(
        replay.artifact_schema_version,
        i32::try_from(artifact.artifact_schema_version).unwrap()
    );
}

#[test]
fn end_match_transitions_all_connected_players_to_tick_zero_replay() {
    let players = replay_test_players(2);
    let (game, replay_start, _artifact) = replay_test_artifact_with_start(&players, 3);
    let mut task = RoomTask::new(
        "post-match-replay-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_test_room_player(&mut task, players[0].id, true);
    let mut writer_b = add_test_room_player(&mut task, players[1].id, true);
    task.match_player_count = 2;
    task.match_human_count = 2;
    task.replay_start = Some(replay_start);
    task.outcome_sent.insert(players[1].id);

    task.players
        .get(&players[0].id)
        .unwrap()
        .msg_tx
        .try_send_snapshot(replay_transition_test_snapshot(99));
    task.players
        .get(&players[1].id)
        .unwrap()
        .msg_tx
        .try_send_snapshot(replay_transition_test_snapshot(100));

    task.end_match(Some(players[0].id), game.scores(), Some(&game));

    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("match should transition into replay viewer");
    };
    assert_eq!(session.current_tick(), 0);
    assert_eq!(session.speed(), ReplaySession::DEFAULT_SPEED);
    assert_eq!(session.vision_player_ids_for(players[0].id), vec![1, 2]);
    assert!(writer_a.snapshots.take().is_none());
    assert!(writer_b.snapshots.take().is_none());

    let a_messages: Vec<_> = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
    let b_messages: Vec<_> = std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
    assert!(a_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::GameOver { .. })));
    assert!(a_messages.iter().any(|msg| {
        matches!(msg, ServerMessage::Start(payload) if payload.replay.is_some() && payload.tick == 0)
    }));
    assert!(a_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::RoomTimeState(state) if state.current_tick == 0)));
    assert!(!b_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::GameOver { .. })));
    assert!(b_messages.iter().any(|msg| {
        matches!(msg, ServerMessage::Start(payload) if payload.replay.is_some() && payload.tick == 0)
    }));
    assert!(b_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::RoomTimeState(state) if state.current_tick == 0)));
}

#[test]
fn dedicated_replay_room_return_to_lobby_does_not_stop_other_viewers() {
    let players = replay_test_players(2);
    let (_game, artifact) = replay_test_artifact(&players, 2);
    let mut task = RoomTask::new(
        "persisted-replay-return-test".to_string(),
        RoomMode::Replay { artifact },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx_a, _writer_a) = ConnectionSink::new();
    let (ack_a, mut ack_rx_a) = tokio::sync::oneshot::channel();
    let (msg_tx_b, writer_b) = ConnectionSink::new();
    let (ack_b, mut ack_rx_b) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer A".to_string(), true, true, msg_tx_a, ack_a);
    task.on_join(100, "Viewer B".to_string(), true, true, msg_tx_b, ack_b);

    assert_eq!(ack_rx_a.try_recv(), Ok(true));
    assert_eq!(ack_rx_b.try_recv(), Ok(true));
    task.on_start_request(99);
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));

    task.on_return_to_lobby(99);

    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    assert!(!task.players.contains_key(&99));
    assert!(task.players.contains_key(&100));

    task.on_tick_replay_viewer(TokioInstant::now());
    assert!(
        writer_b.snapshots.take().is_some(),
        "other viewers should keep receiving replay snapshots"
    );
}
