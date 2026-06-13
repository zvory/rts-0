use super::*;

/// Outbound connection handle shared with the room task. Reliable messages keep FIFO ordering;
/// snapshots share a single latest-only slot because older unsent snapshots are superseded by
/// newer full-state snapshots.
#[derive(Clone)]
pub struct ConnectionSink {
    reliable_tx: mpsc::Sender<ServerMessage>,
    snapshots: Arc<LatestSnapshotSlot>,
}

impl std::fmt::Debug for ConnectionSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionSink").finish_non_exhaustive()
    }
}

pub struct ConnectionWriter {
    pub reliable_rx: mpsc::Receiver<ServerMessage>,
    pub snapshots: Arc<LatestSnapshotSlot>,
}

pub struct LatestSnapshotSlot {
    pending: StdMutex<Option<Snapshot>>,
    notify: Notify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SnapshotSendStatus {
    Stored,
    Replaced,
    Closed,
}

impl ConnectionSink {
    pub fn new() -> (Self, ConnectionWriter) {
        let (reliable_tx, reliable_rx) = mpsc::channel(PLAYER_RELIABLE_CHANNEL_CAP);
        let snapshots = Arc::new(LatestSnapshotSlot {
            pending: StdMutex::new(None),
            notify: Notify::new(),
        });
        (
            ConnectionSink {
                reliable_tx,
                snapshots: snapshots.clone(),
            },
            ConnectionWriter {
                reliable_rx,
                snapshots,
            },
        )
    }

    pub async fn send_reliable(
        &self,
        msg: ServerMessage,
    ) -> Result<(), mpsc::error::SendError<ServerMessage>> {
        self.reliable_tx.send(msg).await
    }

    #[allow(clippy::result_large_err)]
    pub fn try_send_reliable(
        &self,
        msg: ServerMessage,
    ) -> Result<(), mpsc::error::TrySendError<ServerMessage>> {
        self.reliable_tx.try_send(msg)
    }

    pub(crate) fn try_send_snapshot(&self, mut snapshot: Snapshot) -> SnapshotSendStatus {
        if self.reliable_tx.is_closed() {
            return SnapshotSendStatus::Closed;
        }

        let mut pending = self.snapshots.lock_pending();
        let replaced = if let Some(previous) = pending.as_ref() {
            merge_resource_deltas(&mut snapshot, &previous.resource_deltas);
            true
        } else {
            false
        };
        *pending = Some(snapshot);
        drop(pending);

        self.snapshots.notify.notify_one();
        if replaced {
            SnapshotSendStatus::Replaced
        } else {
            SnapshotSendStatus::Stored
        }
    }

    pub(crate) fn has_pending_snapshot(&self) -> bool {
        self.snapshots.has_pending()
    }

    pub(crate) fn clear_pending_snapshot(&self) {
        self.snapshots.clear();
    }
}

impl LatestSnapshotSlot {
    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<Snapshot>> {
        match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn take(&self) -> Option<Snapshot> {
        self.lock_pending().take()
    }

    fn has_pending(&self) -> bool {
        self.lock_pending().is_some()
    }

    fn clear(&self) {
        self.lock_pending().take();
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
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
