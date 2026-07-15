use super::support::*;
use crate::lobby::room_task::types::LabSeekTarget;

#[test]
fn hellhole_scripted_combat_orders_are_recorded_once_and_replayable() {
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
    task.enqueue_lab_scenario_commands();
    task.enqueue_lab_scenario_commands();

    let timeline = task.lab_timeline.as_ref().expect("lab timeline");
    assert_eq!(timeline.replay_entry_count(), 2);
    let scripted_players: Vec<_> = timeline
        .replay_entries()
        .iter()
        .map(|entry| {
            assert_eq!(entry.tick, 1);
            match &entry.op {
                crate::protocol::LabReplayOperation::IssueCommandAs {
                    player_id,
                    cmd: Command::AttackMove { units, .. },
                    ignore_command_limits,
                } => {
                    assert_eq!(units.len(), 111);
                    assert!(*ignore_command_limits);
                    *player_id
                }
                other => panic!("unexpected scripted replay operation: {other:?}"),
            }
        })
        .collect();
    assert_eq!(scripted_players, vec![1, 2]);

    let recorded_entries = timeline.replay_entries().to_vec();
    task.lab_driver
        .as_mut()
        .expect("hellhole driver")
        .sync_to_tick(1, &recorded_entries);
    task.enqueue_lab_scenario_commands();
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
