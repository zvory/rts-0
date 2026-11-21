//! Diff only snapshots actually taken by the single socket writer. Pending snapshots stay
//! complete, so coalescing never loses a depletion or a perspective replacement.
use crate::protocol::Snapshot;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct ResourceDelivery {
    generation: Option<u64>,
    remaining: BTreeMap<u32, u32>,
}

impl ResourceDelivery {
    pub(super) fn encode(&mut self, snapshot: &mut Snapshot, generation: u64) {
        if !snapshot.resource_reset {
            return;
        }
        let reset = self.generation != Some(generation);
        let complete = std::mem::take(&mut snapshot.resource_deltas);
        let next = complete.iter().map(|row| (row.id, row.remaining)).collect();
        snapshot.resource_deltas = complete
            .into_iter()
            .filter(|row| reset || self.remaining.get(&row.id) != Some(&row.remaining))
            .collect();
        self.remaining = next;
        self.generation = Some(generation);
        snapshot.resource_reset = reset;
    }
}
