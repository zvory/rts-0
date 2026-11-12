use super::support::*;

#[test]
fn room_records_give_up_and_disconnect_for_replay_integrity() {
    for disconnect in [false, true] {
        let players = replay_test_players(2);
        let (game, start, _) = replay_test_artifact_with_start(&players, 2);
        let mut task = RoomTask::new(
            "integrity-test".into(),
            RoomMode::Normal,
            None,
            false,
            DrainHandle::default(),
        );
        let _writer_a = add_test_room_player(&mut task, 1, true);
        let _writer_b = add_test_room_player(&mut task, 2, true);
        task.match_player_count = 2;
        task.match_human_count = 2;
        task.replay_start = Some(start);
        task.phase = Phase::InGame(Box::new(game));
        if disconnect {
            task.on_leave(2);
            // Disconnect resolution happens at the next live tick.
            task.on_tick(TokioInstant::now());
        } else {
            task.on_give_up(2);
        }
        let Phase::ReplayViewer(mut replay) = std::mem::replace(&mut task.phase, Phase::Lobby)
        else {
            panic!("finished match should enter replay viewer");
        };
        assert_eq!(replay.artifact.eliminations.len(), 1);
        assert_eq!(replay.artifact.eliminations[0].player_id, 2);
        while replay.has_remaining_ticks() {
            replay.enqueue_for_current_tick().unwrap();
            replay.tick(None);
        }
        assert_eq!(
            replay.artifact.verify_end_state(replay.game()).unwrap(),
            Some(true)
        );
    }
}

#[test]
fn divergence_warning_reaches_viewers_on_playback_and_seek_after_score_was_sent() {
    let players = replay_test_players(2);
    let (_, mut artifact) = replay_test_artifact(&players, 1);
    artifact.end_state_hash.as_mut().unwrap().digest = "0".repeat(64);
    let mut task = RoomTask::new(
        "integrity-warning".into(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_spectator(&mut task, 99);
    task.phase = Phase::ReplayViewer(Box::new(ReplaySession::new(artifact).unwrap()));
    // Automatic post-match viewing has already shown a score screen to existing participants.
    task.outcome_sent.insert(99);
    task.on_tick(TokioInstant::now());
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|m| matches!(m, ServerMessage::Error { msg } if msg.contains("divergence"))));
    assert!(!messages
        .iter()
        .any(|m| matches!(m, ServerMessage::GameOver { .. })));
    task.on_seek_room_time_to(99, 1);
    task.on_tick(TokioInstant::now());
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|m| matches!(m, ServerMessage::Error { msg } if msg.contains("divergence"))));
}
