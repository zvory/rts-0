#![cfg_attr(test, allow(dead_code))]

use super::*;
use crate::protocol::{ObserverAnalysisPayload, SnapshotPayloadDiagnostics};
use std::collections::{BTreeMap, VecDeque};
use std::time::Instant as StdInstant;

/// Outbound connection handle shared with the room task. Reliable messages keep FIFO ordering;
/// snapshots and observer analysis use separate latest-only slots because older unsent live-state
/// messages are superseded by newer full-state messages.
#[derive(Clone)]
pub struct ConnectionSink {
    reliable_tx: mpsc::Sender<ServerMessage>,
    snapshots: Arc<LatestSnapshotSlot>,
    observer_analysis: Arc<LatestObserverAnalysisSlot>,
    stats: Arc<ConnectionReportCounters>,
}

impl std::fmt::Debug for ConnectionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionSink").finish_non_exhaustive()
    }
}

pub struct ConnectionWriter {
    pub reliable_rx: mpsc::Receiver<ServerMessage>,
    pub snapshots: Arc<LatestSnapshotSlot>,
    pub observer_analysis: Arc<LatestObserverAnalysisSlot>,
    stats: Arc<ConnectionReportCounters>,
}

pub struct LatestSnapshotSlot {
    pending: StdMutex<Option<PendingSnapshot>>,
    notify: Notify,
}

pub(crate) struct PendingSnapshot {
    snapshot: Snapshot,
    enqueued_at: StdInstant,
}

pub struct LatestObserverAnalysisSlot {
    pending: StdMutex<Option<ObserverAnalysisPayload>>,
    notify: Notify,
}

pub struct ConnectionSnapshotSend {
    snapshot: Snapshot,
    stats: SnapshotSendStats,
}

#[derive(Clone, Copy)]
pub struct SnapshotSendStats {
    age_ms: u32,
    waited_behind_reliable: bool,
}

pub struct SnapshotWriterSendStats {
    pub serialize_ms: u32,
    pub send_ms: u32,
    pub bytes: u32,
    pub payload: SnapshotPayloadDiagnostics,
}

pub struct ConnectionWriterStats {
    counters: Arc<ConnectionReportCounters>,
    reliable_drained_before_snapshot: u32,
}

const COMMAND_LIFECYCLE_TOP_N: usize = 5;
const SNAPSHOT_PAYLOAD_TOP_N: usize = 8;
const COMMAND_LIFECYCLE_BUCKETS_MS: [u32; 16] = [
    1, 2, 4, 8, 12, 16, 24, 33, 50, 75, 100, 150, 250, 500, 1_000, 2_000,
];
const COMMAND_LIFECYCLE_BUCKET_COUNT: usize = COMMAND_LIFECYCLE_BUCKETS_MS.len() + 1;
const SNAPSHOT_LIFECYCLE_BUCKETS_MS: [u32; 16] = COMMAND_LIFECYCLE_BUCKETS_MS;
const SNAPSHOT_PAYLOAD_BUCKETS_BYTES: [u32; 25] = [
    512, 768, 1_024, 1_280, 1_536, 2_048, 3_072, 4_096, 6_144, 8_192, 12_288, 16_384, 24_576,
    32_768, 49_152, 65_536, 98_304, 131_072, 196_608, 262_144, 393_216, 524_288, 786_432,
    1_048_576, 1_572_864,
];

#[derive(Debug, Clone, Default)]
pub struct ConnectionReportStats {
    pub command_receipts_accepted: u32,
    pub command_receipts_rejected: u32,
    pub reliable_drained_before_snapshot: u32,
    pub reliable_drained_before_snapshot_max: u32,
    pub snapshot_waited_behind_reliable: u32,
    pub snapshot_sent: u32,
    pub snapshot_send_age_latest_ms: u32,
    pub snapshot_send_age_max_ms: u32,
    pub snapshot_send_age_avg_ms: u32,
    pub snapshot_slot_stored: u32,
    pub snapshot_slot_replaced: u32,
    pub snapshot_slot_closed: u32,
    pub snapshot_lifecycle: SnapshotLifecycleReportStats,
    pub command_lifecycle: CommandLifecycleReportStats,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotLifecycleReportStats {
    pub projected: SnapshotWindowStats,
    pub compacted: SnapshotWindowStats,
    pub queue_age: SnapshotWindowStats,
    pub serialized: SnapshotWindowStats,
    pub writer_send: SnapshotWindowStats,
    pub payload_bytes: SnapshotWindowStats,
    pub writer_taken: u32,
    pub sections: Vec<SnapshotPayloadSectionReportStats>,
    pub entity_kinds: Vec<SnapshotPayloadEntityKindReportStats>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotWindowStats {
    pub latest: u32,
    pub max: u32,
    pub p95: u32,
    pub avg: u32,
    pub total: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotPayloadSectionReportStats {
    pub section: String,
    pub count: u32,
    pub bytes: u32,
    pub pct_x100: u16,
}

#[derive(Debug, Clone, Default)]
pub struct SnapshotPayloadEntityKindReportStats {
    pub kind: String,
    pub count: u32,
    pub approx_bytes: u32,
    pub pct_x100: u16,
}

#[derive(Debug, Clone, Default)]
pub struct CommandLifecycleReportStats {
    pub count: u32,
    pub accepted: u32,
    pub rejected: u32,
    pub frame_deserialize: CommandTimingStats,
    pub deserialize_to_room_enqueue: CommandTimingStats,
    pub room_queue: CommandTimingStats,
    pub room_handle: CommandTimingStats,
    pub receipt_send_age: CommandTimingStats,
    pub accepted_to_sim_ack: CommandTimingStats,
    pub exemplars: Vec<CommandLifecycleExemplarStats>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CommandTimingStats {
    pub latest_ms: u32,
    pub max_ms: u32,
    pub p95_ms: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct CommandLifecycleExemplarStats {
    pub received_unix_ms: u64,
    pub client_seq: u32,
    pub family: String,
    pub stage: String,
    pub stage_ms: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct CommandLifecycleSample {
    pub received_unix_ms: u64,
    pub client_seq: u32,
    pub family: &'static str,
    pub accepted: bool,
    pub frame_deserialize_ms: u32,
    pub deserialize_to_room_enqueue_ms: u32,
    pub room_queue_ms: u32,
    pub room_handle_ms: u32,
}

impl CommandLifecycleSample {
    pub(crate) fn from_timing(
        client_seq: u32,
        lifecycle: CommandLifecycleTiming,
        room_handle_started_at: StdInstant,
        accepted: bool,
    ) -> Self {
        Self {
            received_unix_ms: lifecycle.received_unix_ms,
            client_seq,
            family: lifecycle.family.as_str(),
            accepted,
            frame_deserialize_ms: duration_ms_u32(
                lifecycle
                    .deserialized_at
                    .saturating_duration_since(lifecycle.frame_received_at),
            ),
            deserialize_to_room_enqueue_ms: duration_ms_u32(
                lifecycle
                    .room_event_enqueued_at
                    .saturating_duration_since(lifecycle.deserialized_at),
            ),
            room_queue_ms: duration_ms_u32(
                room_handle_started_at.saturating_duration_since(lifecycle.room_event_enqueued_at),
            ),
            room_handle_ms: duration_ms_u32(room_handle_started_at.elapsed()),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct CommandSimAckSample {
    pub received_unix_ms: u64,
    pub client_seq: u32,
    pub family: &'static str,
    pub accepted_to_sim_ack_ms: u32,
}

fn duration_ms_u32(duration: std::time::Duration) -> u32 {
    duration.as_millis().min(u32::MAX as u128) as u32
}

#[derive(Default)]
struct CommandLifecycleWindow {
    count: u32,
    accepted: u32,
    rejected: u32,
    frame_deserialize: CommandTimingWindow,
    deserialize_to_room_enqueue: CommandTimingWindow,
    room_queue: CommandTimingWindow,
    room_handle: CommandTimingWindow,
    receipt_send_age: CommandTimingWindow,
    accepted_to_sim_ack: CommandTimingWindow,
    exemplars: Vec<CommandLifecycleExemplarStats>,
}

#[derive(Default)]
struct CommandTimingWindow {
    latest_ms: u32,
    max_ms: u32,
    count: u32,
    bucket_counts: [u32; COMMAND_LIFECYCLE_BUCKET_COUNT],
}

#[derive(Default)]
pub(crate) struct ConnectionReportCounters {
    command_receipts_accepted: AtomicU32,
    command_receipts_rejected: AtomicU32,
    reliable_drained_before_snapshot: AtomicU32,
    reliable_drained_before_snapshot_max: AtomicU32,
    snapshot_waited_behind_reliable: AtomicU32,
    snapshot_sent: AtomicU32,
    snapshot_send_age_latest_ms: AtomicU32,
    snapshot_send_age_max_ms: AtomicU32,
    snapshot_send_age_total_ms: AtomicU64,
    snapshot_slot_stored: AtomicU32,
    snapshot_slot_replaced: AtomicU32,
    snapshot_slot_closed: AtomicU32,
    command_receipt_queued_at: StdMutex<VecDeque<StdInstant>>,
    snapshot_lifecycle: StdMutex<SnapshotLifecycleWindow>,
    command_lifecycle: StdMutex<CommandLifecycleWindow>,
}

#[derive(Default)]
struct SnapshotLifecycleWindow {
    projected: SnapshotValueWindow,
    compacted: SnapshotValueWindow,
    queue_age: SnapshotValueWindow,
    serialized: SnapshotValueWindow,
    writer_send: SnapshotValueWindow,
    payload_bytes: SnapshotValueWindow,
    writer_taken: u32,
    sections: BTreeMap<&'static str, SnapshotPayloadTotals>,
    entity_kinds: BTreeMap<String, SnapshotPayloadTotals>,
}

#[derive(Default)]
struct SnapshotValueWindow {
    latest: u32,
    max: u32,
    total: u64,
    count: u32,
    bucket_counts: Vec<u32>,
}

#[derive(Default)]
struct SnapshotPayloadTotals {
    count: u64,
    bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotSendStatus {
    Stored,
    Replaced,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LatestOnlySendStatus {
    Stored,
    Replaced,
    Closed,
}

impl ConnectionWriter {
    pub fn into_parts(
        self,
    ) -> (
        mpsc::Receiver<ServerMessage>,
        Arc<LatestSnapshotSlot>,
        Arc<LatestObserverAnalysisSlot>,
        ConnectionWriterStats,
    ) {
        (
            self.reliable_rx,
            self.snapshots,
            self.observer_analysis,
            ConnectionWriterStats::new(self.stats),
        )
    }
}

impl ConnectionSink {
    pub fn new() -> (Self, ConnectionWriter) {
        let (reliable_tx, reliable_rx) = mpsc::channel(PLAYER_RELIABLE_CHANNEL_CAP);
        let stats = Arc::new(ConnectionReportCounters::default());
        let snapshots = Arc::new(LatestSnapshotSlot {
            pending: StdMutex::new(None),
            notify: Notify::new(),
        });
        let observer_analysis = Arc::new(LatestObserverAnalysisSlot {
            pending: StdMutex::new(None),
            notify: Notify::new(),
        });
        (
            ConnectionSink {
                reliable_tx,
                snapshots: snapshots.clone(),
                observer_analysis: observer_analysis.clone(),
                stats: stats.clone(),
            },
            ConnectionWriter {
                reliable_rx,
                snapshots,
                observer_analysis,
                stats,
            },
        )
    }

    pub async fn send_reliable(
        &self,
        msg: ServerMessage,
    ) -> Result<(), mpsc::error::SendError<ServerMessage>> {
        let receipt = command_receipt_accepted(&msg);
        match self.reliable_tx.send(msg).await {
            Ok(()) => {
                self.record_command_receipt_queued(receipt);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    #[allow(clippy::result_large_err)]
    pub fn try_send_reliable(
        &self,
        msg: ServerMessage,
    ) -> Result<(), mpsc::error::TrySendError<ServerMessage>> {
        let receipt = command_receipt_accepted(&msg);
        match self.reliable_tx.try_send(msg) {
            Ok(()) => {
                self.record_command_receipt_queued(receipt);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub(crate) fn try_send_snapshot(&self, mut snapshot: Snapshot) -> SnapshotSendStatus {
        if self.reliable_tx.is_closed() {
            self.stats
                .snapshot_slot_closed
                .fetch_add(1, Ordering::Relaxed);
            return SnapshotSendStatus::Closed;
        }

        let mut pending = self.snapshots.lock_pending();
        let replaced = if let Some(previous) = pending.as_ref() {
            merge_resource_deltas(&mut snapshot, &previous.snapshot.resource_deltas);
            true
        } else {
            false
        };
        *pending = Some(PendingSnapshot {
            snapshot,
            enqueued_at: StdInstant::now(),
        });
        drop(pending);

        self.snapshots.notify.notify_one();
        if replaced {
            self.stats
                .snapshot_slot_replaced
                .fetch_add(1, Ordering::Relaxed);
            SnapshotSendStatus::Replaced
        } else {
            self.stats
                .snapshot_slot_stored
                .fetch_add(1, Ordering::Relaxed);
            SnapshotSendStatus::Stored
        }
    }

    pub(crate) fn try_send_observer_analysis(
        &self,
        payload: ObserverAnalysisPayload,
    ) -> LatestOnlySendStatus {
        if self.reliable_tx.is_closed() {
            return LatestOnlySendStatus::Closed;
        }
        let replaced = self.observer_analysis.store(payload);
        if replaced {
            LatestOnlySendStatus::Replaced
        } else {
            LatestOnlySendStatus::Stored
        }
    }

    pub(crate) fn has_pending_snapshot(&self) -> bool {
        self.snapshots.has_pending()
    }

    pub(crate) fn clear_pending_snapshot(&self) {
        self.snapshots.clear();
        self.observer_analysis.clear();
    }

    pub fn consume_report_stats(&self) -> ConnectionReportStats {
        self.stats.consume()
    }

    pub(crate) fn record_command_lifecycle(&self, sample: CommandLifecycleSample) {
        self.stats.record_command_lifecycle(sample);
    }

    pub(crate) fn record_command_sim_ack(&self, sample: CommandSimAckSample) {
        self.stats.record_command_sim_ack(sample);
    }

    pub(crate) fn record_snapshot_projected(&self, projected_ms: u32, compacted_ms: u32) {
        self.stats
            .record_snapshot_projected(projected_ms, compacted_ms);
    }

    fn record_command_receipt_queued(&self, accepted: Option<bool>) {
        match accepted {
            Some(true) => {
                self.stats
                    .command_receipts_accepted
                    .fetch_add(1, Ordering::Relaxed);
                self.stats.record_command_receipt_queued_at();
            }
            Some(false) => {
                self.stats
                    .command_receipts_rejected
                    .fetch_add(1, Ordering::Relaxed);
                self.stats.record_command_receipt_queued_at();
            }
            None => {}
        }
    }
}

fn command_receipt_accepted(msg: &ServerMessage) -> Option<bool> {
    if let ServerMessage::CommandReceipt { accepted, .. } = msg {
        Some(*accepted)
    } else {
        None
    }
}

impl LatestSnapshotSlot {
    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<PendingSnapshot>> {
        match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn take(&self) -> Option<Snapshot> {
        self.lock_pending().take().map(|pending| pending.snapshot)
    }

    pub fn take_for_send(
        &self,
        writer_stats: &mut ConnectionWriterStats,
    ) -> Option<ConnectionSnapshotSend> {
        self.lock_pending()
            .take()
            .map(|pending| writer_stats.prepare_snapshot_send(pending))
    }

    pub fn has_pending(&self) -> bool {
        self.lock_pending().is_some()
    }

    fn clear(&self) {
        self.lock_pending().take();
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

impl LatestObserverAnalysisSlot {
    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<ObserverAnalysisPayload>> {
        match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn take(&self) -> Option<ObserverAnalysisPayload> {
        self.lock_pending().take()
    }

    fn clear(&self) {
        self.lock_pending().take();
    }

    fn store(&self, payload: ObserverAnalysisPayload) -> bool {
        let mut pending = self.lock_pending();
        let replaced = pending.is_some();
        *pending = Some(payload);
        drop(pending);
        self.notify.notify_one();
        replaced
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

impl PendingSnapshot {
    fn into_snapshot(self) -> Snapshot {
        self.snapshot
    }

    fn age_ms(&self) -> u32 {
        self.enqueued_at.elapsed().as_millis().min(u32::MAX as u128) as u32
    }
}

impl ConnectionSnapshotSend {
    pub fn into_parts(self) -> (Snapshot, SnapshotSendStats) {
        (self.snapshot, self.stats)
    }
}

impl ConnectionWriterStats {
    fn new(counters: Arc<ConnectionReportCounters>) -> Self {
        Self {
            counters,
            reliable_drained_before_snapshot: 0,
        }
    }

    pub fn note_reliable_message(&mut self, snapshot_pending: bool) {
        if snapshot_pending {
            self.reliable_drained_before_snapshot =
                self.reliable_drained_before_snapshot.saturating_add(1);
        }
    }

    pub fn note_reliable_for_snapshot(
        &mut self,
        was_pending: bool,
        snapshots: &LatestSnapshotSlot,
    ) {
        self.note_reliable_message(was_pending || snapshots.has_pending());
    }

    fn prepare_snapshot_send(&mut self, pending: PendingSnapshot) -> ConnectionSnapshotSend {
        self.counters
            .record_reliable_drained_before_snapshot(self.reliable_drained_before_snapshot);
        let age_ms = pending.age_ms();
        self.counters.record_snapshot_taken(age_ms);
        let stats = SnapshotSendStats {
            age_ms,
            waited_behind_reliable: self.reliable_drained_before_snapshot > 0,
        };
        self.reliable_drained_before_snapshot = 0;
        ConnectionSnapshotSend {
            snapshot: pending.into_snapshot(),
            stats,
        }
    }

    pub fn record_snapshot_sent(
        &self,
        stats: SnapshotSendStats,
        writer_stats: SnapshotWriterSendStats,
    ) {
        self.counters
            .record_snapshot_sent(stats.age_ms, stats.waited_behind_reliable);
        self.counters.record_snapshot_written(writer_stats);
    }

    pub fn record_reliable_sent(&self, command_receipt: bool) {
        self.counters.record_reliable_sent(command_receipt);
    }
}

impl ConnectionReportCounters {
    pub(crate) fn record_reliable_drained_before_snapshot(&self, count: u32) {
        if count == 0 {
            return;
        }
        self.reliable_drained_before_snapshot
            .fetch_add(count, Ordering::Relaxed);
        fetch_max(&self.reliable_drained_before_snapshot_max, count);
    }

    pub(crate) fn record_snapshot_sent(&self, age_ms: u32, waited_behind_reliable: bool) {
        self.snapshot_sent.fetch_add(1, Ordering::Relaxed);
        self.snapshot_send_age_latest_ms
            .store(age_ms, Ordering::Relaxed);
        fetch_max(&self.snapshot_send_age_max_ms, age_ms);
        self.snapshot_send_age_total_ms
            .fetch_add(age_ms as u64, Ordering::Relaxed);
        if waited_behind_reliable {
            self.snapshot_waited_behind_reliable
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn record_snapshot_projected(&self, projected_ms: u32, compacted_ms: u32) {
        self.with_snapshot_lifecycle(|window| {
            window
                .projected
                .add(projected_ms, &SNAPSHOT_LIFECYCLE_BUCKETS_MS);
            window
                .compacted
                .add(compacted_ms, &SNAPSHOT_LIFECYCLE_BUCKETS_MS);
        });
    }

    pub(crate) fn record_snapshot_taken(&self, queue_age_ms: u32) {
        self.with_snapshot_lifecycle(|window| {
            window.writer_taken = window.writer_taken.saturating_add(1);
            window
                .queue_age
                .add(queue_age_ms, &SNAPSHOT_LIFECYCLE_BUCKETS_MS);
        });
    }

    pub(crate) fn record_snapshot_written(&self, stats: SnapshotWriterSendStats) {
        self.with_snapshot_lifecycle(|window| {
            window
                .serialized
                .add(stats.serialize_ms, &SNAPSHOT_LIFECYCLE_BUCKETS_MS);
            window
                .writer_send
                .add(stats.send_ms, &SNAPSHOT_LIFECYCLE_BUCKETS_MS);
            window
                .payload_bytes
                .add(stats.bytes, &SNAPSHOT_PAYLOAD_BUCKETS_BYTES);
            window.add_payload(stats.payload);
        });
    }

    pub(crate) fn record_command_receipt_queued_at(&self) {
        match self.command_receipt_queued_at.lock() {
            Ok(mut guard) => guard.push_back(StdInstant::now()),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.push_back(StdInstant::now());
            }
        }
    }

    pub(crate) fn record_reliable_sent(&self, command_receipt: bool) {
        if command_receipt {
            let queued_at = match self.command_receipt_queued_at.lock() {
                Ok(mut guard) => guard.pop_front(),
                Err(poisoned) => {
                    let mut guard = poisoned.into_inner();
                    guard.pop_front()
                }
            };
            if let Some(queued_at) = queued_at {
                self.with_command_lifecycle(|window| {
                    window
                        .receipt_send_age
                        .add(duration_since_ms_u32(queued_at));
                });
            }
        }
    }

    pub(crate) fn record_command_lifecycle(&self, sample: CommandLifecycleSample) {
        self.with_command_lifecycle(|window| {
            window.count = window.count.saturating_add(1);
            if sample.accepted {
                window.accepted = window.accepted.saturating_add(1);
            } else {
                window.rejected = window.rejected.saturating_add(1);
            }
            window.frame_deserialize.add(sample.frame_deserialize_ms);
            window
                .deserialize_to_room_enqueue
                .add(sample.deserialize_to_room_enqueue_ms);
            window.room_queue.add(sample.room_queue_ms);
            window.room_handle.add(sample.room_handle_ms);
            window.add_exemplar(
                sample.received_unix_ms,
                sample.client_seq,
                sample.family,
                [
                    ("serverFrameDeserialize", sample.frame_deserialize_ms),
                    (
                        "serverDeserializeToRoomEnqueue",
                        sample.deserialize_to_room_enqueue_ms,
                    ),
                    ("serverRoomQueue", sample.room_queue_ms),
                    ("serverRoomHandle", sample.room_handle_ms),
                ],
            );
        });
    }

    pub(crate) fn record_command_sim_ack(&self, sample: CommandSimAckSample) {
        self.with_command_lifecycle(|window| {
            window
                .accepted_to_sim_ack
                .add(sample.accepted_to_sim_ack_ms);
            window.add_exemplar(
                sample.received_unix_ms,
                sample.client_seq,
                sample.family,
                [("serverAcceptedToSimAck", sample.accepted_to_sim_ack_ms)],
            );
        });
    }

    fn consume(&self) -> ConnectionReportStats {
        let snapshot_sent = self.snapshot_sent.swap(0, Ordering::Relaxed);
        let snapshot_send_age_total_ms = self.snapshot_send_age_total_ms.swap(0, Ordering::Relaxed);
        let snapshot_lifecycle = self.consume_snapshot_lifecycle();
        let command_lifecycle = self.consume_command_lifecycle();
        ConnectionReportStats {
            command_receipts_accepted: self.command_receipts_accepted.swap(0, Ordering::Relaxed),
            command_receipts_rejected: self.command_receipts_rejected.swap(0, Ordering::Relaxed),
            reliable_drained_before_snapshot: self
                .reliable_drained_before_snapshot
                .swap(0, Ordering::Relaxed),
            reliable_drained_before_snapshot_max: self
                .reliable_drained_before_snapshot_max
                .swap(0, Ordering::Relaxed),
            snapshot_waited_behind_reliable: self
                .snapshot_waited_behind_reliable
                .swap(0, Ordering::Relaxed),
            snapshot_sent,
            snapshot_send_age_latest_ms: self
                .snapshot_send_age_latest_ms
                .swap(0, Ordering::Relaxed),
            snapshot_send_age_max_ms: self.snapshot_send_age_max_ms.swap(0, Ordering::Relaxed),
            snapshot_send_age_avg_ms: if snapshot_sent > 0 {
                (snapshot_send_age_total_ms / snapshot_sent as u64).min(u32::MAX as u64) as u32
            } else {
                0
            },
            snapshot_slot_stored: self.snapshot_slot_stored.swap(0, Ordering::Relaxed),
            snapshot_slot_replaced: self.snapshot_slot_replaced.swap(0, Ordering::Relaxed),
            snapshot_slot_closed: self.snapshot_slot_closed.swap(0, Ordering::Relaxed),
            snapshot_lifecycle,
            command_lifecycle,
        }
    }

    fn with_snapshot_lifecycle(&self, f: impl FnOnce(&mut SnapshotLifecycleWindow)) {
        match self.snapshot_lifecycle.lock() {
            Ok(mut guard) => f(&mut guard),
            Err(poisoned) => f(&mut poisoned.into_inner()),
        }
    }

    fn consume_snapshot_lifecycle(&self) -> SnapshotLifecycleReportStats {
        match self.snapshot_lifecycle.lock() {
            Ok(mut guard) => guard.consume(),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.consume()
            }
        }
    }

    fn with_command_lifecycle(&self, f: impl FnOnce(&mut CommandLifecycleWindow)) {
        match self.command_lifecycle.lock() {
            Ok(mut guard) => f(&mut guard),
            Err(poisoned) => f(&mut poisoned.into_inner()),
        }
    }

    fn consume_command_lifecycle(&self) -> CommandLifecycleReportStats {
        match self.command_lifecycle.lock() {
            Ok(mut guard) => guard.consume(),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.consume()
            }
        }
    }
}

impl SnapshotLifecycleWindow {
    fn add_payload(&mut self, payload: SnapshotPayloadDiagnostics) {
        for section in payload.sections {
            let entry = self.sections.entry(section.section).or_default();
            entry.count = entry.count.saturating_add(section.count as u64);
            entry.bytes = entry.bytes.saturating_add(section.bytes as u64);
        }
        for kind in payload.entity_kinds {
            let entry = self.entity_kinds.entry(kind.kind).or_default();
            entry.count = entry.count.saturating_add(kind.count as u64);
            entry.bytes = entry.bytes.saturating_add(kind.approx_bytes as u64);
        }
    }

    fn consume(&mut self) -> SnapshotLifecycleReportStats {
        let payload_total_bytes = self.payload_bytes.total;
        SnapshotLifecycleReportStats {
            projected: self.projected.consume(&SNAPSHOT_LIFECYCLE_BUCKETS_MS),
            compacted: self.compacted.consume(&SNAPSHOT_LIFECYCLE_BUCKETS_MS),
            queue_age: self.queue_age.consume(&SNAPSHOT_LIFECYCLE_BUCKETS_MS),
            serialized: self.serialized.consume(&SNAPSHOT_LIFECYCLE_BUCKETS_MS),
            writer_send: self.writer_send.consume(&SNAPSHOT_LIFECYCLE_BUCKETS_MS),
            payload_bytes: self.payload_bytes.consume(&SNAPSHOT_PAYLOAD_BUCKETS_BYTES),
            writer_taken: std::mem::take(&mut self.writer_taken),
            sections: consume_payload_sections(&mut self.sections, payload_total_bytes),
            entity_kinds: consume_entity_kinds(&mut self.entity_kinds, payload_total_bytes),
        }
    }
}

impl SnapshotValueWindow {
    fn add(&mut self, value: u32, buckets: &[u32]) {
        if self.bucket_counts.len() != buckets.len() + 1 {
            self.bucket_counts = vec![0; buckets.len() + 1];
        }
        self.latest = value;
        self.max = self.max.max(value);
        self.total = self.total.saturating_add(value as u64);
        self.count = self.count.saturating_add(1);
        let index = buckets
            .iter()
            .position(|bucket| value <= *bucket)
            .unwrap_or(buckets.len());
        if let Some(count) = self.bucket_counts.get_mut(index) {
            *count = count.saturating_add(1);
        }
    }

    fn consume(&mut self, buckets: &[u32]) -> SnapshotWindowStats {
        let out = SnapshotWindowStats {
            latest: self.latest,
            max: self.max,
            p95: self.p95(buckets),
            avg: if self.count > 0 {
                (self.total / self.count as u64).min(u32::MAX as u64) as u32
            } else {
                0
            },
            total: self.total,
            count: self.count,
        };
        *self = Self::default();
        out
    }

    fn p95(&self, buckets: &[u32]) -> u32 {
        if self.count == 0 {
            return 0;
        }
        let target = self.count.saturating_mul(95).saturating_add(99) / 100;
        let mut seen = 0u32;
        for (index, count) in self.bucket_counts.iter().enumerate() {
            seen = seen.saturating_add(*count);
            if seen >= target {
                if index == buckets.len() {
                    return self.max;
                }
                return buckets
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| *buckets.last().unwrap_or(&0));
            }
        }
        0
    }
}

fn consume_payload_sections(
    sections: &mut BTreeMap<&'static str, SnapshotPayloadTotals>,
    total_bytes: u64,
) -> Vec<SnapshotPayloadSectionReportStats> {
    let mut out = std::mem::take(sections)
        .into_iter()
        .map(|(section, totals)| SnapshotPayloadSectionReportStats {
            section: section.to_string(),
            count: totals.count.min(u32::MAX as u64) as u32,
            bytes: totals.bytes.min(u32::MAX as u64) as u32,
            pct_x100: pct_x100(totals.bytes, total_bytes),
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.section.cmp(&b.section))
    });
    out.truncate(SNAPSHOT_PAYLOAD_TOP_N);
    out
}

fn consume_entity_kinds(
    entity_kinds: &mut BTreeMap<String, SnapshotPayloadTotals>,
    total_bytes: u64,
) -> Vec<SnapshotPayloadEntityKindReportStats> {
    let mut out = std::mem::take(entity_kinds)
        .into_iter()
        .map(|(kind, totals)| SnapshotPayloadEntityKindReportStats {
            kind,
            count: totals.count.min(u32::MAX as u64) as u32,
            approx_bytes: totals.bytes.min(u32::MAX as u64) as u32,
            pct_x100: pct_x100(totals.bytes, total_bytes),
        })
        .collect::<Vec<_>>();
    out.sort_by(|a, b| {
        b.approx_bytes
            .cmp(&a.approx_bytes)
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.kind.cmp(&b.kind))
    });
    out.truncate(SNAPSHOT_PAYLOAD_TOP_N);
    out
}

fn pct_x100(value: u64, total: u64) -> u16 {
    if total == 0 {
        return 0;
    }
    ((value.saturating_mul(10_000) / total).min(u16::MAX as u64)) as u16
}

impl CommandLifecycleWindow {
    fn add_exemplar<const N: usize>(
        &mut self,
        received_unix_ms: u64,
        client_seq: u32,
        family: &'static str,
        stages: [(&'static str, u32); N],
    ) {
        let Some((stage, stage_ms)) = stages.into_iter().max_by_key(|(_, value)| *value) else {
            return;
        };
        self.exemplars.push(CommandLifecycleExemplarStats {
            received_unix_ms,
            client_seq,
            family: family.to_string(),
            stage: stage.to_string(),
            stage_ms,
        });
        self.exemplars.sort_by(|a, b| {
            b.stage_ms
                .cmp(&a.stage_ms)
                .then_with(|| a.client_seq.cmp(&b.client_seq))
        });
        self.exemplars
            .truncate(COMMAND_LIFECYCLE_TOP_N.min(self.exemplars.len()));
    }

    fn consume(&mut self) -> CommandLifecycleReportStats {
        let out = CommandLifecycleReportStats {
            count: self.count,
            accepted: self.accepted,
            rejected: self.rejected,
            frame_deserialize: self.frame_deserialize.consume(),
            deserialize_to_room_enqueue: self.deserialize_to_room_enqueue.consume(),
            room_queue: self.room_queue.consume(),
            room_handle: self.room_handle.consume(),
            receipt_send_age: self.receipt_send_age.consume(),
            accepted_to_sim_ack: self.accepted_to_sim_ack.consume(),
            exemplars: std::mem::take(&mut self.exemplars),
        };
        self.count = 0;
        self.accepted = 0;
        self.rejected = 0;
        out
    }
}

impl CommandTimingWindow {
    fn add(&mut self, value_ms: u32) {
        let value_ms = value_ms.min(u16::MAX as u32);
        self.latest_ms = value_ms;
        self.max_ms = self.max_ms.max(value_ms);
        self.count = self.count.saturating_add(1);
        let index = COMMAND_LIFECYCLE_BUCKETS_MS
            .iter()
            .position(|bucket| value_ms <= *bucket)
            .unwrap_or(COMMAND_LIFECYCLE_BUCKETS_MS.len());
        if let Some(count) = self.bucket_counts.get_mut(index) {
            *count = count.saturating_add(1);
        }
    }

    fn consume(&mut self) -> CommandTimingStats {
        let out = CommandTimingStats {
            latest_ms: self.latest_ms,
            max_ms: self.max_ms,
            p95_ms: self.p95_ms(),
            count: self.count,
        };
        *self = Self::default();
        out
    }

    fn p95_ms(&self) -> u32 {
        if self.count == 0 {
            return 0;
        }
        let target = self.count.saturating_mul(95).saturating_add(99) / 100;
        let mut seen = 0u32;
        for (index, count) in self.bucket_counts.iter().enumerate() {
            seen = seen.saturating_add(*count);
            if seen >= target {
                if index == COMMAND_LIFECYCLE_BUCKETS_MS.len() {
                    return self.max_ms;
                }
                return COMMAND_LIFECYCLE_BUCKETS_MS
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| *COMMAND_LIFECYCLE_BUCKETS_MS.last().unwrap_or(&0));
            }
        }
        0
    }
}

fn fetch_max(target: &AtomicU32, value: u32) {
    let mut current = target.load(Ordering::Relaxed);
    while value > current {
        match target.compare_exchange_weak(current, value, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => current = next,
        }
    }
}

fn duration_since_ms_u32(start: StdInstant) -> u32 {
    start.elapsed().as_millis().min(u32::MAX as u128) as u32
}

fn merge_resource_deltas(snapshot: &mut Snapshot, previous: &[ResourceDelta]) {
    if previous.is_empty() {
        return;
    }
    for old in previous {
        if !snapshot.resource_deltas.iter().any(|d| d.id == old.id) {
            snapshot.resource_deltas.push(old.clone());
        }
    }
    snapshot.resource_deltas.sort_by_key(|d| d.id);
}

/// Send to one player's sink without ever blocking the room task. Reliable messages use a bounded
/// FIFO; snapshots and observer analysis use replaceable latest-only slots.
pub(super) fn send_or_log(
    room: &str,
    player_id: u32,
    tx: &ConnectionSink,
    msg: ServerMessage,
) -> Option<SnapshotSendStatus> {
    match msg {
        ServerMessage::Snapshot(snapshot) => match tx.try_send_snapshot(snapshot) {
            SnapshotSendStatus::Stored => Some(SnapshotSendStatus::Stored),
            SnapshotSendStatus::Replaced => {
                crate::log_debug!(room = %room, player_id, "coalesced pending snapshot");
                Some(SnapshotSendStatus::Replaced)
            }
            SnapshotSendStatus::Closed => {
                crate::log_debug!(room = %room, player_id, "snapshot sink closed; client gone");
                Some(SnapshotSendStatus::Closed)
            }
        },
        ServerMessage::ObserverAnalysis(payload) => {
            match tx.try_send_observer_analysis(payload) {
                LatestOnlySendStatus::Stored => {}
                LatestOnlySendStatus::Replaced => {
                    crate::log_debug!(room = %room, player_id, "coalesced pending observer analysis");
                }
                LatestOnlySendStatus::Closed => {
                    crate::log_debug!(room = %room, player_id, "observer analysis sink closed; client gone");
                }
            }
            None
        }
        reliable => {
            if let Err(err) = tx.try_send_reliable(reliable) {
                match err {
                    mpsc::error::TrySendError::Full(_) => {
                        crate::log_warn!(room = %room, player_id, "reliable outbound queue full; dropping message");
                    }
                    mpsc::error::TrySendError::Closed(_) => {
                        crate::log_debug!(room = %room, player_id, "reliable outbound channel closed; client gone");
                    }
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn clearing_pending_snapshot_also_clears_observer_analysis() {
        let (sink, writer) = ConnectionSink::new();

        assert_eq!(
            sink.try_send_snapshot(Snapshot {
                tick: 1,
                world_combat_position: None,
                steel: 0,
                oil: 0,
                supply_used: 0,
                supply_cap: 0,
                entities: Vec::new(),
                resource_deltas: Vec::new(),
                smokes: Vec::new(),
                trenches: Vec::new(),
                ability_objects: Vec::new(),
                visible_tiles: Vec::new(),
                explored_tiles: Vec::new(),
                remembered_buildings: Vec::new(),
                events: Vec::new(),
                upgrades: Vec::new(),
                player_resources: Vec::new(),
                net_status: crate::protocol::SnapshotNetStatus::default(),
            }),
            SnapshotSendStatus::Stored
        );
        assert_eq!(
            sink.try_send_observer_analysis(observer_analysis_payload(30)),
            LatestOnlySendStatus::Stored
        );

        sink.clear_pending_snapshot();

        assert!(writer.snapshots.take().is_none());
        assert!(writer.observer_analysis.take().is_none());
    }
}
