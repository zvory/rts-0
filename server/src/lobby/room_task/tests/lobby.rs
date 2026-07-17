use super::support::*;

#[test]
fn lobby_name_update_changes_roster_and_host_summary() {
    let mut task = summary_task("rename-lobby");
    let mut observer = add_test_room_spectator(&mut task, 2);

    task.on_set_name(1, "Renamed".to_string());

    assert_eq!(
        task.players.get(&1).map(|player| player.name.as_str()),
        Some("Renamed")
    );
    assert_eq!(
        task.lobby_summary().and_then(|summary| summary.host_name),
        Some("Renamed".to_string())
    );
    let lobby = std::iter::from_fn(|| observer.reliable_rx.try_recv().ok())
        .find_map(|message| match message {
            ServerMessage::Lobby { players, .. } => Some(players),
            _ => None,
        })
        .expect("rename should broadcast an updated lobby roster");
    assert_eq!(
        lobby
            .iter()
            .find(|player| player.id == 1)
            .map(|player| player.name.as_str()),
        Some("Renamed")
    );
}

#[test]
fn lobby_name_update_is_ignored_during_countdown() {
    let mut task = summary_task("countdown-rename");
    task.match_countdown_deadline = Some(TokioInstant::now() + Duration::from_secs(3));

    task.on_set_name(1, "Too Late".to_string());

    assert_eq!(
        task.players.get(&1).map(|player| player.name.as_str()),
        Some("Player 1")
    );
}

#[test]
fn lobby_summary_reports_open_waiting_room_state() {
    let task = summary_task("open-summary");

    let summary = task
        .lobby_summary()
        .expect("normal hosted lobby should be summarized");

    assert_eq!(summary.room, "open-summary");
    assert_eq!(summary.kind, crate::protocol::LobbyKind::Normal);
    assert_eq!(summary.host_name.as_deref(), Some("Player 1"));
    assert_eq!(summary.map, "1v1");
    assert_eq!(summary.created_at_unix_ms, 123_456);
    assert_eq!(summary.occupied_slots, 1);
    assert_eq!(summary.max_slots, 2);
    assert_eq!(summary.spectator_count, 0);
    assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
    assert_eq!(summary.join_state, LobbyJoinState::Open);
}

#[test]
fn lobby_summary_marks_full_waiting_rooms_spectator_joinable() {
    let mut task = summary_task("full-summary");
    add_test_room_player(&mut task, 2, false);
    task.assign_missing_team_for(2);
    task.assign_missing_faction_for(2);
    add_test_room_spectator(&mut task, 99);

    let summary = task
        .lobby_summary()
        .expect("full waiting lobby should remain visible");

    assert_eq!(summary.occupied_slots, 2);
    assert_eq!(summary.max_slots, 2);
    assert_eq!(summary.spectator_count, 1);
    assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
    assert_eq!(summary.join_state, LobbyJoinState::FullSpectatorOnly);
}

#[test]
fn lobby_summary_marks_countdown_as_starting() {
    let mut task = summary_task("countdown-summary");
    add_test_room_player(&mut task, 2, true);
    task.match_countdown_deadline = Some(TokioInstant::now() + Duration::from_secs(3));

    let summary = task
        .lobby_summary()
        .expect("countdown lobby should remain visible");

    assert_eq!(summary.phase, LobbySummaryPhase::Countdown);
    assert_eq!(summary.join_state, LobbyJoinState::Starting);
}

#[test]
fn lobby_summary_includes_live_normal_rooms_as_non_joinable() {
    let mut task = summary_task("ingame-summary");
    let players = replay_test_players(2);
    task.phase = Phase::InGame(Box::new(replay_test_game(&players, 0)));
    task.match_map_name = "Chokes".to_string();

    let summary = task
        .lobby_summary()
        .expect("normal live room should remain visible");

    assert_eq!(summary.map, "Chokes");
    assert_eq!(summary.phase, LobbySummaryPhase::InGame);
    assert_eq!(summary.join_state, LobbyJoinState::InGame);
}

#[test]
fn lobby_summary_marks_persisted_replay_lobbies() {
    let replay_players = replay_test_players(2);
    let (_live, replay_artifact) = replay_test_artifact(&replay_players, 0);

    let mut persisted_replay = RoomTask::new(
        "__match_replay__:00000001".to_string(),
        RoomMode::Replay {
            artifact: replay_artifact,
        },
        None,
        false,
        DrainHandle::default(),
    );
    persisted_replay.created_at_unix_ms = 123_456;
    persisted_replay.host_id = Some(1);
    add_test_room_spectator(&mut persisted_replay, 1);

    let summary = persisted_replay
        .lobby_summary()
        .expect("persisted replay staging lobby should be summarized");

    assert_eq!(summary.room, "__match_replay__:00000001");
    assert_eq!(summary.kind, crate::protocol::LobbyKind::Replay);
    assert_eq!(summary.map, "Chokes");
    assert_eq!(summary.occupied_slots, 0);
    assert_eq!(summary.max_slots, 0);
    assert_eq!(summary.spectator_count, 1);
    assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
    assert_eq!(summary.join_state, LobbyJoinState::FullSpectatorOnly);
}

#[test]
fn lobby_summary_hides_internal_room_modes() {
    let replay_players = replay_test_players(2);
    let branch_seed = replay_branch_test_seed(&replay_players, 0);

    let mut lab = RoomTask::new(
        "__lab__:sandbox:map=Chokes".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    lab.host_id = Some(1);
    add_test_room_spectator(&mut lab, 1);
    assert!(lab.lobby_summary().is_none());

    let mut saved_replay = RoomTask::new(
        "__replay_artifact__:demo".to_string(),
        RoomMode::ReplayArtifact {
            artifact: "demo".to_string(),
        },
        None,
        false,
        DrainHandle::default(),
    );
    saved_replay.host_id = Some(1);
    add_test_room_spectator(&mut saved_replay, 1);
    assert!(saved_replay.lobby_summary().is_none());

    let mut branch = RoomTask::new(
        "__replay_branch__:00000001".to_string(),
        RoomMode::ReplayBranch { seed: branch_seed },
        None,
        false,
        DrainHandle::default(),
    );
    branch.host_id = Some(1);
    add_test_room_spectator(&mut branch, 1);
    assert!(branch.lobby_summary().is_none());

    let mut dev = RoomTask::new(
        "__dev_scenario__:demo".to_string(),
        RoomMode::DevScenario(DevScenarioConfig {
            id: DevScenarioId::DirectReverseOrder,
            unit: EntityKind::Worker,
            count: 1,
            blocker: None,
            case: None,
        }),
        None,
        false,
        DrainHandle::default(),
    );
    dev.host_id = Some(1);
    add_test_room_spectator(&mut dev, 1);
    assert!(dev.lobby_summary().is_none());
}

#[test]
fn set_faction_accepts_playable_and_rejects_fixture_in_lobby() {
    let mut task = RoomTask::new(
        "faction-lobby-policy".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.host_id = Some(1);
    add_test_room_player(&mut task, 1, true);
    task.assign_missing_faction_for(1);

    task.on_set_faction(1, EKAT_FACTION_ID.to_string());
    assert_eq!(
        task.human_faction_assignments.get(&1).map(String::as_str),
        Some(EKAT_FACTION_ID)
    );

    task.on_set_faction(1, EMPTY_FIXTURE_FACTION_ID.to_string());
    assert_eq!(
        task.human_faction_assignments.get(&1).map(String::as_str),
        Some(EKAT_FACTION_ID),
        "fixture-only catalog ids must not overwrite a playable lobby selection"
    );

    task.on_set_faction(1, "unknown_faction".to_string());
    assert_eq!(
        task.human_faction_assignments.get(&1).map(String::as_str),
        Some(EKAT_FACTION_ID),
        "unknown catalog ids must be ignored"
    );
}

#[test]
fn set_faction_is_ignored_for_spectators_countdown_and_in_game() {
    let mut spectator_task = RoomTask::new(
        "faction-spectator-policy".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_spectator(&mut spectator_task, 1);
    spectator_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
    assert!(
        !spectator_task.human_faction_assignments.contains_key(&1),
        "spectator setFaction requests must not create active-seat faction state"
    );

    let mut countdown_task = RoomTask::new(
        "faction-countdown-policy".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut countdown_task, 1, true);
    countdown_task.assign_missing_faction_for(1);
    countdown_task.match_countdown_deadline = Some(TokioInstant::now());
    countdown_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
    assert_eq!(
        countdown_task
            .human_faction_assignments
            .get(&1)
            .map(String::as_str),
        Some(DEFAULT_FACTION_ID),
        "countdown setFaction requests must preserve the pre-countdown selection"
    );

    let players = replay_test_players(2);
    let mut in_game_task = RoomTask::new(
        "faction-in-game-policy".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut in_game_task, 1, true);
    in_game_task.assign_missing_faction_for(1);
    in_game_task.phase = Phase::InGame(Box::new(replay_test_game(&players, 0)));
    in_game_task.on_set_faction(1, EKAT_FACTION_ID.to_string());
    assert_eq!(
        in_game_task
            .human_faction_assignments
            .get(&1)
            .map(String::as_str),
        Some(DEFAULT_FACTION_ID),
        "in-game setFaction requests must not mutate active match faction state"
    );
}

#[test]
fn ai_colors_start_at_accessibility_palette_head_without_humans() {
    let task = RoomTask::new(
        "ai-colors".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );

    let colors: Vec<String> = (0..4).map(|seat| task.ai_color(seat)).collect();

    assert_eq!(
        colors,
        vec![
            PLAYER_PALETTE[0].to_string(),
            PLAYER_PALETTE[1].to_string(),
            PLAYER_PALETTE[2].to_string(),
            PLAYER_PALETTE[3].to_string(),
        ]
    );
}

#[test]
fn ai_colors_skip_active_human_colors_in_palette_order() {
    let mut task = RoomTask::new(
        "mixed-colors".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    add_test_room_player(&mut task, 1, false);

    let colors: Vec<String> = (0..3).map(|seat| task.ai_color(seat)).collect();

    assert_eq!(
        colors,
        vec![
            PLAYER_PALETTE[1].to_string(),
            PLAYER_PALETTE[2].to_string(),
            PLAYER_PALETTE[3].to_string(),
        ]
    );
}

#[test]
fn host_can_move_another_human_to_spectators() {
    let mut task = RoomTask::new(
        "host-spectator-target".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.host_id = Some(1);
    add_test_room_player(&mut task, 1, true);
    add_test_room_player(&mut task, 2, true);
    add_test_room_player(&mut task, 3, true);
    task.human_team_assignments.insert(2, 2);
    task.human_faction_assignments
        .insert(2, "kriegsia".to_string());

    task.on_set_spectator(3, 2, true);
    assert!(
        !task.players.get(&2).unwrap().spectator,
        "non-host targeted spectator move must be ignored"
    );

    task.on_set_spectator(1, 2, true);

    let target = task.players.get(&2).unwrap();
    assert!(target.spectator);
    assert!(!target.ready);
    assert_eq!(target.color, "#6f8fa8");
    assert!(!task.human_team_assignments.contains_key(&2));
    assert!(!task.human_faction_assignments.contains_key(&2));
}

#[test]
fn host_can_move_spectator_back_to_active_lobby_seat() {
    let mut task = RoomTask::new(
        "host-spectator-return".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.host_id = Some(1);
    add_test_room_player(&mut task, 1, true);
    add_test_room_spectator(&mut task, 2);
    task.human_team_assignments.insert(1, 1);

    task.on_set_spectator(1, 2, false);

    let target = task.players.get(&2).unwrap();
    assert!(!target.spectator);
    assert!(!target.ready);
    assert_ne!(target.color, "#6f8fa8");
    assert_eq!(task.human_team_assignments.get(&2), Some(&2));
    assert_eq!(
        task.human_faction_assignments.get(&2).map(String::as_str),
        Some("kriegsia")
    );
}

#[test]
fn default_ai_team_appends_after_occupied_teams_when_possible() {
    let mut task = RoomTask::new(
        "ai-default-team-append".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.host_id = Some(1);
    add_test_room_player(&mut task, 1, true);
    task.human_team_assignments.insert(1, 2);

    assert_eq!(task.next_default_team_for_new_seat(999_999), 3);
}

#[test]
fn selecting_two_player_map_trims_excess_active_seats() {
    let mut task = summary_task("two-player-map-cap");
    task.on_select_map(1, "Chokes".to_string());
    for id in 2..=3 {
        add_test_room_player(&mut task, id, true);
        task.assign_missing_team_for(id);
        task.assign_missing_faction_for(id);
    }
    add_test_room_spectator(&mut task, 4);
    task.on_add_ai(1, Some(4), None);
    assert_eq!(task.total_player_count(), 4);

    task.on_select_map(1, "1v1 No Terrain".to_string());

    assert_eq!(task.selected_map, "1v1 No Terrain");
    assert!(
        task.ai_players.is_empty(),
        "overflow AI seats are removed first"
    );
    assert_eq!(task.total_player_count(), 2);
    assert!(
        !task.players.get(&1).unwrap().spectator,
        "host stays active"
    );
    assert!(
        !task.players.get(&2).unwrap().spectator,
        "earlier active human stays active"
    );
    assert!(
        task.players.get(&3).unwrap().spectator,
        "latest excess human becomes spectator"
    );
    assert_eq!(task.players.get(&3).unwrap().color, "#6f8fa8");
    assert!(!task.human_team_assignments.contains_key(&3));
    assert!(!task.human_faction_assignments.contains_key(&3));

    let summary = task
        .lobby_summary()
        .expect("normal hosted lobby should be summarized");
    assert_eq!(summary.occupied_slots, 2);
    assert_eq!(summary.max_slots, 2);
    assert_eq!(summary.join_state, LobbyJoinState::FullSpectatorOnly);

    task.on_add_ai(1, Some(3), None);
    assert!(
        task.ai_players.is_empty(),
        "full 1v1 map rejects more AI seats"
    );

    task.on_set_spectator(1, 4, false);
    assert!(
        task.players.get(&4).unwrap().spectator,
        "full 1v1 map rejects spectator return to active play"
    );
}

#[test]
fn one_active_human_with_ai_start_skips_match_countdown() {
    let drain = DrainHandle::default();
    let host_id = 99;
    let mut task = RoomTask::new(
        "one-human-ai-countdown-skip".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    task.host_id = Some(host_id);
    let mut writer = add_test_room_player(&mut task, host_id, false);
    task.assign_missing_team_for(host_id);
    task.assign_missing_faction_for(host_id);
    task.on_add_ai(host_id, Some(2), None);
    task.on_ready(host_id, true);

    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_start_request(host_id);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
    assert_eq!(drain.active_matches(), 1);
}

#[test]
fn ai_only_start_skips_match_countdown() {
    let drain = DrainHandle::default();
    let host_id = 99;
    let mut task = RoomTask::new(
        "ai-only-countdown-skip".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    task.host_id = Some(host_id);
    let mut writer = add_test_room_player(&mut task, host_id, false);
    task.assign_missing_team_for(host_id);
    task.assign_missing_faction_for(host_id);
    task.on_add_ai(host_id, Some(2), None);
    task.on_add_ai(host_id, Some(3), None);
    task.on_set_spectator(host_id, host_id, true);

    assert_eq!(task.active_human_count(), 0);
    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_start_request(host_id);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
    assert_eq!(drain.active_matches(), 1);
}

#[test]
fn two_active_humans_with_ai_start_uses_match_countdown() {
    let drain = DrainHandle::default();
    let host_id = 99;
    let guest_id = 100;
    let mut task = RoomTask::new(
        "two-human-ai-countdown".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    task.host_id = Some(host_id);
    let mut writer = add_test_room_player(&mut task, host_id, false);
    add_test_room_player(&mut task, guest_id, false);
    task.assign_missing_team_for(host_id);
    task.assign_missing_team_for(guest_id);
    task.assign_missing_faction_for(host_id);
    task.assign_missing_faction_for(guest_id);
    task.on_add_ai(host_id, Some(3), None);
    task.on_ready(host_id, true);
    task.on_ready(guest_id, true);

    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_start_request(host_id);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn lobby_phase_ignores_gameplay_commands() {
    let mut task = RoomTask::new(
        "lobby-command-readonly-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 1, true);

    task.on_command(
        1,
        1,
        SimCommand::Stop {
            units: vec![1, 2, 3],
        },
    );

    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.pending_client_command_acks.is_empty());
    assert_eq!(
        task.players.get(&1).unwrap().last_received_client_seq,
        0,
        "lobby-phase commands must not consume client sequence state"
    );
    assert!(
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).any(|msg| {
            matches!(
                msg,
                ServerMessage::CommandReceipt {
                    client_seq: 1,
                    accepted: false,
                    reason: Some(reason),
                    ..
                } if reason == "notInGame"
            )
        })
    );
}
