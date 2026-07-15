use std::collections::BTreeMap;

/// Stable, tick-scoped ids for entities whose bodies can intercept direct fire.
///
/// Combat ray tests are much more frequent than blocker topology changes. Building this once at
/// the start of the combat phase avoids recreating and sorting the complete entity-id list for
/// every candidate target while keeping exact geometry and current-health checks at query time.
pub(super) struct ShotBlockerIndex {
    all: Vec<ShotBlockerEntry>,
    by_owner: BTreeMap<u32, Vec<ShotBlockerEntry>>,
}

#[derive(Clone, Copy)]
pub(super) struct ShotBlockerEntry {
    pub(super) id: u32,
    pub(super) bounds: ShotBlockerBounds,
}

#[derive(Clone, Copy)]
pub(super) struct ShotBlockerBounds {
    pub(super) min_x: f32,
    pub(super) min_y: f32,
    pub(super) max_x: f32,
    pub(super) max_y: f32,
}

impl ShotBlockerBounds {
    pub(super) fn overlaps_segment_bounds(self, start: (f32, f32), end: (f32, f32)) -> bool {
        let segment_min_x = start.0.min(end.0);
        let segment_min_y = start.1.min(end.1);
        let segment_max_x = start.0.max(end.0);
        let segment_max_y = start.1.max(end.1);
        segment_max_x >= self.min_x
            && segment_min_x <= self.max_x
            && segment_max_y >= self.min_y
            && segment_min_y <= self.max_y
    }
}

impl ShotBlockerIndex {
    pub(super) fn from_entries(
        entries: impl Iterator<Item = (u32, u32, ShotBlockerBounds)>,
    ) -> Self {
        let mut all = Vec::new();
        let mut by_owner = BTreeMap::<u32, Vec<ShotBlockerEntry>>::new();
        for (id, owner, bounds) in entries {
            let entry = ShotBlockerEntry { id, bounds };
            all.push(entry);
            by_owner.entry(owner).or_default().push(entry);
        }
        ShotBlockerIndex { all, by_owner }
    }

    pub(super) fn all(&self) -> &[ShotBlockerEntry] {
        &self.all
    }

    pub(super) fn owned_by(&self, owner: u32) -> &[ShotBlockerEntry] {
        self.by_owner.get(&owner).map_or(&[], Vec::as_slice)
    }
}
