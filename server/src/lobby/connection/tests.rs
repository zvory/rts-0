use super::*;
use crate::protocol::{
    ObserverAnalysisPayload, SnapshotPayloadEntityKindDiagnostics,
    SnapshotPayloadSectionDiagnostics,
};

fn observer_analysis_payload(tick: u32) -> ObserverAnalysisPayload {
    ObserverAnalysisPayload {
        tick,
        players: Vec::new(),
        map_analysis: None,
    }
}

fn room_time_state(tick: u32) -> RoomTimeState {
    RoomTimeState {
        current_tick: tick,
        duration_ticks: 1_000,
        keyframe_ticks: vec![0],
        speed: 2.0,
        paused: false,
        ended: false,
        controller_id: None,
        seek: None,
    }
}

#[test]
fn command_lifecycle_p95_uses_max_for_overflow_bucket() {
    let mut window = CommandTimingWindow::default();
    for _ in 0..20 {
        window.add(60_000);
    }

    let stats = window.consume();

    assert_eq!(stats.max_ms, 60_000);
    assert_eq!(stats.p95_ms, 60_000);
    assert_eq!(stats.count, 20);
}

#[test]
fn snapshot_lifecycle_stats_aggregate_payload_and_reset() {
    let counters = ConnectionReportCounters::default();
    counters.record_snapshot_projected(7, 3);
    counters.record_snapshot_taken(12);
    counters.record_snapshot_sent(12, true);
    counters.record_snapshot_written(SnapshotWriterSendStats {
        serialize_ms: 11,
        send_ms: 5,
        bytes: 1_000,
        payload: SnapshotPayloadDiagnostics {
            bytes: 1_000,
            sections: vec![
                SnapshotPayloadSectionDiagnostics {
                    section: "entities",
                    count: 6,
                    bytes: 600,
                },
                SnapshotPayloadSectionDiagnostics {
                    section: "visibility",
                    count: 10,
                    bytes: 250,
                },
            ],
            entity_kinds: vec![SnapshotPayloadEntityKindDiagnostics {
                kind: "worker".to_string(),
                count: 3,
                approx_bytes: 300,
            }],
        },
    });

    let stats = counters.consume();
    let lifecycle = stats.snapshot_lifecycle;
    assert_eq!(stats.snapshot_sent, 1);
    assert_eq!(stats.snapshot_waited_behind_reliable, 1);
    assert_eq!(lifecycle.writer_taken, 1);
    assert_eq!(lifecycle.projected.latest, 7);
    assert_eq!(lifecycle.projected.max, 7);
    assert_eq!(lifecycle.projected.p95, 8);
    assert_eq!(lifecycle.projected.avg, 7);
    assert_eq!(lifecycle.compacted.max, 3);
    assert_eq!(lifecycle.queue_age.max, 12);
    assert_eq!(lifecycle.serialized.max, 11);
    assert_eq!(lifecycle.writer_send.max, 5);
    assert_eq!(lifecycle.payload_bytes.latest, 1_000);
    assert_eq!(lifecycle.payload_bytes.p95, 1_024);
    assert_eq!(lifecycle.payload_bytes.total, 1_000);

    assert_eq!(lifecycle.sections.len(), 2);
    assert_eq!(lifecycle.sections[0].section, "entities");
    assert_eq!(lifecycle.sections[0].count, 6);
    assert_eq!(lifecycle.sections[0].bytes, 600);
    assert_eq!(lifecycle.sections[0].pct_x100, 6_000);
    assert_eq!(lifecycle.entity_kinds.len(), 1);
    assert_eq!(lifecycle.entity_kinds[0].kind, "worker");
    assert_eq!(lifecycle.entity_kinds[0].count, 3);
    assert_eq!(lifecycle.entity_kinds[0].approx_bytes, 300);
    assert_eq!(lifecycle.entity_kinds[0].pct_x100, 3_000);

    let reset = counters.consume();
    assert_eq!(reset.snapshot_sent, 0);
    assert_eq!(reset.snapshot_lifecycle.projected.count, 0);
    assert!(reset.snapshot_lifecycle.sections.is_empty());
    assert!(reset.snapshot_lifecycle.entity_kinds.is_empty());
}

#[test]
fn observer_analysis_uses_latest_only_slot() {
    let (sink, writer) = ConnectionSink::new();

    assert_eq!(
        sink.try_send_observer_analysis(observer_analysis_payload(10)),
        LatestOnlySendStatus::Stored
    );
    assert_eq!(
        sink.try_send_observer_analysis(observer_analysis_payload(11)),
        LatestOnlySendStatus::Replaced
    );

    let latest = writer.observer_analysis.take().expect("observer analysis");
    assert_eq!(latest.tick, 11);
    assert!(writer.observer_analysis.take().is_none());
}

#[test]
fn room_time_state_uses_latest_only_slot() {
    let (sink, writer) = ConnectionSink::new();

    assert_eq!(
        sink.try_send_room_time_state(room_time_state(10)),
        LatestOnlySendStatus::Stored
    );
    assert_eq!(
        sink.try_send_room_time_state(room_time_state(11)),
        LatestOnlySendStatus::Replaced
    );

    let latest = writer.room_time_state.take().expect("room-time state");
    assert_eq!(latest.current_tick, 11);
    assert!(writer.room_time_state.take().is_none());
}

#[test]
fn deferred_room_time_state_never_replaces_a_newer_publication() {
    let (sink, writer) = ConnectionSink::new();
    sink.try_send_room_time_state(room_time_state(10));
    let deferred = writer.room_time_state.take().expect("deferred state");

    sink.try_send_room_time_state(room_time_state(12));
    writer.room_time_state.defer(deferred);

    assert_eq!(
        writer
            .room_time_state
            .take()
            .expect("latest room-time state")
            .current_tick,
        12
    );
}

#[test]
fn room_time_state_defers_behind_a_preceding_reliable_barrier() {
    let (sink, mut writer) = ConnectionSink::new();
    sink.try_send_reliable(ServerMessage::RoomTimeSeekStarted {
        controller_id: 7,
        from_tick: 10,
        target_tick: 20,
    })
    .expect("seek-start barrier");
    sink.try_send_room_time_state(room_time_state(20));

    let deferred = writer.room_time_state.take().expect("room-time state");
    assert!(!writer.reliable_rx.is_empty());
    writer.room_time_state.defer(deferred);

    assert!(matches!(
        writer.reliable_rx.try_recv(),
        Ok(ServerMessage::RoomTimeSeekStarted { .. })
    ));
    assert_eq!(
        writer
            .room_time_state
            .take()
            .expect("state after reliable barrier")
            .current_tick,
        20
    );
}

#[test]
fn send_or_log_routes_room_time_state_outside_full_reliable_fifo() {
    let (sink, writer) = ConnectionSink::new();
    for index in 0..PLAYER_RELIABLE_CHANNEL_CAP {
        sink.try_send_reliable(ServerMessage::Error {
            msg: format!("queued-{index}"),
        })
        .expect("fill reliable queue");
    }

    send_or_log(
        "test-room",
        7,
        &sink,
        ServerMessage::RoomTimeState(room_time_state(20)),
    );
    send_or_log(
        "test-room",
        7,
        &sink,
        ServerMessage::RoomTimeState(room_time_state(21)),
    );

    assert_eq!(writer.reliable_rx.len(), PLAYER_RELIABLE_CHANNEL_CAP);
    let latest = writer.room_time_state.take().expect("room-time state");
    assert_eq!(latest.current_tick, 21);
}

#[test]
fn reliable_api_routes_room_time_state_to_latest_only_slot() {
    let (sink, mut writer) = ConnectionSink::new();

    sink.try_send_reliable(ServerMessage::RoomTimeState(room_time_state(40)))
        .expect("route room-time state");

    assert!(writer.reliable_rx.try_recv().is_err());
    assert_eq!(
        writer
            .room_time_state
            .take()
            .expect("room-time state")
            .current_tick,
        40
    );
}

#[test]
fn send_or_log_routes_observer_analysis_outside_reliable_fifo() {
    let (sink, mut writer) = ConnectionSink::new();

    send_or_log(
        "test-room",
        7,
        &sink,
        ServerMessage::ObserverAnalysis(observer_analysis_payload(20)),
    );
    send_or_log(
        "test-room",
        7,
        &sink,
        ServerMessage::ObserverAnalysis(observer_analysis_payload(21)),
    );

    assert!(writer.reliable_rx.try_recv().is_err());
    let latest = writer.observer_analysis.take().expect("observer analysis");
    assert_eq!(latest.tick, 21);
}

#[test]
fn clearing_pending_snapshot_preserves_room_time_and_clears_snapshot_state() {
    let (sink, writer) = ConnectionSink::new();

    assert_eq!(
        sink.try_send_snapshot(Snapshot {
            cosmic_energy: 0,
            tick: 1,
            ground_decal_revision: 0,
            ground_decal_delta: None,
            world_combat_position: None,
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 0,
            auto_build: None,
            entities: Vec::new(),
            resource_reset: false,
            resource_deltas: Vec::new(),
            smokes: Vec::new(),
            trenches: Vec::new(),
            ability_objects: Vec::new(),
            visible_tiles: Vec::new(),
            explored_tiles: Vec::new(),
            remembered_buildings: Vec::new(),
            remembered_anti_tank_guns: Vec::new(),
            events: Vec::new(),
            upgrades: Vec::new(),
            player_resources: Vec::new(),
            player_factions: Vec::new(),
            net_status: crate::protocol::SnapshotNetStatus::default(),
        }),
        SnapshotSendStatus::Stored
    );
    assert_eq!(
        sink.try_send_observer_analysis(observer_analysis_payload(30)),
        LatestOnlySendStatus::Stored
    );
    assert_eq!(
        sink.try_send_room_time_state(room_time_state(30)),
        LatestOnlySendStatus::Stored
    );

    sink.clear_pending_snapshot();

    assert!(writer.snapshots.take().is_none());
    assert_eq!(
        writer
            .room_time_state
            .take()
            .expect("room-time state remains pending")
            .current_tick,
        30
    );
    assert!(writer.observer_analysis.take().is_none());
}

#[test]
fn resource_delivery_diffs_taken_snapshots_and_resets_after_seek_view_and_rematch() {
    use super::super::tests::test_snapshot;
    let (sink, writer) = ConnectionSink::new();
    let mut stats = ConnectionWriterStats::new(sink.stats.clone());
    let full = |tick, remaining| {
        let mut snapshot = test_snapshot(tick, vec![ResourceDelta { id: 7, remaining }]);
        snapshot.resource_reset = true;
        snapshot
    };
    sink.try_send_snapshot(full(1, 625));
    let (first, _) = writer
        .snapshots
        .take_for_send(&mut stats)
        .unwrap()
        .into_parts();
    assert!(first.resource_reset);
    assert_eq!(first.resource_deltas.len(), 1);
    sink.try_send_snapshot(full(2, 625));
    let (unchanged, _) = writer
        .snapshots
        .take_for_send(&mut stats)
        .unwrap()
        .into_parts();
    assert!(!unchanged.resource_reset);
    assert!(
        unchanged.resource_deltas.is_empty(),
        "unchanged quantities cost no resource records"
    );
    sink.try_send_snapshot(full(3, 500));
    sink.try_send_snapshot(full(4, 0));
    let (depleted, _) = writer
        .snapshots
        .take_for_send(&mut stats)
        .unwrap()
        .into_parts();
    assert_eq!(
        depleted.resource_deltas,
        vec![ResourceDelta {
            id: 7,
            remaining: 0
        }]
    );
    // The first unsent replacement is itself coalesced. Its full-baseline requirement survives.
    sink.clear_pending_snapshot();
    sink.try_send_snapshot(full(2, 500));
    sink.try_send_snapshot(full(3, 625));
    let (reset, _) = writer
        .snapshots
        .take_for_send(&mut stats)
        .unwrap()
        .into_parts();
    assert!(reset.resource_reset);
    assert_eq!(
        reset.resource_deltas,
        vec![ResourceDelta {
            id: 7,
            remaining: 625
        }]
    );
    stats.reset_resource_baseline();
    sink.try_send_snapshot(full(0, 625));
    let (rematch, _) = writer
        .snapshots
        .take_for_send(&mut stats)
        .unwrap()
        .into_parts();
    assert!(rematch.resource_reset);
    assert_eq!(rematch.resource_deltas.len(), 1);
}
