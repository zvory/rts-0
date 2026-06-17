use std::time::Duration;

use super::connection::SnapshotSendStatus;
use super::room_task::{
    is_automated_match_history_room, match_history_participants_are_automated, RoomMode, RoomTask,
};
use super::snapshots::compact_snapshot_for_wire;
use super::*;
use crate::protocol::{kinds, EntityView, Event, ResourceDelta};

fn join_test_player(task: &mut RoomTask, player_id: u32) {
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        player_id,
        format!("Player {player_id}"),
        false,
        false,
        msg_tx,
        ack,
    );
}

fn join_test_player_with_writer(task: &mut RoomTask, player_id: u32) -> ConnectionWriter {
    let (msg_tx, writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(
        player_id,
        format!("Player {player_id}"),
        false,
        false,
        msg_tx,
        ack,
    );
    writer
}

fn test_drain() -> DrainHandle {
    DrainHandle::default()
}

fn test_snapshot(tick: u32, resource_deltas: Vec<ResourceDelta>) -> Snapshot {
    Snapshot {
        tick,
        steel: 75,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: vec![EntityView::new(
            1,
            1,
            kinds::WORKER,
            10.0,
            20.0,
            40,
            40,
            "idle",
        )],
        resource_deltas,
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: crate::protocol::SnapshotNetStatus::default(),
    }
}

#[test]
fn connection_sink_keeps_reliable_fifo_separate_from_snapshots() {
    let (sink, mut writer) = ConnectionSink::new();

    sink.try_send_snapshot(test_snapshot(10, Vec::new()));
    sink.try_send_reliable(ServerMessage::Error {
        msg: "first".to_string(),
    })
    .unwrap();
    sink.try_send_reliable(ServerMessage::Pong { ts: 42.0 })
        .unwrap();

    let first = writer.reliable_rx.try_recv().unwrap();
    let second = writer.reliable_rx.try_recv().unwrap();

    assert!(matches!(first, ServerMessage::Error { .. }));
    assert!(matches!(second, ServerMessage::Pong { ts } if ts == 42.0));
    assert_eq!(writer.snapshots.take().unwrap().tick, 10);
}

#[test]
fn connection_sink_coalesces_snapshots_to_latest_tick() {
    let (sink, writer) = ConnectionSink::new();

    assert_eq!(
        sink.try_send_snapshot(test_snapshot(10, Vec::new())),
        SnapshotSendStatus::Stored
    );
    assert_eq!(
        sink.try_send_snapshot(test_snapshot(11, Vec::new())),
        SnapshotSendStatus::Replaced
    );

    let snapshot = writer.snapshots.take().unwrap();
    assert_eq!(snapshot.tick, 11);
    assert!(writer.snapshots.take().is_none());
}

#[test]
fn connection_sink_carries_resource_deltas_across_snapshot_replacement() {
    let (sink, writer) = ConnectionSink::new();

    sink.try_send_snapshot(test_snapshot(
        10,
        vec![ResourceDelta {
            id: 200,
            remaining: 1498,
        }],
    ));
    sink.try_send_snapshot(test_snapshot(11, Vec::new()));

    let snapshot = writer.snapshots.take().unwrap();
    assert_eq!(snapshot.tick, 11);
    assert_eq!(
        snapshot.resource_deltas,
        vec![ResourceDelta {
            id: 200,
            remaining: 1498,
        }]
    );
}

#[test]
fn connection_sink_keeps_newest_resource_delta_for_same_node() {
    let (sink, writer) = ConnectionSink::new();

    sink.try_send_snapshot(test_snapshot(
        10,
        vec![ResourceDelta {
            id: 200,
            remaining: 1498,
        }],
    ));
    sink.try_send_snapshot(test_snapshot(
        11,
        vec![ResourceDelta {
            id: 200,
            remaining: 1496,
        }],
    ));

    let snapshot = writer.snapshots.take().unwrap();
    assert_eq!(
        snapshot.resource_deltas,
        vec![ResourceDelta {
            id: 200,
            remaining: 1496,
        }]
    );
}

#[test]
fn joining_after_earlier_player_leaves_reuses_open_color() {
    let mut task = RoomTask::new("r".to_string(), RoomMode::Normal, None, false, test_drain());

    join_test_player(&mut task, 1);
    join_test_player(&mut task, 2);
    join_test_player(&mut task, 3);
    task.on_leave(1);
    join_test_player(&mut task, 4);

    let color_2 = &task.players.get(&2).unwrap().color;
    let color_3 = &task.players.get(&3).unwrap().color;
    let color_4 = &task.players.get(&4).unwrap().color;

    assert_eq!(color_4, PLAYER_PALETTE[0]);
    assert_ne!(color_4, color_2);
    assert_ne!(color_4, color_3);
}

#[test]
fn saved_artifact_replay_rooms_use_normal_tick_until_replay_viewer_starts() {
    let normal = RoomTask::new("r".to_string(), RoomMode::Normal, None, false, test_drain());
    let replay = RoomTask::new(
        "r".to_string(),
        RoomMode::ReplayArtifact {
            artifact: "demo".to_string(),
        },
        None,
        false,
        test_drain(),
    );
    assert_eq!(normal.current_tick_interval(), Duration::from_millis(33));
    assert_eq!(replay.current_tick_interval(), Duration::from_millis(33));
}

#[test]
fn saved_artifact_replay_speed_is_ignored_until_replay_viewer_starts() {
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::ReplayArtifact {
            artifact: "demo".to_string(),
        },
        None,
        false,
        test_drain(),
    );
    task.on_set_replay_speed(1, 2.0);
    assert_eq!(task.current_tick_interval(), Duration::from_millis(33));
}

#[test]
fn drain_handle_tracks_active_matches() {
    let drain = DrainHandle::default();

    assert!(!drain.is_draining());
    assert_eq!(drain.active_matches(), 0);

    let notice = drain.begin_draining(Duration::from_secs(295));
    assert!(drain.is_draining());
    assert_eq!(drain.notice(), Some(notice));
    assert_eq!(notice.seconds_remaining, 295);
    assert_eq!(drain.begin_draining(Duration::from_secs(5)), notice);

    drain.match_started();
    drain.match_started();
    assert_eq!(drain.active_matches(), 2);

    drain.match_finished();
    assert_eq!(drain.active_matches(), 1);

    drain.match_finished();
    assert_eq!(drain.active_matches(), 0);

    drain.match_finished();
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn draining_join_sends_warning_and_start_is_blocked() {
    let drain = DrainHandle::default();
    let notice = drain.begin_draining(Duration::from_secs(295));
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let mut writer = join_test_player_with_writer(&mut task, 1);

    task.on_ready(1, true);
    task.on_start_request(1);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages.iter().any(|msg| {
        matches!(
            msg,
            ServerMessage::ShutdownWarning {
                deadline_unix_ms,
                seconds_remaining,
            } if *deadline_unix_ms == notice.deadline_unix_ms && *seconds_remaining == 295
        )
    }));
    assert!(messages.iter().any(|msg| matches!(
        msg,
        ServerMessage::Lobby {
            can_start: false,
            ..
        }
    )));
    assert!(messages
        .iter()
        .any(|msg| { matches!(msg, ServerMessage::Error { msg } if msg.contains("draining")) }));
    assert_eq!(drain.active_matches(), 0);
}

#[test]
fn one_player_start_skips_match_countdown() {
    let drain = test_drain();
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let mut writer = join_test_player_with_writer(&mut task, 1);

    task.on_ready(1, true);
    task.on_start_request(1);

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
fn quickstart_skips_match_countdown() {
    let drain = test_drain();
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let mut writer = join_test_player_with_writer(&mut task, 1);

    join_test_player(&mut task, 2);
    task.on_set_quickstart(1, true);
    task.on_ready(1, true);
    task.on_ready(2, true);
    task.on_start_request(1);

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
fn normal_multiplayer_start_uses_match_countdown() {
    let drain = test_drain();
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::Normal,
        None,
        false,
        drain.clone(),
    );
    let mut writer = join_test_player_with_writer(&mut task, 1);

    join_test_player(&mut task, 2);
    task.on_ready(1, true);
    task.on_ready(2, true);
    task.on_start_request(1);

    let messages: Vec<_> = std::iter::from_fn(|| writer.reliable_rx.try_recv().ok()).collect();
    assert!(messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::MatchCountdown { .. })));
    assert!(!messages
        .iter()
        .any(|msg| matches!(msg, ServerMessage::Start(_))));
    assert_eq!(drain.active_matches(), 0);
}

#[tokio::test]
async fn draining_rejects_new_rooms_but_keeps_existing_rooms_joinable() {
    let lobby = Lobby::new();
    let existing = lobby.get_or_create("existing").await;

    lobby.begin_draining(Duration::from_secs(295)).await;

    let joined_existing = lobby
        .get_or_create_join_target("existing")
        .await
        .expect("existing rooms should remain joinable during drain");
    assert!(existing.event_tx.same_channel(&joined_existing.event_tx));

    let rejected = lobby.get_or_create_join_target("new-room").await;
    assert!(matches!(
        rejected,
        Err(DrainNotice {
            seconds_remaining: 295,
            ..
        })
    ));
}

#[tokio::test]
async fn drain_waiter_releases_after_last_match_finishes() {
    let drain = DrainHandle::default();
    drain.match_started();

    let mut waiter = {
        let drain = drain.clone();
        tokio::spawn(async move {
            drain.wait_for_matches_to_drain().await;
        })
    };

    tokio::select! {
        result = &mut waiter => {
            result.expect("drain waiter task should not panic");
            panic!("waiter should remain pending while a match is active");
        }
        _ = tokio::time::sleep(Duration::from_millis(20)) => {}
    }

    drain.match_finished();
    waiter.await.expect("drain waiter task should not panic");
}

#[test]
fn automated_match_history_rooms_are_detected() {
    assert!(is_automated_match_history_room("itest-123"));
    assert!(is_automated_match_history_room("ai-itest-123"));
    assert!(is_automated_match_history_room("client-smoke-123"));
    assert!(is_automated_match_history_room("reg-join-123"));
    assert!(!is_automated_match_history_room("main"));
    assert!(!is_automated_match_history_room("ranked-1"));
}

#[test]
fn automated_match_history_participants_are_detected() {
    assert!(match_history_participants_are_automated(&[
        "smoke".to_string(),
        "Computer 1".to_string(),
    ]));
    assert!(match_history_participants_are_automated(&[
        "Alpha".to_string(),
        "Bravo".to_string(),
    ]));
    assert!(!match_history_participants_are_automated(&[
        "Player".to_string(),
        "Computer 2".to_string(),
    ]));
    assert!(!match_history_participants_are_automated(&[
        "Alpha".to_string(),
        "Charlie".to_string(),
    ]));
    assert!(!match_history_participants_are_automated(&[
        "Player 1".to_string(),
        "Player 2".to_string(),
    ]));
}

#[test]
fn wire_compaction_removes_resource_entities_but_keeps_deltas() {
    let mut snapshot = Snapshot {
        tick: 10,
        steel: 75,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: vec![
            EntityView::new(1, 1, kinds::WORKER, 10.0, 20.0, 40, 40, "idle"),
            EntityView::new(2, 0, kinds::STEEL, 30.0, 40.0, 1, 1, "idle"),
            EntityView::new(3, 0, kinds::OIL, 50.0, 60.0, 1, 1, "idle"),
        ],
        resource_deltas: vec![ResourceDelta {
            id: 2,
            remaining: 1498,
        }],
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: vec![Event::Notice {
            msg: "hello".to_string(),
            x: None,
            y: None,
            severity: crate::protocol::NoticeSeverity::Info,
        }],
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: crate::protocol::SnapshotNetStatus::default(),
    };

    compact_snapshot_for_wire(&mut snapshot);

    assert_eq!(snapshot.entities.len(), 1);
    assert_eq!(snapshot.entities[0].kind, kinds::WORKER);
    assert_eq!(snapshot.resource_deltas.len(), 1);
    assert_eq!(snapshot.resource_deltas[0].remaining, 1498);
    assert_eq!(snapshot.events.len(), 1);
}

#[test]
fn wire_compaction_converts_visible_resource_death_to_zero_delta() {
    let mut snapshot = Snapshot {
        tick: 10,
        steel: 75,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: vec![EntityView::new(
            1,
            1,
            kinds::WORKER,
            10.0,
            20.0,
            40,
            40,
            "idle",
        )],
        smokes: Vec::new(),
        resource_deltas: Vec::new(),
        ability_objects: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: vec![Event::Death {
            id: 200,
            x: 30.0,
            y: 40.0,
            kind: kinds::STEEL.to_string(),
        }],
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: crate::protocol::SnapshotNetStatus::default(),
    };

    compact_snapshot_for_wire(&mut snapshot);

    assert_eq!(
        snapshot.resource_deltas,
        vec![ResourceDelta {
            id: 200,
            remaining: 0,
        }]
    );
}
