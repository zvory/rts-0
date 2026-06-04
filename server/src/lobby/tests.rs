use std::time::Duration;

use super::connection::SnapshotSendStatus;
use super::room_task::{DevSelfPlayConfig, RoomMode, RoomTask};
use super::snapshots::compact_snapshot_for_wire;
use super::*;
use crate::protocol::{EntityView, Event, ResourceDelta};

fn join_test_player(task: &mut RoomTask, player_id: u32) {
    let (msg_tx, _writer) = ConnectionSink::new();
    let (ack, _ack_rx) = tokio::sync::oneshot::channel();
    task.on_join(player_id, format!("Player {player_id}"), false, msg_tx, ack);
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
        events: Vec::new(),
        player_resources: Vec::new(),
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
    let mut task = RoomTask::new("r".to_string(), RoomMode::Normal);

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
fn replay_rooms_default_to_1_5x_speed() {
    let normal = RoomTask::new("r".to_string(), RoomMode::Normal);
    let live = RoomTask::new(
        "r".to_string(),
        RoomMode::DevSelfPlay(DevSelfPlayConfig::Live),
    );
    let replay = RoomTask::new(
        "r".to_string(),
        RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
            artifact: "demo".to_string(),
        }),
    );
    assert_eq!(normal.current_tick_interval(), Duration::from_millis(33));
    assert_eq!(live.current_tick_interval(), Duration::from_millis(33));
    // 33ms / 1.5 = 22ms
    assert_eq!(replay.current_tick_interval(), Duration::from_millis(22));
}

#[test]
fn replay_speed_clamped_and_applied() {
    let mut task = RoomTask::new(
        "r".to_string(),
        RoomMode::DevSelfPlay(DevSelfPlayConfig::Replay {
            artifact: "demo".to_string(),
        }),
    );
    task.on_set_replay_speed(2.0);
    // 33ms / 2.0 = 16.5ms → rounds to 16ms via div_f32
    assert!(task.current_tick_interval() < Duration::from_millis(17));
    assert!(task.current_tick_interval() > Duration::from_millis(15));
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
        events: vec![Event::Notice {
            msg: "hello".to_string(),
            x: None,
            y: None,
            severity: crate::protocol::NoticeSeverity::Info,
        }],
        player_resources: Vec::new(),
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
        resource_deltas: Vec::new(),
        events: vec![Event::Death {
            id: 200,
            x: 30.0,
            y: 40.0,
            kind: kinds::STEEL.to_string(),
        }],
        player_resources: Vec::new(),
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
