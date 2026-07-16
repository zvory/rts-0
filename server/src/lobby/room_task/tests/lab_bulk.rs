use super::support::*;

#[test]
fn lab_plural_mutation_is_one_logged_operation_with_structured_failure_details() {
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Chokes".to_string(),
        RoomMode::Lab(lab_config()),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    while writer.reliable_rx.try_recv().is_ok() {}
    let _ = writer.snapshots.take();

    let first = lab_tile_center(&task, 30, 30);
    let second = lab_tile_center(&task, 34, 30);
    let tick_before_spawn = match &task.phase {
        Phase::InGame(game) => game.tick_count(),
        _ => panic!("lab game should be running"),
    };
    task.on_lab_request(
        99,
        70,
        LabClientOp::SpawnEntities {
            spawns: vec![
                crate::protocol::LabSpawnEntitySpec {
                    owner: LAB_PLAYER_ONE_ID,
                    kind: crate::protocol::kinds::RIFLEMAN.to_string(),
                    x: first.0,
                    y: first.1,
                    completed: true,
                },
                crate::protocol::LabSpawnEntitySpec {
                    owner: LAB_PLAYER_TWO_ID,
                    kind: crate::protocol::kinds::RIFLEMAN.to_string(),
                    x: second.0,
                    y: second.1,
                    completed: true,
                },
            ],
        },
    );
    let result = lab_results(&mut writer).pop().expect("plural result");
    assert!(result.ok);
    assert_eq!(result.op, "spawnEntities");
    assert_eq!(
        result
            .outcome
            .as_ref()
            .and_then(|outcome| outcome.get("items"))
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let spawned_ids: Vec<u32> = result
        .outcome
        .as_ref()
        .and_then(|outcome| outcome.get("items"))
        .and_then(serde_json::Value::as_array)
        .expect("plural spawn outcomes")
        .iter()
        .filter_map(|item| item.get("outcome"))
        .filter_map(|outcome| outcome.get("entityId"))
        .filter_map(serde_json::Value::as_u64)
        .filter_map(|id| u32::try_from(id).ok())
        .collect();
    assert_eq!(spawned_ids.len(), 2);
    assert_eq!(
        match &task.phase {
            Phase::InGame(game) => game.tick_count(),
            _ => panic!("lab game should be running"),
        },
        tick_before_spawn,
        "paused setup mutation must not advance live combat"
    );
    let snapshot = writer
        .snapshots
        .take()
        .expect("accepted setup mutation snapshot");
    assert_eq!(snapshot.tick, tick_before_spawn);
    assert!(spawned_ids
        .iter()
        .all(|id| snapshot.entities.iter().any(|entity| entity.id == *id)));
    assert_eq!(task.lab_session.as_ref().unwrap().operation_log.len(), 1);
    assert!(matches!(
        task.lab_timeline
            .as_ref()
            .and_then(|timeline| timeline.replay_entries().first())
            .map(|entry| &entry.op),
        Some(crate::protocol::LabReplayOperation::SpawnEntities { spawns }) if spawns.len() == 2
    ));

    let entity_count = match &task.phase {
        Phase::InGame(game) => game.snapshot_for(LAB_PLAYER_ONE_ID).entities.len(),
        _ => panic!("lab game should be running"),
    };
    task.on_lab_request(
        99,
        71,
        LabClientOp::SpawnEntities {
            spawns: vec![crate::protocol::LabSpawnEntitySpec {
                owner: LAB_PLAYER_ONE_ID,
                kind: crate::protocol::kinds::RIFLEMAN.to_string(),
                x: first.0,
                y: first.1,
                completed: true,
            }],
        },
    );
    let rejected = lab_results(&mut writer)
        .pop()
        .expect("rejected plural result");
    assert!(!rejected.ok);
    assert_eq!(rejected.failed_index, Some(0));
    assert!(rejected.details.as_ref().is_some_and(|details| {
        details
            .get("blockers")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|rows| !rows.is_empty())
            && details
                .get("suggestions")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|rows| !rows.is_empty())
    }));
    assert_eq!(
        match &task.phase {
            Phase::InGame(game) => game.snapshot_for(LAB_PLAYER_ONE_ID).entities.len(),
            _ => panic!("lab game should be running"),
        },
        entity_count
    );
    assert_eq!(task.lab_session.as_ref().unwrap().operation_log.len(), 1);
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 1);
}
