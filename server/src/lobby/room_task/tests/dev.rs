use super::support::*;
use crate::protocol::Event;
use rts_sim::game::command::{CommandRejection, SimCommand};

#[test]
fn dev_scenario_start_payload_is_read_only_viewer_payload() {
    let mut task = RoomTask::new(
        "dev-start-payload-test".to_string(),
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
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();

    task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    let starts = start_payloads(&mut writer);
    assert_eq!(starts.len(), 1);
    let payload = &starts[0];
    assert_eq!(payload.player_id, 99);
    assert!(payload.spectator);
    assert!(payload.prediction_build_id.is_none());
    assert_eq!(payload.prediction_version, 0);
    assert!(payload.replay.is_none());
    assert!(payload.lab.is_none());
    assert!(payload.capabilities.room_time.available);
    assert!(payload.capabilities.room_time.set_speed);
    assert!(payload.capabilities.room_time.pause);
    assert!(payload.capabilities.room_time.step);
    assert!(!payload.capabilities.room_time.seek_relative);
    assert!(!payload.capabilities.room_time.seek_absolute);
    assert!(!payload.capabilities.room_time.timeline);
    assert!(!payload.capabilities.commands.gameplay);
    assert!(!payload.capabilities.match_controls.pause);
    assert!(payload.capabilities.visibility.vision_selection);
    assert!(!payload.capabilities.actions.branch_from_tick);
    assert_eq!(
        payload.diagnostics.movement_paths,
        MovementPathDiagnosticScope::All
    );
    assert!(!payload.diagnostics.observer_analysis);
}

#[test]
fn paused_dev_scenario_steps_one_tick_at_a_time() {
    let mut task = RoomTask::new(
        "dev-scenario-step-test".to_string(),
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
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(in_game_tick(&task), 0);
    while writer.reliable_rx.try_recv().is_ok() {}

    task.on_set_room_time_speed(99, 0.0);
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(state)
            if state.paused && state.speed == 0.0 && state.current_tick == 0
    ));
    task.on_tick(TokioInstant::now());
    assert_eq!(
        in_game_tick(&task),
        0,
        "scheduled ticks should not advance while paused"
    );

    task.on_step_room_time(99);
    assert_eq!(in_game_tick(&task), 1);
    let snapshot = writer.snapshots.take().expect("dev watch snapshot");
    let Phase::InGame(game) = &task.phase else {
        panic!("dev scenario should remain live");
    };
    let expected = game.snapshot_full_for(task.dev_view_player_id.unwrap());
    assert_eq!(snapshot.visible_tiles, expected.visible_tiles);
    assert_eq!(snapshot.net_status.prediction_version, 0);
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(state)
            if state.paused && state.speed == 0.0 && state.current_tick == 1
    ));
    task.on_step_room_time(99);
    assert_eq!(in_game_tick(&task), 2);
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(state)
            if state.paused && state.speed == 0.0 && state.current_tick == 2
    ));

    task.on_set_room_time_speed(99, 1.0);
    assert!(matches!(
        writer.reliable_rx.try_recv().unwrap(),
        ServerMessage::RoomTimeState(state)
            if !state.paused && state.speed == 1.0 && state.current_tick == 2
    ));
    task.on_tick(TokioInstant::now());
    assert_eq!(
        in_game_tick(&task),
        3,
        "scheduled ticks should resume after selecting a non-zero speed"
    );
}

#[test]
fn dev_full_world_snapshot_receives_event_from_non_view_player_bucket() {
    let mut task = RoomTask::new(
        "dev-full-world-event-union-test".to_string(),
        RoomMode::DevScenario(DevScenarioConfig {
            id: DevScenarioId::TankTrapPathingMatrix,
            unit: EntityKind::Rifleman,
            count: 1,
            blocker: None,
            case: Some("explicit_infantry_attack"),
        }),
        None,
        false,
        DrainHandle::default(),
    );
    let (msg_tx, mut writer) = ConnectionSink::new();
    let (ack, mut ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(99, "Viewer".to_string(), true, false, msg_tx, ack);

    assert_eq!(ack_rx.try_recv(), Ok(true));
    assert_eq!(task.dev_view_player_id, Some(1));
    while writer.reliable_rx.try_recv().is_ok() {}

    let Phase::InGame(game) = &mut task.phase else {
        panic!("dev scenario should be live");
    };
    assert!(
        game.start_payload()
            .players
            .iter()
            .any(|player| player.id == 2),
        "scenario should include a non-view player"
    );
    game.enqueue(
        2,
        SimCommand::Rejected {
            reason: CommandRejection::Unit,
        },
    );

    task.on_tick(TokioInstant::now());

    let snapshot = writer.snapshots.take().expect("dev watch snapshot");
    assert!(
        snapshot
            .events
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "Unknown unit")),
        "full-world dev snapshot should include P2-only events, got {:?}",
        snapshot.events
    );
}
