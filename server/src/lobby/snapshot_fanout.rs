use super::connection::{send_or_log, SnapshotSendStatus};
use super::room_task::RoomPlayer;
use super::snapshots::compact_snapshot_for_wire;
use crate::protocol::{ServerMessage, Snapshot, SnapshotNetStatus, PREDICTION_PROTOCOL_VERSION};
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
    ) {
        let mut slow_tick_counted = false;
        let fanout_start = StdInstant::now();

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
    }
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
