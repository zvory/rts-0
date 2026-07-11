pub(super) use super::super::super::lab_timeline::LabTimelineEntryKind;
pub(super) use super::super::super::replay_branch::BranchStagingState;
pub(super) use super::super::*;
use crate::lobby::{CommandLifecycleFamily, CommandLifecycleTiming};
pub(super) use crate::protocol::{NoticeSeverity, ObserverAnalysisPayload, DEFAULT_FACTION_ID};
pub(super) use rts_rules::faction::{EKAT_FACTION_ID, EMPTY_FIXTURE_FACTION_ID};
pub(super) use rts_sim::game::command::SimCommand;
pub(super) use rts_sim::game::map::Map;
use std::time::Instant as StdInstant;

pub(super) use super::super::helpers::DRAINING_NEW_MATCHES_DISABLED_MSG;

pub(super) trait RoomTaskCommandTestExt {
    fn on_command(&mut self, player_id: u32, client_seq: u32, cmd: SimCommand);
}

impl RoomTaskCommandTestExt for RoomTask {
    fn on_command(&mut self, player_id: u32, client_seq: u32, cmd: SimCommand) {
        let now = StdInstant::now();
        self.on_command_with_lifecycle(
            player_id,
            client_seq,
            cmd,
            CommandLifecycleTiming {
                received_unix_ms: 0,
                frame_received_at: now,
                deserialized_at: now,
                room_event_enqueued_at: now,
                family: CommandLifecycleFamily::Other,
            },
        );
    }
}

pub(super) fn replay_test_players(count: usize) -> Vec<PlayerInit> {
    (1..=count as u32)
        .map(|id| PlayerInit {
            id,
            team_id: id,
            faction_id: "kriegsia".to_string(),
            name: format!("Player {id}"),
            color: PLAYER_PALETTE[(id as usize - 1) % PLAYER_PALETTE.len()].to_string(),
            is_ai: false,
        })
        .collect()
}

pub(super) fn replay_test_game(players: &[PlayerInit], seed: u32) -> Game {
    let metadata = Map::metadata_for_name("Default").unwrap();
    let start_players: Vec<_> = players
        .iter()
        .map(|player| {
            let team_id = if player.team_id == 0 {
                player.id
            } else {
                player.team_id
            };
            (player.id, team_id)
        })
        .collect();
    let map = Map::load_for_players("Default", &start_players, seed).unwrap();
    Game::new_with_random_ai_profiles_and_map_metadata(players, seed, map, metadata)
}

pub(super) fn replay_test_artifact(players: &[PlayerInit], ticks: u32) -> (Game, ReplayArtifactV1) {
    let (game, _replay_start, artifact) = replay_test_artifact_with_start(players, ticks);
    (game, artifact)
}

pub(super) fn replay_test_artifact_with_start(
    players: &[PlayerInit],
    ticks: u32,
) -> (
    Game,
    rts_sim::game::replay::ReplayStartComposition,
    ReplayArtifactV1,
) {
    let seed = 0x5150_2202;
    let mut game = replay_test_game(players, seed);
    let replay_start =
        rts_sim::game::replay::ReplayStartComposition::capture(&game, server_build_sha()).unwrap();
    for _ in 0..ticks {
        game.tick();
    }
    let artifact = replay_start.finalize(&game, None, game.scores());
    (game, replay_start, artifact)
}

pub(super) fn replay_branch_test_seed(players: &[PlayerInit], ticks: u32) -> ReplayBranchSeed {
    let (_live, artifact) = replay_test_artifact(players, ticks);
    let mut replay = ReplaySession::new(artifact).unwrap();
    while replay.current_tick() < ticks {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    replay.branch_seed().unwrap()
}

pub(super) fn write_selfplay_replay_test_artifact(
    name: &str,
    artifact: &ReplayArtifactV1,
) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("selfplay-artifacts")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("replay.json"),
        serde_json::to_vec_pretty(artifact).unwrap(),
    )
    .unwrap();
    dir
}

pub(super) fn add_test_room_player(task: &mut RoomTask, id: u32, ready: bool) -> ConnectionWriter {
    let (msg_tx, writer) = ConnectionSink::new();
    task.order.push(id);
    task.players.insert(
        id,
        RoomPlayer {
            name: format!("Player {id}"),
            color: PLAYER_PALETTE[(id as usize - 1) % PLAYER_PALETTE.len()].to_string(),
            ready,
            spectator: false,
            msg_tx,
            head_of_line_count: 0,
            last_received_client_seq: 0,
            last_sim_consumed_client_seq: 0,
            last_sim_consumed_client_tick: None,
        },
    );
    writer
}

pub(super) fn add_test_room_spectator(task: &mut RoomTask, id: u32) -> ConnectionWriter {
    let (msg_tx, writer) = ConnectionSink::new();
    task.order.push(id);
    task.players.insert(
        id,
        RoomPlayer {
            name: format!("Spectator {id}"),
            color: "#6f8fa8".to_string(),
            ready: true,
            spectator: true,
            msg_tx,
            head_of_line_count: 0,
            last_received_client_seq: 0,
            last_sim_consumed_client_seq: 0,
            last_sim_consumed_client_tick: None,
        },
    );
    writer
}

pub(super) fn lab_config() -> LabRoomConfig {
    LabRoomConfig {
        public_id: "sandbox".to_string(),
        map_name: "Default".to_string(),
        seed: Some(0x1A2B_3C4D),
        scenario: None,
        map_draft: None,
    }
}

pub(super) fn lategame_lab_config() -> LabRoomConfig {
    let mut config = lab_config();
    config.scenario = Some("lategame".to_string());
    config
}

pub(super) fn summary_task(room: &str) -> RoomTask {
    let mut task = RoomTask::new(
        room.to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    task.created_at_unix_ms = 123_456;
    task.host_id = Some(1);
    add_test_room_player(&mut task, 1, false);
    task.assign_missing_team_for(1);
    task.assign_missing_faction_for(1);
    task
}

pub(super) fn add_branch_occupant(task: &mut RoomTask, id: u32) -> ConnectionWriter {
    let (msg_tx, writer) = ConnectionSink::new();
    task.order.push(id);
    task.players.insert(
        id,
        RoomPlayer {
            name: format!("Viewer {id}"),
            color: "#6f8fa8".to_string(),
            ready: true,
            spectator: true,
            msg_tx,
            head_of_line_count: 0,
            last_received_client_seq: 0,
            last_sim_consumed_client_seq: 0,
            last_sim_consumed_client_tick: None,
        },
    );
    task.reassign_host_if_needed();
    writer
}

pub(super) fn start_payloads(writer: &mut ConnectionWriter) -> Vec<StartPayload> {
    std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .filter_map(|msg| match msg {
            ServerMessage::Start(payload) => Some(payload),
            _ => None,
        })
        .collect()
}

pub(super) fn lab_results(writer: &mut ConnectionWriter) -> Vec<LabResult> {
    std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .filter_map(|msg| match msg {
            ServerMessage::LabResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

pub(super) fn room_time_states(writer: &mut ConnectionWriter) -> Vec<RoomTimeState> {
    std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .filter_map(|msg| match msg {
            ServerMessage::RoomTimeState(state) => Some(state),
            _ => None,
        })
        .collect()
}

pub(super) fn take_observer_analysis(
    writer: &ConnectionWriter,
    context: &str,
) -> ObserverAnalysisPayload {
    writer.observer_analysis.take().unwrap_or_else(|| {
        panic!("expected observer analysis for {context}");
    })
}

pub(super) fn branch_staging_messages(writer: &mut ConnectionWriter) -> Vec<ServerMessage> {
    std::iter::from_fn(|| writer.reliable_rx.try_recv().ok())
        .filter(|msg| matches!(msg, ServerMessage::BranchStaging { .. }))
        .collect()
}

pub(super) fn snapshot_notice_events(writer: &mut ConnectionWriter) -> Vec<Event> {
    writer
        .snapshots
        .take()
        .map(|snapshot| {
            snapshot
                .events
                .into_iter()
                .filter(|event| matches!(event, Event::Notice { .. }))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn assert_single_late_spectator_notice(
    writer: &mut ConnectionWriter,
    expected_msg: &str,
) {
    let notices = snapshot_notice_events(writer);
    assert_eq!(
        notices
            .iter()
            .filter(|event| matches!(event, Event::Notice { msg, .. } if msg == expected_msg))
            .count(),
        1,
        "expected exactly one notice {expected_msg:?}, got {notices:?}"
    );
    assert!(notices.iter().any(|event| matches!(
        event,
        Event::Notice {
            msg,
            severity: NoticeSeverity::Info,
            x: None,
            y: None
        } if msg == expected_msg
    )));
}

pub(super) fn assert_no_late_spectator_notice(writer: &mut ConnectionWriter, expected_msg: &str) {
    let notices = snapshot_notice_events(writer);
    assert!(
        !notices
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == expected_msg)),
        "unexpected notice {expected_msg:?}: {notices:?}"
    );
}

pub(super) fn replay_transition_test_snapshot(tick: u32) -> Snapshot {
    Snapshot {
        tick,
        steel: 75,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: Vec::new(),
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        trenches: Vec::new(),
        ability_objects: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: SnapshotNetStatus::default(),
    }
}

pub(super) fn in_game_tick(task: &RoomTask) -> u32 {
    match &task.phase {
        Phase::InGame(game) => game.tick_count(),
        Phase::ReplayViewer(session) => session.current_tick(),
        Phase::BranchStaging(staging) => staging.source_tick(),
        Phase::Lobby => 0,
    }
}

pub(super) fn lab_snapshot(task: &RoomTask) -> Snapshot {
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should be running");
    };
    game.snapshot_full_for(LAB_PLAYER_ONE_ID)
}

pub(super) fn lab_player_resources(task: &RoomTask, player_id: u32) -> (u32, u32) {
    let snapshot = lab_snapshot(task);
    let resources = snapshot
        .player_resources
        .iter()
        .find(|resources| resources.id == player_id)
        .expect("player resources");
    (resources.steel, resources.oil)
}

pub(super) fn lab_entity_position(task: &RoomTask, entity_id: u32) -> (f32, f32) {
    let snapshot = lab_snapshot(task);
    let entity = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == entity_id)
        .expect("entity should be visible in full lab snapshot");
    (entity.x, entity.y)
}

pub(super) fn lab_worker_id(task: &RoomTask) -> u32 {
    lab_snapshot(task)
        .entities
        .iter()
        .find(|entity| {
            entity.owner == LAB_PLAYER_ONE_ID && entity.kind == crate::protocol::kinds::WORKER
        })
        .expect("starting worker")
        .id
}

pub(super) fn lab_tile_center(task: &RoomTask, tile_x: u32, tile_y: u32) -> (f32, f32) {
    let Phase::InGame(game) = &task.phase else {
        panic!("lab should be running");
    };
    let map = game.start_payload().map;
    let tile_size = map.tile_size as f32;
    (
        (tile_x as f32 + 0.5) * tile_size,
        (tile_y as f32 + 0.5) * tile_size,
    )
}

pub(super) fn replay_start_payload_after(
    room: &str,
    action: impl FnOnce(&mut RoomTask),
) -> StartPayload {
    let players = replay_test_players(2);
    let (_live, artifact) = replay_test_artifact(&players, 4);
    let mut replay = ReplaySession::new(artifact).unwrap();
    for _ in 0..3 {
        replay.enqueue_for_current_tick().unwrap();
        replay.tick(None);
    }
    let mut task = RoomTask::new(
        room.to_string(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    );
    let mut writer = add_test_room_player(&mut task, 99, true);
    task.phase = Phase::ReplayViewer(Box::new(replay));

    action(&mut task);

    let mut payloads = start_payloads(&mut writer);
    assert_eq!(
        payloads.len(),
        1,
        "expected exactly one replay start payload"
    );
    payloads.remove(0)
}
