use super::support::*;

#[test]
fn lab_timeline_records_mutations_and_issue_as_commands() {
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
    let issued_command = Command::Stop {
        units: vec![worker],
    };
    task.on_lab_request(
        100,
        31,
        LabClientOp::IssueCommandAs {
            player_id: LAB_PLAYER_ONE_ID,
            cmd: issued_command.clone(),
        },
    );
    assert!(lab_results(&mut collab_writer)[0].ok);

    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert_eq!(timeline.keyframe_ticks(), vec![0]);
    let entries = timeline.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].sequence, 0);
    assert_eq!(entries[0].tick, 0);
    assert_eq!(entries[0].request_id, 30);
    assert_eq!(entries[0].operator_id, 99);
    match &entries[0].kind {
        LabTimelineEntryKind::LabOperation {
            op_kind,
            op:
                LabOp::SetPlayerResources(LabSetPlayerResources {
                    player_id,
                    steel,
                    oil,
                }),
        } => {
            assert_eq!(op_kind, "setPlayerResources");
            assert_eq!(*player_id, LAB_PLAYER_ONE_ID);
            assert_eq!(*steel, 456);
            assert_eq!(*oil, 78);
        }
        other => panic!("unexpected first timeline entry: {other:?}"),
    }
    assert_eq!(entries[1].sequence, 1);
    assert_eq!(entries[1].tick, 0);
    assert_eq!(entries[1].request_id, 31);
    assert_eq!(entries[1].operator_id, 100);
    match &entries[1].kind {
        LabTimelineEntryKind::IssueCommandAs { player_id, command } => {
            assert_eq!(*player_id, LAB_PLAYER_ONE_ID);
            assert_eq!(command, &issued_command);
        }
        other => panic!("unexpected second timeline entry: {other:?}"),
    }
}

#[test]
fn lab_seek_rebuilds_world_and_resends_authoritative_reset_state() {
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

    let spawn_position = lab_tile_center(&task, 30, 30);
    task.on_lab_request(
        99,
        50,
        LabClientOp::SpawnEntity {
            owner: LAB_PLAYER_ONE_ID,
            kind: crate::protocol::kinds::RIFLEMAN.to_string(),
            x: spawn_position.0,
            y: spawn_position.1,
            completed: true,
        },
    );
    let spawn_result = lab_results(&mut operator_writer)
        .pop()
        .expect("spawn result");
    assert!(spawn_result.ok);
    let entity_id = spawn_result
        .outcome
        .as_ref()
        .and_then(|outcome| outcome.get("entityId"))
        .and_then(serde_json::Value::as_u64)
        .expect("spawned entity id") as u32;
    while collab_writer.reliable_rx.try_recv().is_ok() {}
    assert_eq!(lab_entity_position(&task, entity_id), spawn_position);

    task.on_tick(TokioInstant::now());
    assert_eq!(in_game_tick(&task), 1);
    let move_position = lab_tile_center(&task, 31, 30);
    task.on_lab_request(
        99,
        51,
        LabClientOp::MoveEntity {
            entity_id,
            x: move_position.0,
            y: move_position.1,
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);
    while collab_writer.reliable_rx.try_recv().is_ok() {}
    assert_eq!(lab_entity_position(&task, entity_id), move_position);

    task.on_seek_room_time(100, 1);

    assert_eq!(in_game_tick(&task), 0);
    assert_eq!(lab_entity_position(&task, entity_id), spawn_position);
    for writer in [&mut operator_writer, &mut collab_writer] {
        let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
        assert!(messages.iter().any(|msg| {
            matches!(
                msg,
                ServerMessage::Start(payload)
                    if payload.lab.as_ref().is_some_and(|lab| lab.role == LabStartRole::Operator)
                        && payload.capabilities.room_time.seek_absolute
                        && payload.capabilities.room_time.timeline
            )
        }));
        assert!(messages.iter().any(|msg| {
            matches!(
                msg,
                ServerMessage::RoomTimeState(state)
                    if state.current_tick == 0
                        && state.duration_ticks == 1
                        && state.keyframe_ticks.as_slice() == &[0]
                        && state.controller_id == Some(100)
            )
        }));
        assert!(messages.iter().any(|msg| {
            matches!(
                msg,
                ServerMessage::LabState(state)
                    if state.role == LabStartRole::Operator
                        && state.vision == LabVisionMode::FullWorld
            )
        }));
        assert!(writer.snapshots.take().is_some());
    }
}

#[test]
fn lab_seek_replays_issue_as_commands_through_rebuild() {
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
    while operator_writer.reliable_rx.try_recv().is_ok() {}

    let worker = lab_worker_id(&task);
    let start_position = lab_entity_position(&task, worker);
    task.on_lab_request(
        99,
        60,
        LabClientOp::IssueCommandAs {
            player_id: LAB_PLAYER_ONE_ID,
            cmd: Command::Move {
                units: vec![worker],
                x: start_position.0 + 128.0,
                y: start_position.1,
                queued: false,
            },
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);

    for _ in 0..12 {
        task.on_tick(TokioInstant::now());
    }
    let moved_position = lab_entity_position(&task, worker);
    assert_ne!(moved_position, start_position);

    task.on_seek_room_time_to(99, 12);

    assert_eq!(in_game_tick(&task), 12);
    assert_eq!(lab_entity_position(&task, worker), moved_position);
    let messages: Vec<_> =
        std::iter::from_fn(|| operator_writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::RoomTimeState(state)
                if state.current_tick == 12 && state.controller_id == Some(99)
        )
    }));
    assert!(operator_writer.snapshots.take().is_some());
}

#[test]
fn lab_timeline_truncates_future_after_past_seek_and_new_operation() {
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
    while operator_writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        70,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 100,
            oil: 10,
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);
    task.on_tick(TokioInstant::now());
    task.on_lab_request(
        99,
        71,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 200,
            oil: 20,
        },
    );
    assert!(lab_results(&mut operator_writer)[0].ok);
    assert_eq!(lab_player_resources(&task, LAB_PLAYER_ONE_ID), (200, 20));
    assert_eq!(
        task.lab_timeline
            .as_ref()
            .expect("lab timeline")
            .entries()
            .len(),
        2
    );

    task.on_seek_room_time_to(99, 0);
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    assert_eq!(lab_player_resources(&task, LAB_PLAYER_ONE_ID), (100, 10));

    task.on_lab_request(
        99,
        72,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 300,
            oil: 30,
        },
    );

    assert_eq!(lab_player_resources(&task, LAB_PLAYER_ONE_ID), (300, 30));
    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert_eq!(timeline.entries().len(), 2);
    assert!(timeline.entries().iter().all(|entry| entry.tick == 0));
    assert_eq!(timeline.keyframe_ticks(), vec![0]);
    let messages: Vec<_> =
        std::iter::from_fn(|| operator_writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::LabResult(result)
                if result.ok && result.request_id == 72 && result.op == "setPlayerResources"
        )
    }));
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::RoomTimeState(state)
                if state.current_tick == 0
                    && state.duration_ticks == 0
                    && state.keyframe_ticks.as_slice() == &[0]
        )
    }));
}

#[test]
fn lab_seek_rejects_read_only_and_rapid_repeat_requests() {
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
    task.lab_session
        .as_mut()
        .expect("lab session")
        .viewer_roles
        .insert(100, LabStartRole::ReadOnly);
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    while viewer_writer.reliable_rx.try_recv().is_ok() {}

    task.on_seek_room_time_to(100, 0);
    assert!(matches!(
        viewer_writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Error { msg } if msg.contains("Only lab operators")
    ));

    task.on_seek_room_time_to(99, 0);
    while operator_writer.reliable_rx.try_recv().is_ok() {}
    task.on_seek_room_time_to(99, 0);
    assert!(matches!(
        operator_writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::Error { msg } if msg.contains("wait before seeking again")
    ));
}

#[test]
fn lab_scenario_export_and_import_round_trip_through_room_ops() {
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
    while writer.reliable_rx.try_recv().is_ok() {}
    while collab_writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        20,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 777,
            oil: 66,
        },
    );
    assert!(lab_results(&mut writer)[0].ok);
    while collab_writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        21,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 2 },
        },
    );
    assert!(lab_results(&mut writer)[0].ok);
    while collab_writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        22,
        LabClientOp::ExportScenario {
            name: Some("saved setup".to_string()),
        },
    );
    let export_result = lab_results(&mut writer).pop().expect("export result");
    assert!(export_result.ok);
    let scenario: crate::protocol::LabScenarioV1 = serde_json::from_value(
        export_result
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("scenario"))
            .cloned()
            .expect("scenario outcome"),
    )
    .expect("scenario JSON");
    assert_eq!(scenario.kind, "labScenario");
    assert_eq!(scenario.name, "saved setup");
    assert_eq!(
        scenario.metadata.lab.vision,
        LabVisionMode::Team { team_id: 2 }
    );
    assert!(scenario.players.iter().any(|player| {
        player.id == LAB_PLAYER_ONE_ID
            && player.resources.steel == 777
            && player.resources.oil == 66
    }));

    task.on_lab_request(
        100,
        25,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 1 },
        },
    );
    assert!(lab_results(&mut collab_writer)[0].ok);
    while writer.reliable_rx.try_recv().is_ok() {}

    task.on_lab_request(
        99,
        23,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 1,
            oil: 1,
        },
    );
    assert!(lab_results(&mut writer)[0].ok);

    task.on_lab_request(99, 24, LabClientOp::ImportScenario { scenario });
    let import_result = lab_results(&mut writer).pop().expect("import result");
    assert!(import_result.ok);
    assert_eq!(import_result.op, "importScenario");
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should still be live after import");
    };
    let snapshot = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
    assert!(snapshot.player_resources.iter().any(|player| {
        player.id == LAB_PLAYER_ONE_ID && player.steel == 777 && player.oil == 66
    }));
    let session = task.lab_session.as_ref().unwrap();
    assert_eq!(session.vision_for(99), LabVisionMode::Team { team_id: 2 });
    assert_eq!(session.vision_for(100), LabVisionMode::Team { team_id: 1 });

    let (late_tx, mut late_writer) = ConnectionSink::new();
    let (late_ack, _late_ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        101,
        "Late Collaborator".to_string(),
        true,
        false,
        late_tx,
        late_ack,
    );
    let late_start = start_payloads(&mut late_writer)
        .pop()
        .expect("late lab start");
    assert_eq!(
        late_start.lab.as_ref().expect("lab metadata").vision,
        LabVisionMode::Team { team_id: 2 }
    );
}

#[test]
fn lab_timeline_resets_on_scenario_import() {
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
        40,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 777,
            oil: 66,
        },
    );
    assert!(lab_results(&mut writer)[0].ok);
    assert_eq!(
        task.lab_timeline
            .as_ref()
            .expect("lab timeline")
            .entries()
            .len(),
        1
    );

    task.on_lab_request(
        99,
        41,
        LabClientOp::ExportScenario {
            name: Some("baseline".to_string()),
        },
    );
    let export_result = lab_results(&mut writer).pop().expect("export result");
    let scenario: crate::protocol::LabScenarioV1 = serde_json::from_value(
        export_result
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("scenario"))
            .cloned()
            .expect("scenario outcome"),
    )
    .expect("scenario JSON");

    task.on_lab_request(
        99,
        42,
        LabClientOp::SetPlayerResources {
            player_id: LAB_PLAYER_ONE_ID,
            steel: 1,
            oil: 1,
        },
    );
    assert!(lab_results(&mut writer)[0].ok);
    assert_eq!(
        task.lab_timeline
            .as_ref()
            .expect("lab timeline")
            .entries()
            .len(),
        2
    );

    task.on_lab_request(99, 43, LabClientOp::ImportScenario { scenario });
    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::LabResult(result)
                if result.ok && result.request_id == 43 && result.op == "importScenario"
        )
    }));
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::LabState(state) if state.dirty && state.operation_count == 3
        )
    }));
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::RoomTimeState(state)
                if state.current_tick == 0
                    && state.duration_ticks == 0
                    && state.keyframe_ticks.as_slice() == &[0]
        )
    }));
    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert!(timeline.entries().is_empty());
    assert_eq!(timeline.keyframe_ticks(), vec![0]);
    assert_eq!(
        task.lab_session
            .as_ref()
            .expect("lab session")
            .operation_log
            .len(),
        3
    );
}

#[test]
fn lab_issue_as_accepts_single_owner_and_rejects_mixed_owner_commands() {
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
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should be running");
    };
    let snapshot = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
    let unit_one = snapshot
        .entities
        .iter()
        .find(|entity| {
            entity.owner == LAB_PLAYER_ONE_ID && entity.kind == crate::protocol::kinds::WORKER
        })
        .unwrap()
        .id;
    let unit_two = snapshot
        .entities
        .iter()
        .find(|entity| {
            entity.owner == LAB_PLAYER_TWO_ID && entity.kind == crate::protocol::kinds::WORKER
        })
        .unwrap()
        .id;

    task.on_lab_request(
        99,
        10,
        LabClientOp::IssueCommandAs {
            player_id: LAB_PLAYER_ONE_ID,
            cmd: Command::Stop {
                units: vec![unit_one],
            },
        },
    );
    task.on_lab_request(
        99,
        11,
        LabClientOp::IssueCommandAs {
            player_id: LAB_PLAYER_ONE_ID,
            cmd: Command::Stop {
                units: vec![unit_one, unit_two],
            },
        },
    );

    let results = lab_results(&mut writer);
    assert_eq!(results.len(), 2);
    assert!(results[0].ok);
    assert!(!results[1].ok);
    task.on_tick(TokioInstant::now());
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should remain running");
    };
    assert_eq!(game.command_log().len(), 1);
    assert_eq!(game.command_log()[0].player_id, LAB_PLAYER_ONE_ID);
}

#[test]
fn lab_team_vision_uses_server_projection() {
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
        12,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 2 },
        },
    );
    assert!(lab_results(&mut writer)[0].ok);
    while writer.reliable_rx.try_recv().is_ok() {}
    task.on_tick(TokioInstant::now());

    let snapshot = writer.snapshots.take().expect("lab team snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should remain running");
    };
    let mut expected = game.snapshot_for_spectator(&[LAB_PLAYER_TWO_ID]);
    compact_snapshot_for_wire(&mut expected);
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    assert_eq!(snapshot.entities.len(), expected.entities.len());
}

#[test]
fn lab_vision_state_and_snapshots_are_per_operator() {
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
        41,
        LabClientOp::SetVision {
            vision: LabVisionMode::Team { team_id: 2 },
        },
    );

    let operator_messages: Vec<_> =
        std::iter::from_fn(|| operator_writer.reliable_rx.try_recv().ok()).collect();
    let collab_messages: Vec<_> =
        std::iter::from_fn(|| collab_writer.reliable_rx.try_recv().ok()).collect();
    assert!(operator_messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::LabResult(result)
                if result.ok && result.request_id == 41 && result.op == "setVision"
        )
    }));
    let operator_state = operator_messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::LabState(state) => Some(state),
            _ => None,
        })
        .expect("operator lab state");
    let collab_state = collab_messages
        .iter()
        .find_map(|msg| match msg {
            ServerMessage::LabState(state) => Some(state),
            _ => None,
        })
        .expect("collaborator lab state");
    assert_eq!(operator_state.vision, LabVisionMode::Team { team_id: 2 });
    assert_eq!(collab_state.vision, LabVisionMode::FullWorld);
    assert_eq!(operator_state.operation_count, 1);
    assert_eq!(collab_state.operation_count, 1);

    task.on_tick(TokioInstant::now());

    let operator_snapshot = operator_writer
        .snapshots
        .take()
        .expect("operator team snapshot");
    let collab_snapshot = collab_writer
        .snapshots
        .take()
        .expect("collaborator full-world snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should remain running");
    };
    let mut expected_operator = game.snapshot_for_spectator(&[LAB_PLAYER_TWO_ID]);
    let mut expected_collab = game.snapshot_full_for(LAB_PLAYER_ONE_ID);
    compact_snapshot_for_wire(&mut expected_operator);
    compact_snapshot_for_wire(&mut expected_collab);
    assert_eq!(
        operator_snapshot.visible_tiles,
        expected_operator.visible_tiles
    );
    assert_eq!(
        operator_snapshot.entities.len(),
        expected_operator.entities.len()
    );
    assert_eq!(collab_snapshot.visible_tiles, expected_collab.visible_tiles);
    assert_eq!(
        collab_snapshot.entities.len(),
        expected_collab.entities.len()
    );
}
