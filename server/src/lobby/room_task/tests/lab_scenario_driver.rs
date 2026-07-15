use super::support::*;
use crate::lobby::lab_scenario_driver::{LabScenarioAction, LabScenarioDriver};
use crate::lobby::room_task::types::LabSeekTarget;
use rts_sim::game::entity::EntityKind;
use rts_sim::game::lab::{LabCommandOptions, LabOp, LabSpawnEntity};

#[test]
fn hellhole_scripted_shuttles_are_recorded_once_and_replayable() {
    let mut config = lab_config();
    config.scenario = Some("supply-300-hellhole".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=supply-300-hellhole".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert!(task.lab_driver.is_some());
    task.on_seek_lab_room_time(99, LabSeekTarget::Absolute(0));
    task.apply_lab_scenario_actions();
    task.apply_lab_scenario_actions();

    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert_eq!(timeline.replay_entry_count(), 2);
    let scripted_players: Vec<_> = timeline
        .replay_entries()
        .iter()
        .map(|entry| {
            assert_eq!(entry.tick, 0);
            match &entry.op {
                crate::protocol::LabReplayOperation::IssueCommandAs {
                    player_id,
                    cmd: Command::Move { units, .. },
                    ignore_command_limits,
                } => {
                    assert_eq!(units.len(), 43);
                    assert!(*ignore_command_limits);
                    *player_id
                }
                other => panic!("unexpected scripted replay operation: {other:?}"),
            }
        })
        .collect();
    assert_eq!(scripted_players, vec![3, 4]);

    let recorded_entries = timeline.replay_entries().to_vec();
    task.lab_driver
        .as_mut()
        .expect("hellhole driver")
        .sync_to_tick(0, &recorded_entries);
    task.apply_lab_scenario_actions();
    assert_eq!(
        task.lab_timeline.as_ref().unwrap().replay_entry_count(),
        2,
        "a seek rebuilt through scripted entries must not enqueue them twice"
    );

    let artifact = task
        .export_lab_replay_artifact(99, Some("Hellhole scripted replay"))
        .expect("scripted commands should produce a valid replay artifact");
    task.load_lab_replay_artifact(99, artifact)
        .expect("scripted replay artifact should rebuild");
    assert!(task.lab_driver.is_none());
}

#[test]
fn hellhole_respawn_and_partial_commands_seek_and_export_together() {
    let mut config = lab_config();
    config.scenario = Some("supply-300-hellhole".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=supply-300-hellhole".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));

    task.apply_lab_scenario_actions();
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 2);

    let victim = lab_snapshot(&task)
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == "tank")
        .map(|entity| entity.id)
        .unwrap();
    assert!(
        task.apply_and_record_lab_operation(
            99,
            6001,
            "deleteEntity".to_string(),
            LabOp::DeleteEntity { entity_id: victim },
        )
        .ok
    );
    assert_eq!(lab_snapshot(&task).entities.len(), 379);

    task.on_tick(tokio::time::Instant::now());
    task.apply_lab_scenario_actions();
    let entries = task.lab_timeline.as_ref().unwrap().replay_entries();
    assert_eq!(entries.len(), 4);
    assert_eq!(
        entries
            .iter()
            .filter(|entry| matches!(
                &entry.op,
                crate::protocol::LabReplayOperation::IssueCommandAs {
                    cmd: Command::Move { units, .. },
                    ..
                } if units.len() == 43
            ))
            .count(),
        2
    );
    assert!(entries.iter().any(|entry| matches!(
        &entry.op,
        crate::protocol::LabReplayOperation::SpawnEntities { spawns } if !spawns.is_empty()
    )));
    assert_eq!(lab_snapshot(&task).entities.len(), 380);

    task.on_seek_lab_room_time(99, LabSeekTarget::Absolute(1));
    assert_eq!(lab_snapshot(&task).entities.len(), 380);
    task.apply_lab_scenario_actions();
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 4);

    let artifact = task
        .export_lab_replay_artifact(99, Some("Hellhole churn replay"))
        .unwrap();
    task.load_lab_replay_artifact(99, artifact).unwrap();
    assert_eq!(lab_snapshot(&task).entities.len(), 380);
    assert!(task.lab_driver.is_none());
}

#[test]
fn scripted_spawn_is_recorded_once_seekable_and_portable() {
    let mut config = lab_config();
    config.scenario = Some("supply-300-hellhole".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=supply-300-hellhole".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));

    let initial_entity_count = lab_snapshot(&task).entities.len();
    let spawn = LabSpawnEntity {
        owner: 1,
        kind: EntityKind::Rifleman,
        x: 10.0 * 32.0,
        y: 10.0 * 32.0,
        completed: true,
    };
    task.lab_driver = Some(LabScenarioDriver::scripted_for_test(
        0,
        LabScenarioAction::LabOperation {
            request_id: 7001,
            op: LabOp::SpawnEntities(vec![spawn]),
        },
    ));

    task.apply_lab_scenario_actions();
    task.apply_lab_scenario_actions();
    assert!(lab_snapshot(&task).entities.iter().any(|entity| {
        entity.owner == 1
            && entity.kind == "rifleman"
            && (entity.x - 10.0 * 32.0).abs() < 0.1
            && (entity.y - 10.0 * 32.0).abs() < 0.1
    }));
    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert_eq!(timeline.replay_entry_count(), 1);
    assert!(matches!(
        &timeline.replay_entries()[0],
        crate::protocol::LabReplayOperationEntry {
            tick: 0,
            request_id: 7001,
            op: crate::protocol::LabReplayOperation::SpawnEntities { spawns },
            ..
        } if spawns.len() == 1
    ));

    task.on_seek_lab_room_time(99, LabSeekTarget::Absolute(0));
    assert!(lab_snapshot(&task).entities.iter().any(|entity| {
        entity.owner == 1
            && entity.kind == "rifleman"
            && (entity.x - 10.0 * 32.0).abs() < 0.1
            && (entity.y - 10.0 * 32.0).abs() < 0.1
    }));
    task.apply_lab_scenario_actions();
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 1);
    assert_eq!(lab_snapshot(&task).entities.len(), initial_entity_count + 1);

    let artifact = task
        .export_lab_replay_artifact(99, Some("Scripted spawn replay"))
        .expect("scripted spawn should export");
    let json = serde_json::to_vec(&artifact).expect("artifact JSON");
    let imported = crate::protocol::lab_replay_artifact_from_slice(&json)
        .expect("artifact should parse after export");
    task.load_lab_replay_artifact(99, imported)
        .expect("scripted spawn replay should rebuild");
    assert_eq!(lab_snapshot(&task).entities.len(), initial_entity_count + 1);
    assert!(task.lab_driver.is_none());
}

#[test]
fn scripted_spawn_replaces_retained_future_entry_after_earlier_seek() {
    let mut config = lab_config();
    config.scenario = Some("supply-300-hellhole".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=supply-300-hellhole".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));

    let initial_entity_count = lab_snapshot(&task).entities.len();
    task.lab_driver = Some(LabScenarioDriver::scripted_for_test(
        1,
        LabScenarioAction::LabOperation {
            request_id: 7001,
            op: LabOp::SpawnEntities(vec![LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: 10.0 * 32.0,
                y: 10.0 * 32.0,
                completed: true,
            }]),
        },
    ));

    task.on_tick(tokio::time::Instant::now());
    task.on_tick(tokio::time::Instant::now());
    assert!(lab_snapshot(&task).entities.iter().any(|entity| {
        entity.owner == 1
            && entity.kind == "rifleman"
            && (entity.x - 10.0 * 32.0).abs() < 0.1
            && (entity.y - 10.0 * 32.0).abs() < 0.1
    }));
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 1);

    task.on_seek_lab_room_time(99, LabSeekTarget::Absolute(0));
    assert_eq!(lab_snapshot(&task).entities.len(), initial_entity_count);
    task.on_tick(tokio::time::Instant::now());
    task.on_tick(tokio::time::Instant::now());

    assert!(lab_snapshot(&task).entities.iter().any(|entity| {
        entity.owner == 1
            && entity.kind == "rifleman"
            && (entity.x - 10.0 * 32.0).abs() < 0.1
            && (entity.y - 10.0 * 32.0).abs() < 0.1
    }));
    let timeline = task.lab_timeline.as_ref().unwrap();
    assert_eq!(timeline.replay_entry_count(), 1);
    assert_eq!(timeline.replay_entries()[0].tick, 1);
    assert_eq!(timeline.replay_entries()[0].request_id, 7001);
}

#[test]
fn scripted_action_batch_rebases_before_crossing_timeline_cap() {
    let mut config = lab_config();
    config.scenario = Some("supply-300-hellhole".to_string());
    let mut task = RoomTask::new(
        "__lab__:sandbox:map=Default:scenario=supply-300-hellhole".to_string(),
        RoomMode::Lab(config),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Operator".to_string(), true, false, msg_tx, ack);
    assert_eq!(ack_rx.try_recv(), Ok(true));

    let timeline = task.lab_timeline.as_mut().expect("lab timeline");
    for request_id in 0..(crate::lobby::lab_timeline::LabTimeline::MAX_ENTRIES - 1) {
        timeline.record_issue_command_as(
            0,
            u32::try_from(request_id).unwrap(),
            99,
            1,
            crate::protocol::Command::Move {
                units: vec![1],
                x: 0.0,
                y: 0.0,
                queued: false,
            },
            LabCommandOptions::default(),
        );
    }

    let initial_entity_count = lab_snapshot(&task).entities.len();
    let spawn_action = |request_id, x| LabScenarioAction::LabOperation {
        request_id,
        op: LabOp::SpawnEntities(vec![LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x,
            y: 10.0 * 32.0,
            completed: true,
        }]),
    };
    task.lab_driver = Some(LabScenarioDriver::scripted_actions_for_test(
        0,
        vec![
            spawn_action(7001, 10.0 * 32.0),
            spawn_action(7002, 12.0 * 32.0),
        ],
    ));

    task.apply_lab_scenario_actions();
    assert_eq!(lab_snapshot(&task).entities.len(), initial_entity_count + 2);
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 2);

    task.on_seek_lab_room_time(99, LabSeekTarget::Absolute(0));
    task.apply_lab_scenario_actions();
    assert_eq!(lab_snapshot(&task).entities.len(), initial_entity_count + 2);
    assert_eq!(task.lab_timeline.as_ref().unwrap().replay_entry_count(), 2);
}
