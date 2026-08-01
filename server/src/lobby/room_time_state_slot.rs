use std::sync::{Mutex as StdMutex, MutexGuard};

use tokio::sync::Notify;

use crate::protocol::RoomTimeState;

/// Coalescing outbound slot for authoritative room-clock state.
pub struct LatestRoomTimeStateSlot {
    pub(super) pending: StdMutex<Option<RoomTimeState>>,
    pub(super) notify: Notify,
}

impl LatestRoomTimeStateSlot {
    fn lock_pending(&self) -> MutexGuard<'_, Option<RoomTimeState>> {
        match self.pending.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn take(&self) -> Option<RoomTimeState> {
        self.lock_pending().take()
    }

    /// Return a state taken before discovering an ordering barrier, without replacing newer state.
    pub fn defer(&self, state: RoomTimeState) {
        let mut pending = self.lock_pending();
        if pending.is_none() {
            *pending = Some(state);
        }
        drop(pending);
        self.notify.notify_one();
    }

    pub(super) fn store(&self, state: RoomTimeState) -> bool {
        let mut pending = self.lock_pending();
        let replaced = pending.is_some();
        *pending = Some(state);
        drop(pending);
        self.notify.notify_one();
        replaced
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}
