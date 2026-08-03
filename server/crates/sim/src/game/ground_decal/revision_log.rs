use std::collections::BTreeSet;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};

use super::{decal_for_id, GroundDecal, GroundDecalStore, GroundDecalView, TankTrailView};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum GroundDecalRevisionEntry {
    Created { id: u32, tick: u32 },
    Discovered { player: u32, id: u32, tick: u32 },
    TrailCreated { id: u32, tick: u32 },
    TrailDiscovered { player: u32, id: u32, tick: u32 },
}

impl Serialize for GroundDecalRevisionEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Created { id, tick } => (0u32, *id, *tick).serialize(serializer),
            Self::Discovered { player, id, tick } => {
                (1u32, *player, *id, *tick).serialize(serializer)
            }
            Self::TrailCreated { id, tick } => (2u32, *id, *tick).serialize(serializer),
            Self::TrailDiscovered { player, id, tick } => {
                (3u32, *player, *id, *tick).serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for GroundDecalRevisionEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let row = Vec::<u32>::deserialize(deserializer)?;
        match row.as_slice() {
            [0, id, tick] => Ok(Self::Created {
                id: *id,
                tick: *tick,
            }),
            [1, player, id, tick] => Ok(Self::Discovered {
                player: *player,
                id: *id,
                tick: *tick,
            }),
            [2, id, tick] => Ok(Self::TrailCreated {
                id: *id,
                tick: *tick,
            }),
            [3, player, id, tick] => Ok(Self::TrailDiscovered {
                player: *player,
                id: *id,
                tick: *tick,
            }),
            _ => Err(D::Error::custom("invalid ground decal revision row")),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct GroundDecalRevisionLog(Vec<GroundDecalRevisionEntry>);

impl GroundDecalRevisionLog {
    pub(super) fn revisions_matching(
        &self,
        mut include: impl FnMut(&GroundDecalRevisionEntry) -> bool,
    ) -> Vec<u32> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, entry)| include(entry))
            .filter_map(|(index, _)| u32::try_from(index).ok()?.checked_add(1))
            .collect()
    }

    pub(super) fn revisions_at_tick(
        &self,
        tick: u32,
        mut include: impl FnMut(&GroundDecalRevisionEntry) -> bool,
    ) -> Vec<u32> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.tick() == tick && include(entry))
            .filter_map(|(index, _)| u32::try_from(index).ok()?.checked_add(1))
            .collect()
    }

    pub(super) fn record(
        &mut self,
        current_revision: u32,
        entry: GroundDecalRevisionEntry,
    ) -> Option<u32> {
        if usize::try_from(current_revision).ok() != Some(self.0.len()) {
            return None;
        }
        let next = current_revision.checked_add(1)?;
        self.0.push(entry);
        Some(next)
    }

    pub(super) fn valid(
        &self,
        store: &GroundDecalStore,
        used_revision_count: usize,
    ) -> bool {
        if usize::try_from(store.revision).ok() != Some(used_revision_count)
            || self.0.len() != used_revision_count
            || self.0.iter().any(|entry| entry.tick() > store.current_tick)
            || self
                .0
                .windows(2)
                .any(|pair| pair[0].tick() > pair[1].tick())
        {
            return false;
        }
        self.0.iter().enumerate().all(|(index, entry)| {
            let Some(entry_revision) = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
            else {
                return false;
            };
            match entry {
                GroundDecalRevisionEntry::Created { id, .. } => decal_for_id(&store.decals, *id)
                    .is_some_and(|decal| decal.created_revision == entry_revision),
                GroundDecalRevisionEntry::Discovered { player, id, .. } => store
                    .discovered_by_player
                    .get(player)
                    .and_then(|known| known.get(id))
                    .is_some_and(|known_revision| *known_revision == entry_revision),
                GroundDecalRevisionEntry::TrailCreated { id, .. } => store
                    .tank_trails
                    .created_revision(*id)
                    .is_some_and(|revision| revision == entry_revision),
                GroundDecalRevisionEntry::TrailDiscovered { player, id, .. } => {
                    store
                        .discovered_trails_by_player
                        .get(player)
                        .and_then(|known| known.get(id))
                        .is_some_and(|known_revision| *known_revision == entry_revision)
                }
            }
        })
    }
}

impl GroundDecalRevisionEntry {
    fn tick(&self) -> u32 {
        match self {
            Self::Created { tick, .. }
            | Self::Discovered { tick, .. }
            | Self::TrailCreated { tick, .. }
            | Self::TrailDiscovered { tick, .. } => *tick,
        }
    }
}

impl GroundDecalStore {
    pub(crate) fn revision_for_players(&self, players: &[u32]) -> u32 {
        let player_set = players.iter().copied().collect::<BTreeSet<_>>();
        let decal_revision = players
            .iter()
            .filter_map(|player| self.discovered_by_player.get(player))
            .flat_map(|known| known.values().copied())
            .max()
            .unwrap_or(0);
        let trail_revision = players
            .iter()
            .filter_map(|player| self.discovered_trails_by_player.get(player))
            .flat_map(|known| known.values().copied())
            .max()
            .unwrap_or(0);
        let owned_trail_revision = self
            .tank_trails
            .owned_created_revisions(&player_set)
            .max()
            .unwrap_or(0);
        decal_revision.max(trail_revision).max(owned_trail_revision)
    }

    pub(crate) fn views_for_players_after(
        &self,
        players: &[u32],
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>, Vec<TankTrailView>) {
        let revision = self.revision_for_players(players);
        let decals = self
            .decals
            .iter()
            .filter(|decal| {
                players.iter().any(|player| {
                    self.discovered_by_player
                        .get(player)
                        .and_then(|known| known.get(&decal.id))
                        .is_some_and(|revision| *revision > after_revision)
                })
            })
            .map(GroundDecal::to_view)
            .collect();
        let player_set = players.iter().copied().collect::<BTreeSet<_>>();
        let trail_ids = players
            .iter()
            .filter_map(|player| self.discovered_trails_by_player.get(player))
            .flat_map(|known| known.iter())
            .filter_map(|(id, revision)| (*revision > after_revision).then_some(*id))
            .chain(
                self.tank_trails
                    .owned_views_after(&player_set, after_revision)
                    .into_iter()
                    .map(|trail| trail.id),
            )
            .collect::<BTreeSet<_>>();
        let trails = trail_ids
            .into_iter()
            .filter_map(|id| self.tank_trails.view(id))
            .collect();
        (revision, decals, trails)
    }

    pub(crate) fn recent_views_for_players(
        &self,
        players: &[u32],
        max_revisions: usize,
    ) -> (u32, u32, Vec<GroundDecalView>, Vec<TankTrailView>) {
        let revision = self.revision_for_players(players);
        let player_set = players.iter().copied().collect::<BTreeSet<_>>();
        let include = |entry: &GroundDecalRevisionEntry| match entry {
            GroundDecalRevisionEntry::Discovered { player, .. }
            | GroundDecalRevisionEntry::TrailDiscovered { player, .. } => {
                player_set.contains(player)
            }
            GroundDecalRevisionEntry::TrailCreated { id, .. } => self
                .tank_trails
                .owner(*id)
                .is_some_and(|owner| player_set.contains(&owner)),
            _ => false,
        };
        let all_revisions = self.revision_log.revisions_matching(include);
        let current_revisions = self
            .revision_log
            .revisions_at_tick(self.current_tick, include);
        let after_revision =
            current_delta_after(revision, &all_revisions, &current_revisions, max_revisions);
        let (_, decals, trails) = self.views_for_players_after(players, after_revision);
        (revision, after_revision, decals, trails)
    }

    pub(crate) fn full_world_views_after(
        &self,
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>, Vec<TankTrailView>) {
        (
            self.revision,
            self.decals
                .iter()
                .filter(|decal| decal.created_revision > after_revision)
                .map(GroundDecal::to_view)
                .collect(),
            self.tank_trails.full_world_views_after(after_revision),
        )
    }

    pub(crate) fn recent_full_world_views(
        &self,
        max_revisions: usize,
    ) -> (u32, u32, Vec<GroundDecalView>, Vec<TankTrailView>) {
        let revision = self.revision;
        let include = |entry: &GroundDecalRevisionEntry| {
            matches!(
                entry,
                GroundDecalRevisionEntry::Created { .. }
                    | GroundDecalRevisionEntry::TrailCreated { .. }
            )
        };
        let all_revisions = self.revision_log.revisions_matching(include);
        let current_revisions = self
            .revision_log
            .revisions_at_tick(self.current_tick, include);
        let after_revision =
            current_delta_after(revision, &all_revisions, &current_revisions, max_revisions);
        let (_, decals, trails) = self.full_world_views_after(after_revision);
        (revision, after_revision, decals, trails)
    }

    pub(super) fn record_revision(&mut self, entry: GroundDecalRevisionEntry) -> Option<u32> {
        let next = self.revision_log.record(self.revision, entry)?;
        self.revision = next;
        Some(next)
    }
}

fn current_delta_after(
    revision: u32,
    all_revisions: &[u32],
    current_revisions: &[u32],
    max_revisions: usize,
) -> u32 {
    if current_revisions.is_empty() || max_revisions == 0 {
        return revision;
    }
    let retained = current_revisions.len().min(max_revisions);
    let first_retained = current_revisions[current_revisions.len() - retained];
    all_revisions
        .iter()
        .copied()
        .filter(|candidate| *candidate < first_retained)
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
pub(super) fn current_delta_after_for_test(
    revision: u32,
    all_revisions: &[u32],
    current_revisions: &[u32],
    max_revisions: usize,
) -> u32 {
    current_delta_after(revision, all_revisions, current_revisions, max_revisions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkpoint_revision_rows_use_compact_numeric_tuples() {
        let entry = GroundDecalRevisionEntry::TrailDiscovered {
            player: 2,
            id: 17,
            tick: 900,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert_eq!(json, "[3,2,17,900]");
        assert_eq!(
            serde_json::from_str::<GroundDecalRevisionEntry>(&json).unwrap(),
            entry
        );
    }
}
