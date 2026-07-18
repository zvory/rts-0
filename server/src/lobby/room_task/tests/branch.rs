use super::support::*;

#[test]
fn replay_branch_request_rejects_outside_replay_viewer() {
    let mut task = RoomTask::new(
        "branch-outside-replay-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer = add_test_room_player(&mut task, 99, true);

    let err = match task.on_request_branch_from_tick(99) {
        Ok(_) => panic!("branch request outside replay should fail"),
        Err(err) => err,
    };

    assert!(
        err.contains("outside replay playback"),
        "unexpected branch reject: {err}"
    );
    assert!(matches!(task.phase, Phase::Lobby));
}

#[test]
fn replay_branch_seed_captures_current_authoritative_tick() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 5);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    let mut task = RoomTask::new(
        "branch-current-tick-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    let seed = task.on_request_branch_from_tick(99).unwrap();

    assert_eq!(seed.source_tick, 3);
    assert_eq!(seed.game.tick_count(), 3);
    assert_eq!(seed.source_replay.duration_ticks, 5);
    assert_eq!(seed.seats.len(), 2);
    assert!(seed.seats.iter().all(|seat| seat.claimable));
}

#[test]
fn replay_branch_seed_preserves_team_and_faction_ids() {
    let mut players = replay_test_players(4);
    players[0].team_id = 1;
    players[1].team_id = 1;
    players[2].team_id = 2;
    players[3].team_id = 2;
    let (_live, artifact) = replay_test_artifact(&players, 0);
    let replay = ReplaySession::new(artifact).unwrap();
    let seed = replay.branch_seed().unwrap();

    assert_eq!(
        seed.seats
            .iter()
            .map(|seat| seat.team_id)
            .collect::<Vec<_>>(),
        vec![1, 1, 2, 2]
    );
    assert!(seed
        .seats
        .iter()
        .all(|seat| seat.faction_id == DEFAULT_FACTION_ID));

    let mut old_players = replay_test_players(2);
    old_players[0].team_id = 0;
    old_players[1].team_id = 0;
    let (_live, old_artifact) = replay_test_artifact(&old_players, 0);
    let old_replay = ReplaySession::new(old_artifact).unwrap();
    let old_seed = old_replay.branch_seed().unwrap();

    assert_eq!(
        old_seed
            .seats
            .iter()
            .map(|seat| seat.team_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(old_seed
        .seats
        .iter()
        .all(|seat| seat.faction_id == DEFAULT_FACTION_ID));
}

#[test]
fn replay_branch_request_keeps_source_replay_session_intact() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "branch-source-intact-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    let seed = task.on_request_branch_from_tick(99).unwrap();
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("source room should remain a replay viewer");
    };

    assert_eq!(session.current_tick(), 0);
    assert_eq!(session.duration_ticks, 4);
    assert_eq!(session.artifact.command_log.len(), 0);
    assert_eq!(seed.game.tick_count(), session.current_tick());
}

#[test]
fn replay_branch_request_rejects_ai_seats_without_creating_seed() {
    let mut players = replay_test_players(2);
    players[1].is_ai = true;
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "branch-ai-reject-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));
    task.send_replay_start_to(99);

    let err = match task.on_request_branch_from_tick(99) {
        Ok(_) => panic!("branch request with AI seats should fail"),
        Err(err) => err,
    };
    let payload = start_payloads(&mut writer)
        .pop()
        .expect("AI replay viewer should receive a replay start payload");

    assert!(err.contains("AI seats"), "unexpected branch reject: {err}");
    assert!(!payload.capabilities.actions.branch_from_tick);
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
}

#[test]
fn replay_branch_announcement_broadcasts_to_all_viewers() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 0);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "branch-broadcast-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_test_room_player(&mut task, 100, true);
    let mut writer_b = add_test_room_player(&mut task, 101, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_announce_branch_from_tick(
        "__replay_branch__:00000001".to_string(),
        12,
        vec![ReplayBranchSeat {
            player_id: players[0].id,
            team_id: players[0].team_id,
            faction_id: players[0].faction_id.clone(),
            name: players[0].name.clone(),
            color: players[0].color.clone(),
            claimable: true,
        }],
    );

    for writer in [&mut writer_a, &mut writer_b] {
        let msg = writer.reliable_rx.try_recv().expect("branch message");
        assert!(matches!(
            msg,
            ServerMessage::BranchFromTickCreated {
                branch_room,
                source_tick: 12,
                seats
            } if branch_room == "__replay_branch__:00000001"
                && seats.len() == 1
                && seats[0].player_id == players[0].id
        ));
    }
}

#[test]
fn branch_staging_seat_claims_are_exclusive() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 2);
    let mut task = RoomTask::new(
        "branch-exclusive-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_branch_occupant(&mut task, 100);
    let mut writer_b = add_branch_occupant(&mut task, 101);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

    task.on_claim_branch_seat(100, players[0].id);
    task.on_claim_branch_seat(101, players[0].id);

    let messages = branch_staging_messages(&mut writer_a);
    let last = messages.last().expect("branch staging update");
    assert!(matches!(
        last,
        ServerMessage::BranchStaging { seats, .. }
            if seats[0].claimant_id == Some(100)
    ));
    assert!(
        std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).any(|msg| {
            matches!(msg, ServerMessage::Error { msg } if msg.contains("already claimed"))
        })
    );
}

#[test]
fn branch_staging_one_occupant_cannot_claim_multiple_seats() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-single-claim-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_branch_occupant(&mut task, 100);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

    task.on_claim_branch_seat(100, players[0].id);
    task.on_claim_branch_seat(100, players[1].id);

    assert!(std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
        matches!(msg, ServerMessage::Error { msg } if msg.contains("already claimed a branch seat"))
    }));
    let Phase::BranchStaging(staging) = &task.phase else {
        panic!("branch staging should stay active");
    };
    assert_eq!(staging.claimant_for_occupant(100), Some(players[0].id));
    assert_eq!(staging.claimant_for_seat(players[1].id), None);
}

#[test]
fn branch_staging_requires_all_original_seats_before_can_start() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-can-start-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_branch_occupant(&mut task, 100);
    let _writer_b = add_branch_occupant(&mut task, 101);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

    task.broadcast_branch_staging();
    task.on_claim_branch_seat(100, players[0].id);
    task.on_claim_branch_seat(101, players[1].id);

    let updates = branch_staging_messages(&mut writer_a);
    assert!(matches!(
        updates.first(),
        Some(ServerMessage::BranchStaging {
            can_start: false,
            ..
        })
    ));
    assert!(matches!(
        updates.last(),
        Some(ServerMessage::BranchStaging {
            can_start: true,
            ..
        })
    ));
}

#[test]
fn branch_launch_preparation_preserves_original_replay_seat_mapping() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut staging = BranchStagingState::new(seed);
    staging.claim(101, players[1].id).unwrap();
    staging.claim(100, players[0].id).unwrap();

    let launch = staging
        .prepare_launch(|connection_id| matches!(connection_id, 100 | 101))
        .unwrap();

    assert_eq!(launch.seat_by_connection.get(&100), Some(&players[0].id));
    assert_eq!(launch.seat_by_connection.get(&101), Some(&players[1].id));
    assert_eq!(
        launch.participants,
        vec![players[0].name.clone(), players[1].name.clone()]
    );
    assert_eq!(launch.game.tick_count(), 0);
}

#[test]
fn branch_staging_allows_release_and_reclaim() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-release-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_branch_occupant(&mut task, 100);
    let mut writer_b = add_branch_occupant(&mut task, 101);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

    task.on_claim_branch_seat(100, players[0].id);
    task.on_release_branch_seat(100, players[0].id);
    task.on_claim_branch_seat(101, players[0].id);

    let updates = branch_staging_messages(&mut writer_b);
    assert!(matches!(
        updates.last(),
        Some(ServerMessage::BranchStaging { seats, .. })
            if seats[0].claimant_id == Some(101)
    ));
    let Phase::BranchStaging(staging) = &task.phase else {
        panic!("branch staging should stay active");
    };
    assert_eq!(staging.claimant_for_occupant(100), None);
    assert_eq!(staging.claimant_for_occupant(101), Some(players[0].id));
}

#[test]
fn branch_staging_leave_releases_claim_and_reassigns_host() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-leave-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_branch_occupant(&mut task, 100);
    let mut writer_b = add_branch_occupant(&mut task, 101);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
    task.on_claim_branch_seat(100, players[0].id);

    task.on_leave(100);

    assert_eq!(task.host_id, Some(101));
    let updates = branch_staging_messages(&mut writer_b);
    assert!(matches!(
        updates.last(),
        Some(ServerMessage::BranchStaging { seats, host_id: 101, .. })
            if seats[0].claimant_id.is_none()
    ));
}

#[test]
fn branch_staging_rejects_normal_lobby_only_controls() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-lobby-controls-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_branch_occupant(&mut task, 100);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));
    task.host_id = Some(100);

    task.on_ready(100, true);
    task.on_add_ai(100, None, None);
    task.on_remove_ai(100, 999);
    task.on_set_spectator(100, 100, false);
    task.on_select_map(100, "Badlands".to_string());
    task.on_start_request(100);

    assert!(task.ai_players.is_empty());
    assert_eq!(task.selected_map, "1v1");
    assert!(matches!(task.phase, Phase::BranchStaging(_)));
    assert!(!std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .any(|msg| matches!(msg, ServerMessage::Lobby { .. } | ServerMessage::Start(_))));
}

#[test]
fn branch_launch_countdown_promotes_to_live_start_payloads() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 3);
    let mut task = RoomTask::new(
        "branch-promote-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_branch_occupant(&mut task, 100);
    let mut writer_b = add_branch_occupant(&mut task, 101);
    let mut writer_spectator = add_branch_occupant(&mut task, 102);
    task.phase = Phase::BranchStaging(Box::new(BranchStagingState::new(seed)));

    task.on_claim_branch_seat(100, players[0].id);
    task.on_claim_branch_seat(101, players[1].id);
    task.on_start_branch(100);

    assert!(std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok())
        .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
    std::thread::sleep(match_countdown_duration().saturating_mul(2));
    task.on_tick(TokioInstant::now());

    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(
        task.branch_live_seat_by_connection.get(&100),
        Some(&players[0].id)
    );
    assert_eq!(
        task.branch_live_seat_by_connection.get(&101),
        Some(&players[1].id)
    );
    assert!(!task.branch_live_seat_by_connection.contains_key(&102));
    assert!(!task.players.get(&100).unwrap().spectator);
    assert!(!task.players.get(&101).unwrap().spectator);
    assert!(task.players.get(&102).unwrap().spectator);

    let starts_b: Vec<_> = std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
    let start_b = starts_b
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .expect("branch active seat should receive start payload");
    assert_eq!(start_b.player_id, players[1].id);
    assert!(!start_b.spectator);
    assert!(start_b.prediction_build_id.is_some());
    assert_eq!(start_b.prediction_version, PREDICTION_PROTOCOL_VERSION);
    assert!(start_b.replay.is_none());
    assert!(start_b.lab.is_none());
    assert!(start_b.capabilities.commands.gameplay);
    assert!(start_b.capabilities.match_controls.pause);
    assert!(!start_b.capabilities.room_time.available);
    assert!(!start_b.capabilities.actions.branch_from_tick);
    assert!(start_b
        .players
        .iter()
        .all(|player| player.faction_id == DEFAULT_FACTION_ID));

    let starts_spectator: Vec<_> =
        std::iter::from_fn(|| writer_spectator.reliable_rx.try_recv().ok()).collect();
    let spectator_start = starts_spectator
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .expect("branch observer should receive start payload");
    assert_eq!(spectator_start.player_id, 102);
    assert!(spectator_start.spectator);
    assert!(spectator_start.prediction_build_id.is_none());
    assert_eq!(spectator_start.prediction_version, 0);
    assert!(spectator_start.replay.is_none());
    assert!(spectator_start.lab.is_none());
    assert!(!spectator_start.capabilities.commands.gameplay);
    assert!(spectator_start.capabilities.match_controls.pause);
    assert!(spectator_start.diagnostics.observer_analysis);
}

#[test]
fn branch_live_launch_rejects_unsupported_recorded_faction_ids() {
    let players = replay_test_players(2);
    let mut seed = replay_branch_test_seed(&players, 0);
    seed.seats[0].faction_id = EMPTY_FIXTURE_FACTION_ID.to_string();
    let mut task = RoomTask::new(
        "branch-faction-reject-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_branch_occupant(&mut task, 100);
    let mut writer_b = add_branch_occupant(&mut task, 101);
    let mut staging = BranchStagingState::new(seed);
    staging.claim(100, players[0].id).unwrap();
    staging.claim(101, players[1].id).unwrap();
    task.phase = Phase::BranchStaging(Box::new(staging));

    task.start_branch_live();

    assert!(matches!(task.phase, Phase::BranchStaging(_)));
    assert!(task.branch_live_seat_by_connection.is_empty());
    let a_messages: Vec<_> = std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).collect();
    let b_messages: Vec<_> = std::iter::from_fn(|| writer_b.reliable_rx.try_recv().ok()).collect();
    assert!(!a_messages
        .iter()
        .chain(b_messages.iter())
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
}

#[test]
fn branch_live_commands_and_snapshots_use_mapped_original_seats() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-live-map-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let writer_a = add_branch_occupant(&mut task, 100);
    let writer_b = add_branch_occupant(&mut task, 101);
    let writer_spectator = add_branch_occupant(&mut task, 102);
    let mut staging = BranchStagingState::new(seed);
    staging.claim(100, players[0].id).unwrap();
    staging.claim(101, players[1].id).unwrap();
    task.phase = Phase::BranchStaging(Box::new(staging));
    task.start_branch_live();

    task.on_command(
        100,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );
    task.on_command(
        102,
        1,
        SimCommand::Stop {
            units: vec![4, 5, 6],
        },
    );
    assert_eq!(task.pending_client_command_acks.len(), 1);
    assert_eq!(task.pending_client_command_acks[0].connection_id, 100);
    assert_eq!(task.pending_client_command_acks[0].client_seq, 1);
    task.on_tick(TokioInstant::now());

    let snapshot_a = writer_a.snapshots.take().expect("claimed A snapshot");
    let snapshot_b = writer_b.snapshots.take().expect("claimed B snapshot");
    let snapshot_spectator = writer_spectator
        .snapshots
        .take()
        .expect("spectator snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("branch should be live");
    };
    assert_eq!(game.command_log().len(), 1);
    assert_eq!(game.command_log()[0].player_id, players[0].id);
    assert_eq!(
        task.players.get(&100).unwrap().last_sim_consumed_client_seq,
        1
    );
    assert_eq!(task.players.get(&102).unwrap().last_received_client_seq, 0);
    assert_eq!(
        snapshot_a.visible_tiles,
        game.snapshot_for(players[0].id).visible_tiles
    );
    assert_eq!(
        snapshot_b.visible_tiles,
        game.snapshot_for(players[1].id).visible_tiles
    );
    assert_eq!(
        snapshot_spectator.visible_tiles,
        game.snapshot_for_observer(&ObserverView::Omniscient)
            .visible_tiles
    );
}

#[test]
fn branch_live_late_join_start_payload_attaches_as_observer_without_resetting_staging() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-live-late-join-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_branch_occupant(&mut task, 100);
    let _writer_b = add_branch_occupant(&mut task, 101);
    let mut staging = BranchStagingState::new(seed);
    staging.claim(100, players[0].id).unwrap();
    staging.claim(101, players[1].id).unwrap();
    task.phase = Phase::BranchStaging(Box::new(staging));
    task.start_branch_live();
    let original_branch_live_seats = task.branch_live_seat_by_connection.clone();

    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        103,
        "Late Branch Viewer".to_string(),
        false,
        false,
        msg_tx,
        ack,
    );

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(task.phase, Phase::InGame(_)));
    assert_eq!(
        task.branch_live_seat_by_connection,
        original_branch_live_seats
    );
    assert!(!task.branch_live_seat_by_connection.contains_key(&103));
    assert!(task.players.get(&103).unwrap().spectator);
    assert_eq!(task.host_id, Some(100));

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::BranchStaging { .. })));
    let start = messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .expect("late branch observer should receive a live start payload");
    assert_eq!(start.player_id, 103);
    assert!(start.spectator);
    assert_eq!(start.prediction_build_id, None);
    assert_eq!(start.prediction_version, 0);
    let mut expected_capabilities =
        SessionPolicy::for_room(&task.mode, SessionPhase::LiveMatch).start_capabilities(false);
    expected_capabilities.visibility.vision_selection = true;
    assert_eq!(start.capabilities, expected_capabilities);
    assert!(start.diagnostics.observer_analysis);
    assert!(!start.capabilities.commands.gameplay);
    assert!(start.capabilities.match_controls.pause);
}

#[test]
fn branch_live_give_up_resolves_by_original_seat_and_skips_public_history() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 0);
    let mut task = RoomTask::new(
        "branch-give-up-test".to_string(),
        RoomMode::ReplayBranch { seed: seed.clone() },
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer_a = add_branch_occupant(&mut task, 100);
    let _writer_b = add_branch_occupant(&mut task, 101);
    let mut staging = BranchStagingState::new(seed);
    staging.claim(100, players[0].id).unwrap();
    staging.claim(101, players[1].id).unwrap();
    task.phase = Phase::BranchStaging(Box::new(staging));
    task.start_branch_live();

    assert!(!task.should_persist_match_history());
    task.on_give_up(100);

    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    assert!(
        std::iter::from_fn(|| writer_a.reliable_rx.try_recv().ok()).any(|msg| {
            matches!(msg, ServerMessage::GameOver { winner_id: Some(id), you, .. }
            if id == players[1].id && you == "lost")
        })
    );
    assert!(task.branch_live_seat_by_connection.is_empty());
}

#[test]
fn empty_branch_room_preserves_private_branch_identity() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 1);
    let mut task = RoomTask::new(
        "__replay_branch__:empty-test".to_string(),
        RoomMode::ReplayBranch { seed },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack_tx, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(100, "Viewer".to_string(), true, false, msg_tx, ack_tx);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(task.host_id, Some(100));
    assert!(matches!(task.phase, Phase::BranchStaging(_)));

    task.on_leave(100);

    assert!(!task.should_dispose_when_empty());
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(matches!(task.mode, RoomMode::ReplayBranch { .. }));
    assert!(task.players.is_empty());
    assert_eq!(task.host_id, None);
    assert!(task.lobby_summary().is_none());
}

#[test]
fn replay_branch_room_join_initializes_staging_and_broadcasts_seats() {
    let players = replay_test_players(2);
    let seed = replay_branch_test_seed(&players, 2);
    let mut task = RoomTask::new(
        "branch-join-baseline-test".to_string(),
        RoomMode::ReplayBranch { seed },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(100, "Viewer 100".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(task.host_id, Some(100));
    assert!(matches!(task.phase, Phase::BranchStaging(_)));
    let updates = branch_staging_messages(&mut writer);
    let Some(ServerMessage::BranchStaging {
        room,
        source_tick,
        host_id,
        seats,
        occupants,
        can_start,
    }) = updates.last()
    else {
        panic!("branch join should broadcast staging state");
    };
    assert_eq!(room, "branch-join-baseline-test");
    assert_eq!(*source_tick, 2);
    assert_eq!(*host_id, 100);
    assert_eq!(seats.len(), players.len());
    assert!(seats.iter().all(|seat| seat.claimant_id.is_none()));
    assert_eq!(occupants.len(), 1);
    assert_eq!(occupants[0].id, 100);
    assert_eq!(occupants[0].name, "Viewer 100");
    assert!(!can_start);
}
