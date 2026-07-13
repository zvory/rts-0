use super::super::pending_build::{PendingBuildTracker, PENDING_BUILD_STALE_TICKS};
use super::super::player_view::PlayerView;
use crate::config;
use rts_sim::game::command::SimCommand as Command;
use rts_sim::game::entity::EntityKind;
use rts_sim::protocol::{
    kinds, states, terrain, EntityView, MapInfo, PlayerStart, Snapshot, StartPayload,
};

#[cfg(test)]
fn pending_tracker_start_payload() -> StartPayload {
    StartPayload {
        player_id: 1,
        spectator: false,
        prediction_build_id: None,
        prediction_version: 0,
        match_run_id: None,
        capabilities: Default::default(),
        diagnostics: Default::default(),
        replay: None,
        lab: None,
        tick: 0,
        map: MapInfo {
            width: 96,
            height: 96,
            tile_size: config::TILE_SIZE,
            terrain: vec![terrain::GRASS; 96 * 96],
            resources: Vec::new(),
        },
        players: vec![PlayerStart {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".into(),
            color: "#4cc9f0".into(),
            is_ai: true,
            start_tile_x: 10,
            start_tile_y: 85,
        }],
    }
}

#[cfg(test)]
fn pending_tracker_snapshot(tick: u32, worker_x: f32, worker_y: f32) -> Snapshot {
    Snapshot {
        tick,
        steel: 0,
        oil: 0,
        supply_used: 1,
        supply_cap: 10,
        entities: vec![EntityView::new(
            2,
            1,
            kinds::WORKER,
            worker_x,
            worker_y,
            40,
            40,
            states::BUILD,
        )],
        resource_deltas: Vec::new(),
        smokes: Vec::new(),
        ability_objects: Vec::new(),
        trenches: Vec::new(),
        visible_tiles: Vec::new(),
        remembered_buildings: Vec::new(),
        events: Vec::new(),
        upgrades: Vec::new(),
        player_resources: Vec::new(),
        net_status: rts_sim::protocol::SnapshotNetStatus::default(),
    }
}

#[cfg(test)]
fn pending_tracker_view<'a>(
    tick: u32,
    start: &'a StartPayload,
    snapshot: &'a Snapshot,
) -> PlayerView<'a> {
    PlayerView {
        player_id: 1,
        tick,
        start,
        snapshot,
        alive_player_ids: &[1],
    }
}

#[test]
fn pending_build_tracker_keeps_moving_worker_past_stale_window() {
    let start = pending_tracker_start_payload();
    let mut tracker = PendingBuildTracker::default();
    tracker.record_commands(
        10,
        &[Command::Build {
            units: vec![2],
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
            queued: false,
        }],
    );

    for tick in [70, 130, 190, 250, 310, 370, 430] {
        let snapshot = pending_tracker_snapshot(tick, 100.0 + tick as f32, 200.0);
        tracker.observe(pending_tracker_view(tick, &start, &snapshot));
        assert_eq!(
            tracker.intents().len(),
            1,
            "moving expansion builder should remain reserved at tick {tick}"
        );
    }
}

#[test]
fn pending_build_tracker_expires_stuck_worker() {
    let start = pending_tracker_start_payload();
    let mut tracker = PendingBuildTracker::default();
    tracker.record_commands(
        10,
        &[Command::Build {
            units: vec![2],
            building: EntityKind::CityCentre,
            tile_x: 48,
            tile_y: 70,
            queued: false,
        }],
    );

    let first_snapshot = pending_tracker_snapshot(20, 100.0, 200.0);
    tracker.observe(pending_tracker_view(20, &start, &first_snapshot));
    let stale_tick = 20 + PENDING_BUILD_STALE_TICKS;
    let stale_snapshot = pending_tracker_snapshot(stale_tick, 100.0, 200.0);
    tracker.observe(pending_tracker_view(stale_tick, &start, &stale_snapshot));

    assert!(tracker.intents().is_empty());
    assert!(tracker.failed(EntityKind::CityCentre, 48, 70));
}
