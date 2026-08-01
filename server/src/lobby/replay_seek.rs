use std::time::Duration;

use super::reconstruction::contain_reconstruction;
use super::replay_session::{ReplaySeekAdvance, ReplaySession};

const SLICE_MAX_TICKS: u32 = 128;
const SLICE_BUDGET: Duration = Duration::from_millis(12);
const PUBLISH_INTERVAL: Duration = Duration::from_millis(100);

/// Advance one bounded slice without letting reconstruction errors or panics escape the room task.
pub(super) fn advance_slice(session: &mut ReplaySession) -> Result<ReplaySeekAdvance, String> {
    let advance = || session.advance_seek_slice(SLICE_MAX_TICKS, SLICE_BUDGET, PUBLISH_INTERVAL);
    if tokio::runtime::Handle::try_current()
        .is_ok_and(|handle| handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread)
    {
        tokio::task::block_in_place(|| contain_reconstruction("incremental replay seek", advance))
    } else {
        contain_reconstruction("incremental replay seek", advance)
    }
    .map_err(|failure| failure.to_string())
}
