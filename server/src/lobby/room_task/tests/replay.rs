use super::super::types::ReplayTickContext;
use super::support::*;
use rts_rules::balance::TILE_SIZE;
use rts_sim::game::lab::{LabOp, LabOpOutcome, LabSpawnEntity};
use rts_sim::game::{Game, PlayerInit};
use std::time::Instant as StdInstant;

fn tile_center(tile_x: u32, tile_y: u32) -> (f32, f32) {
    let tile_size = TILE_SIZE as f32;
    (
        (tile_x as f32 + 0.5) * tile_size,
        (tile_y as f32 + 0.5) * tile_size,
    )
}

fn spawn_replay_test_entity(
    game: &mut Game,
    owner: u32,
    kind: EntityKind,
    position: (f32, f32),
) -> u32 {
    match game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner,
            kind,
            x: position.0,
            y: position.1,
            completed: true,
        }))
        .expect("test entity spawn should succeed")
    {
        LabOpOutcome::Spawned { entity_id } => entity_id,
        outcome => panic!("unexpected spawn outcome: {outcome:?}"),
    }
}

fn delete_replay_test_entity(game: &mut Game, entity_id: u32) {
    match game
        .apply_lab_op(LabOp::DeleteEntity { entity_id })
        .expect("test entity delete should succeed")
    {
        LabOpOutcome::Deleted { .. } => {}
        outcome => panic!("unexpected delete outcome: {outcome:?}"),
    }
}

fn spawn_hidden_replay_depot_and_scout_position(
    game: &mut Game,
    viewers: [u32; 2],
) -> (u32, (f32, f32)) {
    let current_view = game.snapshot_for_spectator(&viewers);
    let map_size = (current_view.visible_tiles.len() as f64).sqrt() as u32;
    for tile_y in 0..map_size {
        for tile_x in 0..map_size {
            let depot_pos = tile_center(tile_x, tile_y);
            let tile_index = (tile_y * map_size + tile_x) as usize;
            if current_view.visible_tiles.get(tile_index).copied() != Some(0) {
                continue;
            }
            let Ok(LabOpOutcome::Spawned { entity_id: depot }) =
                game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                    owner: 3,
                    kind: EntityKind::Depot,
                    x: depot_pos.0,
                    y: depot_pos.1,
                    completed: true,
                }))
            else {
                continue;
            };

            for offset_y in -2_i32..=2 {
                for offset_x in -2_i32..=2 {
                    if offset_x == 0 && offset_y == 0 {
                        continue;
                    }
                    let scout_x = tile_x as i32 + offset_x;
                    let scout_y = tile_y as i32 + offset_y;
                    if scout_x < 0
                        || scout_y < 0
                        || scout_x >= map_size as i32
                        || scout_y >= map_size as i32
                    {
                        continue;
                    }
                    let scout_pos = tile_center(scout_x as u32, scout_y as u32);
                    let Ok(LabOpOutcome::Spawned { entity_id: scout }) =
                        game.apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                            owner: viewers[0],
                            kind: EntityKind::Rifleman,
                            x: scout_pos.0,
                            y: scout_pos.1,
                            completed: true,
                        }))
                    else {
                        continue;
                    };
                    delete_replay_test_entity(game, scout);
                    return (depot, scout_pos);
                }
            }
            delete_replay_test_entity(game, depot);
        }
    }
    panic!("default replay map should contain a hidden depot position with a scout approach");
}

fn replay_game_with_split_building_memory(players: &[PlayerInit]) -> (Game, u32) {
    let mut game = replay_test_game(players, 0x5150_5505);
    let viewers = [players[0].id, players[1].id];
    let (depot, scout_pos) = spawn_hidden_replay_depot_and_scout_position(&mut game, viewers);

    let p1_scout = spawn_replay_test_entity(&mut game, viewers[0], EntityKind::Rifleman, scout_pos);
    game.tick();

    delete_replay_test_entity(&mut game, p1_scout);
    let p2_scout = spawn_replay_test_entity(&mut game, viewers[1], EntityKind::Rifleman, scout_pos);
    game.tick();

    delete_replay_test_entity(&mut game, p2_scout);
    game.tick();

    assert!(
        game.snapshot_for_spectator(&[viewers[0]])
            .remembered_buildings
            .iter()
            .any(|building| building.id == depot),
        "test setup should give P1 stale memory"
    );
    assert!(
        game.snapshot_for_spectator(&[viewers[1]])
            .remembered_buildings
            .iter()
            .any(|building| building.id == depot),
        "test setup should give P2 stale memory"
    );
    (game, depot)
}

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
        assert!(payload.capabilities.visibility.vision_selection);
        assert!(payload.capabilities.actions.branch_from_tick);
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
                && payload.capabilities.visibility.vision_selection
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
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::RoomTimeSeekStarted { .. })));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
}

#[test]
fn replay_seek_started_reaches_every_viewer_before_rebuild_results() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    let mut task = RoomTask::new(
        "replay-seek-started-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut controller = add_test_room_player(&mut task, 99, true);
    let mut viewer = add_test_room_player(&mut task, 100, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_seek_room_time_to(99, 1);

    for (label, writer) in [("controller", &mut controller), ("viewer", &mut viewer)] {
        let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(
            matches!(
                messages.first(),
                Some(ServerMessage::RoomTimeSeekStarted {
                    controller_id: 99,
                    from_tick: 3,
                    target_tick: 1,
                })
            ),
            "{label} should receive seek progress before rebuilt replay messages: {messages:?}"
        );
        assert!(messages
            .iter()
            .any(|msg| matches!(msg, ServerMessage::Start(_))));
        assert!(messages.iter().any(|msg| matches!(
            msg,
            ServerMessage::RoomTimeState(state) if state.current_tick == 1
        )));
    }
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
    assert!(join_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    let join_analysis = take_observer_analysis(&writer, "replay join");
    assert_eq!(join_analysis.tick, 3);
    assert_eq!(join_analysis.players.len(), 2);

    task.on_seek_room_time_to(99, 1);
    let seek_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Start(payload)
            if payload.capabilities.room_time.seek_relative
                && payload.capabilities.room_time.seek_absolute
                && payload.capabilities.visibility.vision_selection
    )));
    let seek_analysis = take_observer_analysis(&writer, "replay seek");
    assert_eq!(seek_analysis.tick, 1);
    assert_eq!(seek_analysis.players.len(), 2);
}

#[test]
fn rapid_vision_selection_changes_remain_per_viewer() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "vision-selection-stress-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let writer_a = add_test_room_spectator(&mut task, 100);
    let writer_b = add_test_room_spectator(&mut task, 101);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    for _ in 0..8 {
        task.on_set_vision_selection(
            100,
            VisionSelectionRequest::Player {
                player_id: players[0].id,
            },
        );
        task.on_set_vision_selection(
            101,
            VisionSelectionRequest::Player {
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
    assert_eq!(
        snapshot_a
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[0].id],
        "single-player replay vision should only expose that player's resources"
    );
    assert_eq!(
        snapshot_b
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[1].id],
        "single-player replay vision should only expose that player's resources"
    );
    assert_ne!(
        snapshot_a.visible_tiles, snapshot_b.visible_tiles,
        "test setup should exercise different fog perspectives"
    );
}

#[test]
fn omniscient_replay_view_receives_the_full_event_union() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let replay = ReplaySession::new(artifact).unwrap();
    let mut task = RoomTask::new(
        "omniscient-replay-events-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let writer = add_test_room_spectator(&mut task, 100);
    let player_one_event = Event::Notice {
        msg: "player one event".to_string(),
        severity: NoticeSeverity::Info,
        x: None,
        y: None,
    };
    let player_two_event = Event::Notice {
        msg: "player two event".to_string(),
        severity: NoticeSeverity::Warn,
        x: None,
        y: None,
    };
    let mut per_player_events = HashMap::new();
    per_player_events.insert(players[0].id, vec![player_one_event.clone()]);
    per_player_events.insert(players[1].id, vec![player_two_event.clone()]);
    let context = ReplayTickContext {
        scheduler_lag: Duration::ZERO,
        tick_budget: Duration::from_millis(config::TICK_MS),
        tick_start: StdInstant::now(),
        projection_policy: task.projection_policy_for_phase(SessionPhase::ReplayViewer),
    };

    task.fanout_replay_snapshots_to(&replay, [100], per_player_events, context, None);

    let snapshot = writer.snapshots.take().expect("omniscient replay snapshot");
    assert!(snapshot.events.contains(&player_one_event));
    assert!(snapshot.events.contains(&player_two_event));
}

#[test]
fn replay_vision_selection_sends_snapshot_without_waiting_for_tick() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let mut replay = ReplaySession::new(artifact).unwrap();
    replay.set_speed(100, 0.0);

    let mut task = RoomTask::new(
        "paused-vision-selection-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let writer = add_test_room_spectator(&mut task, 100);
    let all_player_ids = players.iter().map(|player| player.id).collect::<Vec<_>>();
    let all_pending = replay.game().snapshot_for_spectator(&all_player_ids);
    let player_one_expected = replay.game().snapshot_for_spectator(&[players[0].id]);
    assert!(
        all_pending.resource_deltas.len() > player_one_expected.resource_deltas.len(),
        "test setup should make all-player replay vision wider than one-player vision"
    );
    task.players
        .get(&100)
        .expect("test spectator")
        .msg_tx
        .try_send_snapshot(all_pending);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_set_vision_selection(
        100,
        VisionSelectionRequest::Player {
            player_id: players[0].id,
        },
    );

    let snapshot = writer
        .snapshots
        .take()
        .expect("vision selection should enqueue an immediate replay snapshot");
    assert_eq!(snapshot.tick, player_one_expected.tick);
    assert_eq!(snapshot.visible_tiles, player_one_expected.visible_tiles);
    assert_eq!(
        snapshot
            .resource_deltas
            .iter()
            .map(|delta| delta.id)
            .collect::<Vec<_>>(),
        player_one_expected
            .resource_deltas
            .iter()
            .map(|delta| delta.id)
            .collect::<Vec<_>>(),
        "vision switch should not merge stale wider-view resource deltas"
    );
    assert_eq!(
        snapshot
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[0].id],
        "single-player replay vision should immediately scope resource rows"
    );
}

#[test]
fn replay_seek_while_paused_sends_snapshot_without_waiting_for_unpause() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    replay.set_speed(100, 0.0);
    let mut task = RoomTask::new(
        "paused-replay-seek-snapshot-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_spectator(&mut task, 100);
    task.observer_views.insert(
        100,
        rts_sim::game::ObserverView::Players(vec![players[0].id]),
    );
    let all_player_ids = players.iter().map(|player| player.id).collect::<Vec<_>>();
    let stale_pending = replay.game().snapshot_for_spectator(&all_player_ids);
    task.players
        .get(&100)
        .expect("test spectator")
        .msg_tx
        .try_send_snapshot(stale_pending);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_seek_room_time_to(100, 1);

    let snapshot = writer
        .snapshots
        .take()
        .expect("paused replay seek should enqueue an immediate snapshot");
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("replay phase should remain active after seek");
    };
    let expected = session.game.snapshot_for_spectator(&[players[0].id]);
    assert_eq!(session.current_tick(), 1);
    assert_eq!(snapshot.tick, 1);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    assert_eq!(
        snapshot
            .resource_deltas
            .iter()
            .map(|delta| delta.id)
            .collect::<Vec<_>>(),
        expected
            .resource_deltas
            .iter()
            .map(|delta| delta.id)
            .collect::<Vec<_>>(),
        "paused seek snapshot should not merge stale wider-view resource deltas"
    );
    assert_eq!(
        snapshot
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[0].id],
        "paused seek should preserve the viewer's selected replay perspective"
    );
    let seek_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Start(payload) if payload.replay.is_some()
    )));
    assert!(seek_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::RoomTimeState(state) if state.current_tick == 1 && state.paused
    )));
}

#[test]
fn replay_vision_switch_replaces_memory_and_resource_scope() {
    let players = replay_test_players(3);
    let (_live, artifact) = replay_test_artifact(&players, 1);
    let mut replay = ReplaySession::new(artifact).unwrap();
    let (memory_game, depot) = replay_game_with_split_building_memory(&players);
    replay.game = Box::new(memory_game);
    replay.duration_ticks = replay.current_tick() + 2;

    let mut task = RoomTask::new(
        "replay-memory-vision-switch-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let writer = add_test_room_spectator(&mut task, 100);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    task.on_set_vision_selection(
        100,
        VisionSelectionRequest::Player {
            player_id: players[0].id,
        },
    );
    task.on_tick_replay_viewer(TokioInstant::now());
    let player_one_snapshot = writer.snapshots.take().expect("P1 replay snapshot");
    let player_one_memory = player_one_snapshot
        .remembered_buildings
        .iter()
        .find(|building| building.id == depot)
        .expect("P1 replay vision should include P1 memory");
    assert_eq!(
        player_one_snapshot
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[0].id],
        "P1 replay vision should only expose P1 resource rows"
    );

    task.on_set_vision_selection(
        100,
        VisionSelectionRequest::Player {
            player_id: players[1].id,
        },
    );
    task.on_tick_replay_viewer(TokioInstant::now());
    let player_two_snapshot = writer.snapshots.take().expect("P2 replay snapshot");
    let player_two_memory = player_two_snapshot
        .remembered_buildings
        .iter()
        .find(|building| building.id == depot)
        .expect("P2 replay vision should include P2 memory after switching");
    assert!(
        player_two_memory.observed_tick > player_one_memory.observed_tick,
        "switching replay vision should replace remembered-building memory with the selected player's store"
    );
    assert_eq!(
        player_two_snapshot
            .player_resources
            .iter()
            .map(|resources| resources.id)
            .collect::<Vec<_>>(),
        vec![players[1].id],
        "P2 replay vision should only expose P2 resource rows after switching"
    );
}

#[test]
fn persisted_replay_room_join_waits_in_spectator_lobby() {
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

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(task.players.get(&99).is_some_and(|p| p.spectator));
    assert_eq!(task.host_id, Some(99));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Lobby {
            room,
            kind: crate::protocol::LobbyKind::Replay,
            players,
            can_start: true,
            map,
            maps,
            ..
        } if room == "persisted-replay-test"
            && players.len() == 1
            && players[0].is_spectator
            && map == "Chokes"
            && maps.is_empty()
    ));
}

#[test]
fn persisted_replay_room_host_start_begins_replay_viewer() {
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
    assert!(matches!(task.phase, Phase::Lobby));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Lobby {
            kind: crate::protocol::LobbyKind::Replay,
            can_start: true,
            ..
        }
    ));

    task.on_start_request(99);

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
    let start_analysis = take_observer_analysis(&writer, "confirmed replay start");
    assert_eq!(start_analysis.tick, 0);
    assert_eq!(start_analysis.players.len(), players.len());

    task.on_tick_replay_viewer(TokioInstant::now());
    let snapshot = writer.snapshots.take().expect("replay viewer snapshot");
    let Phase::ReplayViewer(session) = &task.phase else {
        panic!("confirmed replay join should keep replay viewer active");
    };
    let expected = session
        .game
        .snapshot_for_observer(&ObserverView::Players(session.active_player_ids()));
    assert_eq!(snapshot.tick, expected.tick);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    let tick_analysis = take_observer_analysis(&writer, "confirmed replay tick");
    assert_eq!(tick_analysis.tick, expected.tick);
    assert_eq!(tick_analysis.players.len(), players.len());
}

#[test]
fn persisted_replay_lobby_ignores_active_seat_controls() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let mut task = RoomTask::new(
        "persisted-replay-controls-test".to_string(),
        RoomMode::Replay { artifact },
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), false, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    let _ = writer.reliable_rx.try_recv();

    task.on_ready(99, false);
    task.on_set_spectator(99, 99, false);
    task.on_set_team(99, 99, 2);
    task.on_set_faction(99, EKAT_FACTION_ID.to_string());
    task.on_add_ai(99, Some(2), None);
    task.on_select_map(99, "Chokes".to_string());

    let player = task.players.get(&99).expect("viewer should remain present");
    assert!(player.spectator);
    assert!(player.ready);
    assert!(task.ai_players.is_empty());
    assert!(!task.human_team_assignments.contains_key(&99));
    assert!(!task.human_faction_assignments.contains_key(&99));
    assert_eq!(task.selected_map, "1v1");
    assert!(matches!(task.phase, Phase::Lobby));
}

#[test]
fn persisted_replay_lobby_start_is_blocked_during_drain() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let drain = DrainHandle::default();
    drain.begin_draining(Duration::from_secs(295));
    let mut task = RoomTask::new(
        "persisted-replay-drain-test".to_string(),
        RoomMode::Replay { artifact },
        None,
        false,
        drain,
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));
    let join_messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(join_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::ShutdownWarning { .. })));
    assert!(join_messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Lobby {
            kind: crate::protocol::LobbyKind::Replay,
            can_start: false,
            ..
        }
    )));

    task.on_start_request(99);

    assert!(matches!(task.phase, Phase::Lobby));
    let start_messages: Vec<_> =
        std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(start_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Error { msg } if msg.contains("draining"))));
    assert!(!start_messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
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
    assert_eq!(
        task.observer_view_for(99),
        rts_sim::game::ObserverView::Players(session.active_player_ids())
    );
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
fn confirmed_late_replay_join_receives_current_ended_state_immediately() {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 3);
    let mut replay = ReplaySession::new(artifact).unwrap();
    let end_tick = replay.duration_ticks;
    replay.rebuild_to(end_tick).unwrap();
    let mut task = RoomTask::new(
        "ended-post-match-replay-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.host_id = Some(50);
    let _existing_writer = add_test_room_spectator(&mut task, 50);
    task.phase = Phase::ReplayViewer(Box::new(replay));
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Late Viewer".to_string(), true, true, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Start(payload) if payload.spectator && payload.replay.is_some()
    ));
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(state) if state.current_tick == end_tick && state.ended
    ));
    let snapshot = writer
        .snapshots
        .take()
        .expect("late replay join should receive the current snapshot without another tick");
    assert_eq!(snapshot.tick, end_tick);
    let analysis = take_observer_analysis(&writer, "late ended replay join");
    assert_eq!(analysis.tick, end_tick);
}

#[test]
fn replay_viewer_return_detaches_only_requesting_viewer() {
    let players = replay_test_players(2);
    let (game, replay_start, _artifact) = replay_test_artifact_with_start(&players, 1);
    let mut task = RoomTask::new(
        "post-match-lobby-test".to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let _writer_a = add_test_room_player(&mut task, players[0].id, true);
    let writer_b = add_test_room_player(&mut task, players[1].id, true);
    task.host_id = Some(players[0].id);
    task.match_player_count = 2;
    task.match_human_count = 2;
    task.replay_start = Some(replay_start);

    task.end_match(Some(players[0].id), game.scores(), Some(&game));
    assert!(matches!(task.phase, Phase::ReplayViewer(_)));
    let summary = task
        .lobby_summary()
        .expect("automatic post-match replay should remain joinable from the browser");
    assert_eq!(summary.kind, crate::protocol::LobbyKind::Replay);
    assert_eq!(summary.join_state, LobbyJoinState::InGame);
    assert_eq!(summary.spectator_count, 2);

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
    let (game, replay_start, _artifact) = replay_test_artifact_with_start(&players, 1);
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
    task.replay_start = Some(replay_start);

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
