use super::connection::{send_or_log, SnapshotSendStatus};
use super::projection::{observer_view_or_all, ProjectionPolicy};
use super::replay_session::ReplaySession;
use super::room_task::RoomPlayer;
use super::snapshots::compact_snapshot_for_wire;
use super::snapshots::union_events;
use crate::protocol::{
    Event, ServerMessage, Snapshot, SnapshotNetStatus, PREDICTION_PROTOCOL_VERSION,
};
use rts_sim::game::ObserverView;
use std::collections::HashMap;
use std::time::{Duration, Instant as StdInstant};

pub(super) struct SnapshotFanout<'a> {
    room: &'a str,
    scheduler_lag: Duration,
    tick_budget: Duration,
    tick_start: StdInstant,
    slow_tick_count: &'a mut u32,
    perf: Option<&'a mut rts_sim::perf::TickPerf>,
}

pub(super) struct SnapshotFanoutPayload {
    snapshot: Snapshot,
    spectator: bool,
}

impl SnapshotFanoutPayload {
    pub(super) fn new(snapshot: Snapshot, spectator: bool) -> Self {
        Self {
            snapshot,
            spectator,
        }
    }
}

impl<'a> SnapshotFanout<'a> {
    pub(super) fn new(
        room: &'a str,
        scheduler_lag: Duration,
        tick_budget: Duration,
        tick_start: StdInstant,
        slow_tick_count: &'a mut u32,
        perf: Option<&'a mut rts_sim::perf::TickPerf>,
    ) -> Self {
        Self {
            room,
            scheduler_lag,
            tick_budget,
            tick_start,
            slow_tick_count,
            perf,
        }
    }

    pub(super) fn send_to_recipients(
        &mut self,
        players: &mut HashMap<u32, RoomPlayer>,
        recipients: impl IntoIterator<Item = u32>,
        mut snapshot_for: impl FnMut(u32, &RoomPlayer) -> Option<SnapshotFanoutPayload>,
    ) -> Vec<u32> {
        let mut slow_tick_counted = false;
        let fanout_start = StdInstant::now();
        let mut delivered_recipients = Vec::new();

        for id in recipients {
            let Some(player) = players.get(&id) else {
                continue;
            };

            let snapshot_start = StdInstant::now();
            let Some(payload) = snapshot_for(id, player) else {
                continue;
            };
            let mut snapshot = payload.snapshot;
            let snapshot_duration = snapshot_start.elapsed();
            let entity_count = snapshot.entities.len();
            let resource_delta_count = snapshot.resource_deltas.len();
            let event_count = snapshot.events.len();
            let tick_elapsed = self.tick_start.elapsed();
            let slow_tick =
                self.scheduler_lag >= self.tick_budget || tick_elapsed >= self.tick_budget;
            if slow_tick && !slow_tick_counted {
                *self.slow_tick_count = self.slow_tick_count.saturating_add(1);
                slow_tick_counted = true;
            }
            snapshot.net_status = snapshot_net_status(
                player,
                self.scheduler_lag,
                tick_elapsed,
                slow_tick,
                *self.slow_tick_count,
            );
            let compact_start = StdInstant::now();
            compact_snapshot_for_wire(&mut snapshot);
            let compact_duration = compact_start.elapsed();
            player.msg_tx.record_snapshot_projected(
                saturating_duration_ms_u32(snapshot_duration),
                saturating_duration_ms_u32(compact_duration),
            );

            if let Some(perf) = self.perf.as_mut() {
                perf.record_snapshot(rts_sim::perf::SnapshotRecord {
                    player_id: id,
                    spectator: payload.spectator,
                    snapshot: snapshot_duration,
                    compact: compact_duration,
                    entities: entity_count,
                    resource_deltas: resource_delta_count,
                    events: event_count,
                });
            }

            let enqueue_status = send_or_log(
                self.room,
                id,
                &player.msg_tx,
                ServerMessage::Snapshot(snapshot),
            );
            if matches!(
                enqueue_status,
                Some(SnapshotSendStatus::Stored | SnapshotSendStatus::Replaced)
            ) {
                delivered_recipients.push(id);
            }
            if matches!(enqueue_status, Some(SnapshotSendStatus::Replaced)) {
                if let Some(player) = players.get_mut(&id) {
                    player.head_of_line_count = player.head_of_line_count.saturating_add(1);
                }
            }
            if let (Some(perf), Some(status)) = (self.perf.as_mut(), enqueue_status) {
                perf.record_enqueue(snapshot_enqueue_status(status));
            }
        }

        if let Some(perf) = self.perf.as_mut() {
            perf.record_phase("snapshot_fanout", fanout_start.elapsed());
        }
        delivered_recipients
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fanout_replay_snapshots(
    room: &str,
    players: &mut HashMap<u32, RoomPlayer>,
    observer_views: &HashMap<u32, ObserverView>,
    projection_policy: ProjectionPolicy,
    session: &ReplaySession,
    recipients: impl IntoIterator<Item = u32>,
    per_player_events: &mut HashMap<u32, Vec<Event>>,
    scheduler_lag: Duration,
    tick_budget: Duration,
    tick_start: StdInstant,
    slow_tick_count: &mut u32,
    perf: Option<&mut rts_sim::perf::TickPerf>,
) {
    let full_vision_events = union_events(per_player_events.values());
    SnapshotFanout::new(
        room,
        scheduler_lag,
        tick_budget,
        tick_start,
        slow_tick_count,
        perf,
    )
    .send_to_recipients(players, recipients, |id, _player| {
        let projection = projection_policy.selected_perspective_snapshot_for(observer_view_or_all(
            observer_views.get(&id),
            session.game(),
        ));
        let snapshot =
            projection.snapshot_with_events(session.game(), per_player_events, &full_vision_events);
        Some(SnapshotFanoutPayload::new(snapshot, true))
    });
}

fn snapshot_net_status(
    player: &RoomPlayer,
    scheduler_lag: Duration,
    tick_elapsed: Duration,
    slow_tick: bool,
    slow_tick_count: u32,
) -> SnapshotNetStatus {
    let head_of_line = player.msg_tx.has_pending_snapshot();
    let include_prediction_ack = !player.spectator;
    SnapshotNetStatus {
        server_lag_ms: saturating_duration_ms_u16(scheduler_lag),
        tick_ms: saturating_duration_ms_u16(tick_elapsed),
        slow_tick,
        slow_tick_count,
        head_of_line,
        head_of_line_count: player
            .head_of_line_count
            .saturating_add(u32::from(head_of_line)),
        prediction_version: if include_prediction_ack {
            PREDICTION_PROTOCOL_VERSION
        } else {
            0
        },
        last_sim_consumed_client_seq: if include_prediction_ack {
            player.last_sim_consumed_client_seq
        } else {
            0
        },
        last_sim_consumed_client_tick: if include_prediction_ack {
            player.last_sim_consumed_client_tick
        } else {
            None
        },
    }
}

fn snapshot_enqueue_status(status: SnapshotSendStatus) -> rts_sim::perf::SnapshotEnqueue {
    match status {
        SnapshotSendStatus::Stored => rts_sim::perf::SnapshotEnqueue::Stored,
        SnapshotSendStatus::Replaced => rts_sim::perf::SnapshotEnqueue::Replaced,
        SnapshotSendStatus::Closed => rts_sim::perf::SnapshotEnqueue::Closed,
    }
}

fn saturating_duration_ms_u16(duration: Duration) -> u16 {
    duration.as_millis().min(u16::MAX as u128) as u16
}

fn saturating_duration_ms_u32(duration: Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}
