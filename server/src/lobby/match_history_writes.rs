use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::watch;

#[derive(Clone)]
pub(super) struct MatchHistoryWriteTracker {
    inner: Arc<Inner>,
}

struct Inner {
    state: Mutex<State>,
    changes_tx: watch::Sender<u64>,
}

#[derive(Default)]
struct State {
    next_id: u64,
    generation: u64,
    pending: HashSet<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchHistoryWriteWaitResult {
    pub initial_pending: usize,
    pub remaining_pending: usize,
    pub timed_out: bool,
}

impl Default for MatchHistoryWriteTracker {
    fn default() -> Self {
        let (changes_tx, _changes_rx) = watch::channel(0);
        Self {
            inner: Arc::new(Inner {
                state: Mutex::new(State::default()),
                changes_tx,
            }),
        }
    }
}

impl MatchHistoryWriteTracker {
    pub(super) fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let id = self.register();
        let guard = CompletionGuard {
            id,
            inner: self.inner.clone(),
        };
        tokio::spawn(async move {
            let _guard = guard;
            future.await;
        });
    }

    pub(super) fn pending_count(&self) -> usize {
        self.inner.lock_state().pending.len()
    }

    pub(super) async fn wait_for_pending_at_start(
        &self,
        timeout: Duration,
    ) -> MatchHistoryWriteWaitResult {
        let snapshot = self.snapshot_pending();
        self.wait_for_snapshot(snapshot, timeout).await
    }

    async fn wait_for_snapshot(
        &self,
        snapshot: HashSet<u64>,
        timeout: Duration,
    ) -> MatchHistoryWriteWaitResult {
        let mut changes_rx = self.inner.changes_tx.subscribe();
        let initial_pending = snapshot.len();
        if initial_pending == 0 {
            return MatchHistoryWriteWaitResult {
                initial_pending,
                remaining_pending: 0,
                timed_out: false,
            };
        }

        let wait = async {
            loop {
                let remaining = self.remaining_from_snapshot(&snapshot);
                if remaining == 0 {
                    return 0;
                }
                if changes_rx.changed().await.is_err() {
                    return remaining;
                }
            }
        };

        match tokio::time::timeout(timeout, wait).await {
            Ok(remaining_pending) => MatchHistoryWriteWaitResult {
                initial_pending,
                remaining_pending,
                timed_out: remaining_pending > 0,
            },
            Err(_) => {
                let remaining_pending = self.remaining_from_snapshot(&snapshot);
                MatchHistoryWriteWaitResult {
                    initial_pending,
                    remaining_pending,
                    timed_out: remaining_pending > 0,
                }
            }
        }
    }

    fn register(&self) -> u64 {
        let mut state = self.inner.lock_state();
        let id = state.next_id;
        state.next_id = state.next_id.wrapping_add(1);
        state.pending.insert(id);
        id
    }

    fn snapshot_pending(&self) -> HashSet<u64> {
        self.inner.lock_state().pending.clone()
    }

    fn remaining_from_snapshot(&self, snapshot: &HashSet<u64>) -> usize {
        let state = self.inner.lock_state();
        snapshot
            .iter()
            .filter(|id| state.pending.contains(id))
            .count()
    }
}

impl Inner {
    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn complete(&self, id: u64) {
        let generation = {
            let mut state = self.lock_state();
            if !state.pending.remove(&id) {
                return;
            }
            state.generation = state.generation.wrapping_add(1);
            state.generation
        };
        self.changes_tx.send_replace(generation);
    }
}

struct CompletionGuard {
    id: u64,
    inner: Arc<Inner>,
}

impl Drop for CompletionGuard {
    fn drop(&mut self) {
        self.inner.complete(self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::oneshot;

    async fn wait_for_pending_count(tracker: &MatchHistoryWriteTracker, expected: usize) {
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if tracker.pending_count() == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("pending write count did not settle");
    }

    #[tokio::test]
    async fn tracked_write_increments_and_decrements_pending_count() {
        let tracker = MatchHistoryWriteTracker::default();
        let (release_tx, release_rx) = oneshot::channel();

        tracker.spawn(async move {
            let _ = release_rx.await;
        });
        assert_eq!(tracker.pending_count(), 1);

        release_tx.send(()).expect("release should send");
        wait_for_pending_count(&tracker, 0).await;
    }

    #[tokio::test]
    async fn wait_returns_when_snapshot_writes_complete() {
        let tracker = MatchHistoryWriteTracker::default();
        let (release_tx, release_rx) = oneshot::channel();

        tracker.spawn(async move {
            let _ = release_rx.await;
        });
        let snapshot = tracker.snapshot_pending();
        let waiter = {
            let tracker = tracker.clone();
            tokio::spawn(async move {
                tracker
                    .wait_for_snapshot(snapshot, Duration::from_secs(1))
                    .await
            })
        };

        release_tx.send(()).expect("release should send");
        let result = waiter.await.expect("waiter should not panic");
        assert_eq!(
            result,
            MatchHistoryWriteWaitResult {
                initial_pending: 1,
                remaining_pending: 0,
                timed_out: false,
            }
        );
    }

    #[tokio::test]
    async fn wait_times_out_with_remaining_pending_writes() {
        let tracker = MatchHistoryWriteTracker::default();

        tracker.spawn(std::future::pending::<()>());
        let result = tracker
            .wait_for_pending_at_start(Duration::from_millis(10))
            .await;

        assert_eq!(
            result,
            MatchHistoryWriteWaitResult {
                initial_pending: 1,
                remaining_pending: 1,
                timed_out: true,
            }
        );
    }

    #[tokio::test]
    async fn wait_is_snapshot_based_and_ignores_later_writes() {
        let tracker = MatchHistoryWriteTracker::default();
        let (first_release_tx, first_release_rx) = oneshot::channel();

        tracker.spawn(async move {
            let _ = first_release_rx.await;
        });
        let snapshot = tracker.snapshot_pending();
        tracker.spawn(std::future::pending::<()>());
        first_release_tx
            .send(())
            .expect("first release should send");

        let result = tracker
            .wait_for_snapshot(snapshot, Duration::from_secs(1))
            .await;
        assert_eq!(
            result,
            MatchHistoryWriteWaitResult {
                initial_pending: 1,
                remaining_pending: 0,
                timed_out: false,
            }
        );
        assert_eq!(tracker.pending_count(), 1);
    }
}
