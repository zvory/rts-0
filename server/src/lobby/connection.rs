#![cfg_attr(test, allow(dead_code))]

use super::*;
use std::time::Instant as StdInstant;

/// Outbound connection handle shared with the room task. Reliable messages keep FIFO ordering;
/// snapshots share a single latest-only slot because older unsent snapshots are superseded by
/// newer full-state snapshots.
#[derive(Clone)]
pub struct ConnectionSink {
    reliable_tx: mpsc::Sender<ServerMessage>,
    snapshots: Arc<LatestSnapshotSlot>,
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

pub struct ConnectionSnapshotSend {
    snapshot: Snapshot,
    stats: SnapshotSendStats,
}

#[derive(Clone, Copy)]
pub struct SnapshotSendStats {
    age_ms: u32,
    waited_behind_reliable: bool,
}

pub struct ConnectionWriterStats {
    counters: Arc<ConnectionReportCounters>,
    reliable_drained_before_snapshot: u32,
}

#[derive(Debug, Clone, Copy, Default)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotSendStatus {
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
        ConnectionWriterStats,
    ) {
        (
            self.reliable_rx,
            self.snapshots,
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
        (
            ConnectionSink {
                reliable_tx,
                snapshots: snapshots.clone(),
                stats: stats.clone(),
            },
            ConnectionWriter {
                reliable_rx,
                snapshots,
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

    pub(crate) fn has_pending_snapshot(&self) -> bool {
        self.snapshots.has_pending()
    }

    pub(crate) fn clear_pending_snapshot(&self) {
        self.snapshots.clear();
    }

    pub fn consume_report_stats(&self) -> ConnectionReportStats {
        self.stats.consume()
    }

    fn record_command_receipt_queued(&self, accepted: Option<bool>) {
        match accepted {
            Some(true) => {
                self.stats
                    .command_receipts_accepted
                    .fetch_add(1, Ordering::Relaxed);
            }
            Some(false) => {
                self.stats
                    .command_receipts_rejected
                    .fetch_add(1, Ordering::Relaxed);
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
        let stats = SnapshotSendStats {
            age_ms: pending.age_ms(),
            waited_behind_reliable: self.reliable_drained_before_snapshot > 0,
        };
        self.reliable_drained_before_snapshot = 0;
        ConnectionSnapshotSend {
            snapshot: pending.into_snapshot(),
            stats,
        }
    }

    pub fn record_snapshot_sent(&self, stats: SnapshotSendStats) {
        self.counters
            .record_snapshot_sent(stats.age_ms, stats.waited_behind_reliable);
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

    fn consume(&self) -> ConnectionReportStats {
        let snapshot_sent = self.snapshot_sent.swap(0, Ordering::Relaxed);
        let snapshot_send_age_total_ms = self.snapshot_send_age_total_ms.swap(0, Ordering::Relaxed);
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
        }
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
/// FIFO and snapshots use a replaceable latest-only slot.
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
