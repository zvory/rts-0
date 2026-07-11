use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use super::connection::SnapshotSendStatus;
use super::room_task::{
    is_automated_match_history_room, match_history_participants_are_automated, RoomMode, RoomTask,
};
use super::snapshots::compact_snapshot_for_wire;
use super::*;
use crate::protocol::{kinds, EntityView, Event, ResourceDelta, DEFAULT_FACTION_ID};
use rts_sim::game::replay::ReplayArtifactV1;
use rts_sim::game::{Game, PlayerInit};

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

async fn join_room_handle(
    handle: &RoomHandle,
    player_id: u32,
    name: &str,
    spectator: bool,
) -> ConnectionWriter {
    join_room_handle_with_replay_ok(handle, player_id, name, spectator, false).await
}

async fn join_room_handle_with_replay_ok(
    handle: &RoomHandle,
    player_id: u32,
    name: &str,
    spectator: bool,
    replay_ok: bool,
) -> ConnectionWriter {
    let (msg_tx, writer) = ConnectionSink::new();
    let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
    handle
        .event_tx
        .send(RoomEvent::Join {
            player_id,
            name: name.to_string(),
            spectator,
            replay_ok,
            msg_tx,
            ack: ack_tx,
        })
        .await
        .expect("room task should accept join");
    assert_eq!(ack_rx.await, Ok(true));
    writer
}

async fn start_two_player_match(lobby: &Lobby, room: &str) -> RoomHandle {
    let handle = lobby.get_or_create(room).await;
    let _player_one = join_room_handle(&handle, 1, "Drain Alice", false).await;
    let _player_two = join_room_handle(&handle, 2, "Drain Bruno", false).await;
    handle
        .event_tx
        .send(RoomEvent::Ready {
            player_id: 1,
            ready: true,
        })
        .await
        .expect("room task should accept player one ready");
    handle
        .event_tx
        .send(RoomEvent::Ready {
            player_id: 2,
            ready: true,
        })
        .await
        .expect("room task should accept player two ready");
    handle
        .event_tx
        .send(RoomEvent::StartRequest { player_id: 1 })
        .await
        .expect("room task should accept start request");
    wait_for_active_match_count(lobby, 1).await;
    handle
}

async fn wait_for_lobby_room_count(lobby: &Lobby, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if lobby.rooms.lock().await.len() == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("lobby room count did not settle");
}

async fn wait_for_active_match_count(lobby: &Lobby, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if lobby.active_match_count() == expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("active match count did not settle");
}

fn test_drain() -> DrainHandle {
    DrainHandle::default()
}

fn registry_test_replay_artifact() -> ReplayArtifactV1 {
    let players = vec![PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: DEFAULT_FACTION_ID.to_string(),
        name: "Replay Player".to_string(),
        color: PLAYER_PALETTE[0].to_string(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x5150_3003);
    rts_sim::game::replay::ReplayStartComposition::capture(&game, crate::build_info::build_id())
        .unwrap()
        .finalize(&game, None, game.scores())
}

#[derive(Default)]
struct RecordingMatchHistoryWriter {
    records: Arc<StdMutex<Vec<crate::db::MatchRecord>>>,
    notify: Arc<tokio::sync::Notify>,
}

impl RecordingMatchHistoryWriter {
    async fn next_record(&self) -> crate::db::MatchRecord {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if let Some(record) = self
                    .records
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .first()
                    .cloned()
                {
                    return record;
                }
                self.notify.notified().await;
            }
        })
        .await
        .expect("recorded match write did not arrive")
    }
}

impl match_history_writes::MatchHistoryWriter for RecordingMatchHistoryWriter {
    fn record_match(
        &self,
        rec: crate::db::MatchRecord,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> {
        let records = Arc::clone(&self.records);
        let notify = Arc::clone(&self.notify);
        Box::pin(async move {
            records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(rec);
            notify.notify_waiters();
        })
    }
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
        trenches: Vec::new(),
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
fn connection_sink_report_stats_reset_after_consume() {
    let (sink, _writer) = ConnectionSink::new();

    sink.try_send_reliable(ServerMessage::CommandReceipt {
        client_seq: 1,
        server_tick: 10,
        accepted: true,
        reason: None,
    })
    .unwrap();
    sink.try_send_reliable(ServerMessage::CommandReceipt {
        client_seq: 2,
        server_tick: 10,
        accepted: false,
        reason: Some("notPlayer".to_string()),
    })
    .unwrap();
    assert_eq!(
        sink.try_send_snapshot(test_snapshot(10, Vec::new())),
        SnapshotSendStatus::Stored
    );
    assert_eq!(
        sink.try_send_snapshot(test_snapshot(11, Vec::new())),
        SnapshotSendStatus::Replaced
    );

    let stats = sink.consume_report_stats();
    assert_eq!(stats.command_receipts_accepted, 1);
    assert_eq!(stats.command_receipts_rejected, 1);
    assert_eq!(stats.snapshot_slot_stored, 1);
    assert_eq!(stats.snapshot_slot_replaced, 1);

    let reset = sink.consume_report_stats();
    assert_eq!(reset.command_receipts_accepted, 0);
    assert_eq!(reset.command_receipts_rejected, 0);
    assert_eq!(reset.snapshot_slot_stored, 0);
    assert_eq!(reset.snapshot_slot_replaced, 0);
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
    task.on_set_room_time_speed(1, 2.0);
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
        Err(JoinTargetError::Draining(DrainNotice {
            seconds_remaining: 295,
            ..
        }))
    ));
}

#[tokio::test]
async fn unused_map_editor_lab_rooms_expire_and_cannot_be_recreated_by_name() {
    let lobby = Lobby::new();
    let size = 126;
    let room = lobby
        .create_map_editor_lab_room(LabMapDraft {
            name: "Private editor map".to_string(),
            size,
            terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
            starts: vec![
                crate::protocol::LabMapTile { x: 16, y: 16 },
                crate::protocol::LabMapTile {
                    x: size - 17,
                    y: size - 17,
                },
            ],
            expansion_sites: Vec::new(),
        })
        .await
        .expect("room should be created");
    wait_for_lobby_room_count(&lobby, 0).await;
    assert!(matches!(
        lobby.get_or_create_join_target(&room).await,
        Err(JoinTargetError::MissingPrivateRoom)
    ));
}

#[tokio::test]
async fn draining_rejects_new_map_editor_lab_rooms() {
    let lobby = Lobby::new();
    lobby.begin_draining(Duration::from_secs(295)).await;

    let size = 126;
    let result = lobby
        .create_map_editor_lab_room(LabMapDraft {
            name: "Private editor map".to_string(),
            size,
            terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
            starts: vec![crate::protocol::LabMapTile { x: 16, y: 16 }],
            expansion_sites: Vec::new(),
        })
        .await;

    assert!(matches!(result, Err(DrainNotice { .. })));
}

#[tokio::test]
async fn shutdown_finalization_acks_rooms_and_clears_active_match_count() {
    let lobby = Lobby::new();
    let handle = lobby.get_or_create("shutdown-finalize-room").await;
    let _writer = join_room_handle(&handle, 1, "Player 1", false).await;
    handle
        .event_tx
        .send(RoomEvent::Ready {
            player_id: 1,
            ready: true,
        })
        .await
        .expect("room task should accept ready");
    handle
        .event_tx
        .send(RoomEvent::StartRequest { player_id: 1 })
        .await
        .expect("room task should accept start");
    wait_for_active_match_count(&lobby, 1).await;

    let summary = lobby
        .finalize_active_matches_for_shutdown(Duration::from_secs(1))
        .await;

    assert_eq!(summary.active_matches_before, 1);
    assert_eq!(summary.active_matches_after, 0);
    assert_eq!(summary.rooms_requested, 1);
    assert_eq!(summary.rooms_acked, 1);
    assert_eq!(summary.rooms_unacked, 0);
    assert_eq!(summary.finalized_matches, 1);
    assert_eq!(summary.history_allowed_matches, 1);
    assert_eq!(summary.records_queued, 0);
    assert!(!summary.timed_out);
}

#[tokio::test]
async fn deploy_drain_records_aborted_replay_backed_match_before_connection_shutdown() {
    let writer = Arc::new(RecordingMatchHistoryWriter::default());
    let lobby = Lobby::new().with_match_history_writer_for_test(Some(writer.clone()), false);
    let handle = start_two_player_match(&lobby, "shutdown-abort-record-room").await;
    let mut shutdown_rx = lobby.subscribe_connection_shutdown();

    tokio::time::timeout(
        Duration::from_secs(1),
        lobby.run_deploy_drain_with_budget(DeployDrainBudget {
            natural_match_drain: Duration::from_millis(5),
            forced_finalization: Duration::from_millis(500),
            match_history_write_wait: Duration::from_millis(500),
            shutdown_slack: Duration::ZERO,
        }),
    )
    .await
    .expect("deploy drain should complete with the local recording sink");

    let record = writer.next_record().await;
    assert_eq!(lobby.active_match_count(), 0);
    assert!(
        *shutdown_rx.borrow_and_update(),
        "connection shutdown should be requested after the aborted record is queued and awaited"
    );
    assert_eq!(record.outcome, crate::db::MatchOutcome::Aborted);
    assert_eq!(record.winner_name, None);
    assert_eq!(
        record.participants,
        vec!["Drain Alice".to_string(), "Drain Bruno".to_string()]
    );
    assert_eq!(record.human_count, 2);
    assert!(!record.debug_mode);
    assert!(!record.local_only);
    assert_eq!(
        record
            .score_screen
            .as_array()
            .expect("score screen should serialize as an array")
            .len(),
        2
    );

    let replay = record
        .replay
        .expect("aborted match should include a replay row");
    assert_eq!(replay.map_name, "Default");
    assert!(replay.artifact_schema_version > 0);
    assert!(replay.artifact_json["winnerId"].is_null());
    assert!(replay.artifact_json["winnerTeamId"].is_null());
    assert_eq!(
        replay
            .artifact_json
            .get("finalScores")
            .and_then(|scores| scores.as_array())
            .map(Vec::len),
        Some(2)
    );

    handle
        .event_tx
        .send(RoomEvent::Leave { player_id: 1 })
        .await
        .expect("room task should accept player one cleanup leave");
    handle
        .event_tx
        .send(RoomEvent::Leave { player_id: 2 })
        .await
        .expect("room task should accept player two cleanup leave");
    wait_for_active_match_count(&lobby, 0).await;
}

#[tokio::test]
async fn deploy_drain_write_wait_is_bounded_when_tracked_write_hangs() {
    let lobby = Lobby::new();
    lobby
        .drain
        .track_match_history_write(std::future::pending::<()>());

    let started = std::time::Instant::now();
    lobby
        .run_deploy_drain_with_budget(DeployDrainBudget {
            natural_match_drain: Duration::ZERO,
            forced_finalization: Duration::ZERO,
            match_history_write_wait: Duration::from_millis(25),
            shutdown_slack: Duration::ZERO,
        })
        .await;

    assert!(
        started.elapsed() < Duration::from_secs(1),
        "write wait should honor its bounded budget"
    );
    assert_eq!(lobby.pending_match_history_write_count(), 1);
    assert!(*lobby.subscribe_connection_shutdown().borrow());
}

#[tokio::test]
async fn create_lobby_rejects_duplicate_names() {
    let lobby = Lobby::new();

    let room = lobby
        .create_lobby("  alex's lobby  ")
        .await
        .expect("first create should reserve normalized lobby name");
    assert_eq!(room, "alex's lobby");

    assert!(matches!(
        lobby.create_lobby("alex's lobby").await,
        Err(CreateLobbyError::Duplicate)
    ));
}

#[tokio::test]
async fn create_lobby_abandoned_reservation_expires_and_name_can_be_recreated() {
    let lobby = Lobby::new();

    let room = lobby
        .create_lobby("abandoned-browser-room")
        .await
        .expect("first create should reserve the lobby name");

    assert!(matches!(
        lobby.create_lobby(&room).await,
        Err(CreateLobbyError::Duplicate)
    ));

    wait_for_lobby_room_count(&lobby, 0).await;

    assert_eq!(
        lobby
            .create_lobby(&room)
            .await
            .expect("expired pending create lease should release the name"),
        room
    );
}

#[tokio::test]
async fn create_lobby_rejects_invalid_and_reserved_names() {
    let lobby = Lobby::new();
    let too_long = "x".repeat(PUBLIC_LOBBY_NAME_MAX_BYTES + 1);

    assert!(matches!(
        lobby.create_lobby("   ").await,
        Err(CreateLobbyError::EmptyName)
    ));
    assert!(matches!(
        lobby.create_lobby(&too_long).await,
        Err(CreateLobbyError::NameTooLong { .. })
    ));
    assert!(matches!(
        lobby.create_lobby("bad\nroom").await,
        Err(CreateLobbyError::InvalidCharacters)
    ));
    for reserved in [
        "__dev_scenario__:direct_reverse_order:worker:1",
        "__replay_artifact__:demo",
        "__match_replay__:00000001",
        "__replay_branch__:00000001",
        "__lab__:sandbox:map=Default",
    ] {
        assert!(
            matches!(
                lobby.create_lobby(reserved).await,
                Err(CreateLobbyError::ReservedName)
            ),
            "{reserved} should be reserved"
        );
    }
}

#[tokio::test]
async fn create_lobby_drain_rejects_new_names_but_existing_rooms_remain_joinable() {
    let lobby = Lobby::new();
    let existing = lobby
        .create_lobby("existing-browser-room")
        .await
        .expect("pre-drain lobby should be created");

    lobby.begin_draining(Duration::from_secs(295)).await;

    assert!(matches!(
        lobby.create_lobby("new-browser-room").await,
        Err(CreateLobbyError::Draining(DrainNotice {
            seconds_remaining: 295,
            ..
        }))
    ));
    assert!(lobby.get_or_create_join_target(&existing).await.is_ok());
}

#[tokio::test]
async fn registry_disposal_removes_matching_room() {
    let lobby = Lobby::new();
    let handle = lobby.get_or_create("disposable-room").await;

    handle
        .event_tx
        .send(RoomEvent::ReportDisposableIfEmpty)
        .await
        .expect("room task should accept disposal probe");

    wait_for_lobby_room_count(&lobby, 0).await;
}

#[tokio::test]
async fn registry_disposal_ignores_stale_room_identity() {
    let lobby = Lobby::new();
    let old = lobby.get_or_create("reused-name").await;
    let old_identity = old.identity;

    assert!(
        lobby
            .request_room_disposal_for_test("reused-name", old_identity)
            .await,
        "old room should be removable before the name is reused"
    );
    wait_for_lobby_room_count(&lobby, 0).await;

    let newer = lobby.get_or_create("reused-name").await;
    assert_ne!(newer.identity, old_identity);
    assert!(
        !lobby
            .request_room_disposal_for_test("reused-name", old_identity)
            .await,
        "stale disposal must not remove a newer room under the same name"
    );

    let rooms = lobby.rooms.lock().await;
    let current = rooms
        .get("reused-name")
        .expect("newer room should remain registered");
    assert_eq!(current.identity, newer.identity);
}

#[tokio::test]
async fn registry_disposal_stops_room_task() {
    let lobby = Lobby::new();
    let handle = lobby.get_or_create("shutdown-room").await;
    let event_tx = handle.event_tx.clone();

    handle
        .event_tx
        .send(RoomEvent::ReportDisposableIfEmpty)
        .await
        .expect("room task should accept disposal probe");
    wait_for_lobby_room_count(&lobby, 0).await;

    tokio::time::timeout(Duration::from_secs(1), event_tx.closed())
        .await
        .expect("disposed room task should close its event receiver");
}

#[tokio::test]
async fn lobby_summaries_collect_browser_safe_rows_from_room_tasks() {
    let lobby = Lobby::new();
    let room = lobby
        .create_lobby("summary-collection")
        .await
        .expect("lobby should be created");
    let handle = lobby
        .get_or_create_join_target(&room)
        .await
        .expect("created lobby should stay joinable");
    let _writer = join_room_handle(&handle, 9001, "Browser Host", false).await;

    let summaries = lobby.summaries().await;
    let summary = summaries
        .iter()
        .find(|summary| summary.room == room)
        .expect("joined normal lobby should be summarized");

    assert_eq!(summary.host_name.as_deref(), Some("Browser Host"));
    assert_eq!(summary.kind, crate::protocol::LobbyKind::Normal);
    assert_eq!(summary.phase, LobbySummaryPhase::Lobby);
    assert_eq!(summary.join_state, LobbyJoinState::Open);
    assert_eq!(summary.occupied_slots, 1);
    assert!(matches!(
        lobby.create_lobby(&room).await,
        Err(CreateLobbyError::Duplicate)
    ));

    handle
        .event_tx
        .send(RoomEvent::Leave { player_id: 9001 })
        .await
        .expect("cleanup leave should send");
}

#[tokio::test]
async fn empty_public_lobby_has_no_reconnect_grace_and_releases_name() {
    let lobby = Lobby::new();
    let room = lobby
        .create_lobby("no-reconnect-grace")
        .await
        .expect("lobby should be created");
    let handle = lobby
        .get_or_create_join_target(&room)
        .await
        .expect("created lobby should stay joinable");
    let _writer = join_room_handle(&handle, 42, "Departing Host", false).await;

    assert!(lobby
        .summaries()
        .await
        .iter()
        .any(|summary| summary.room == room));

    handle
        .event_tx
        .send(RoomEvent::Leave { player_id: 42 })
        .await
        .expect("leave should send");
    wait_for_lobby_room_count(&lobby, 0).await;

    assert!(!lobby
        .summaries()
        .await
        .iter()
        .any(|summary| summary.room == room));
    assert_eq!(
        lobby
            .create_lobby(&room)
            .await
            .expect("empty public lobby name should be available immediately"),
        room
    );
}

#[tokio::test]
async fn empty_recreatable_internal_rooms_are_disposed_and_hidden_from_browser() {
    let lobby = Lobby::new();
    for (idx, (room, replay_ok)) in [
        (
            "__dev_scenario__:direct_reverse_order:unit=tank:count=1".to_string(),
            false,
        ),
        ("__lab__:phase3-lab:map=Default:seed=123".to_string(), false),
        (
            "__replay_artifact__:phase3-missing-artifact".to_string(),
            true,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let handle = lobby.get_or_create(&room).await;
        let _writer = join_room_handle_with_replay_ok(
            &handle,
            7000 + idx as u32,
            "Internal Viewer",
            true,
            replay_ok,
        )
        .await;

        assert!(
            lobby
                .summaries()
                .await
                .iter()
                .all(|summary| summary.room != room),
            "{room} should stay out of the public lobby browser while occupied"
        );

        handle
            .event_tx
            .send(RoomEvent::Leave {
                player_id: 7000 + idx as u32,
            })
            .await
            .expect("internal room leave should send");
        wait_for_lobby_room_count(&lobby, 0).await;
    }
}

#[tokio::test]
async fn empty_persisted_replay_room_is_disposed_after_visible_staging_lobby() {
    let lobby = Lobby::new();
    let room = lobby
        .create_replay_room(registry_test_replay_artifact())
        .await;
    let handle = lobby
        .get_or_create_join_target(&room)
        .await
        .expect("created replay room should be joinable");
    let _writer = join_room_handle_with_replay_ok(&handle, 7100, "Replay Viewer", true, true).await;

    let summaries = lobby.summaries().await;
    let summary = summaries
        .iter()
        .find(|summary| summary.room == room)
        .expect("occupied persisted replay staging lobby should be browser-visible");
    assert_eq!(summary.kind, crate::protocol::LobbyKind::Replay);
    assert_eq!(summary.occupied_slots, 0);
    assert_eq!(summary.max_slots, 0);
    assert_eq!(summary.spectator_count, 1);
    assert_eq!(summary.join_state, LobbyJoinState::FullSpectatorOnly);

    handle
        .event_tx
        .send(RoomEvent::Leave { player_id: 7100 })
        .await
        .expect("replay room leave should send");
    wait_for_lobby_room_count(&lobby, 0).await;
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
        "AI 2.1".to_string(),
    ]));
    assert!(match_history_participants_are_automated(&[
        "Alpha".to_string(),
        "Bravo".to_string(),
    ]));
    assert!(!match_history_participants_are_automated(&[
        "Player".to_string(),
        "AI 2.1".to_string(),
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
        trenches: Vec::new(),
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
        trenches: Vec::new(),
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
