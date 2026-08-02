use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{decal_for_id, GroundDecal, GroundDecalStore, GroundDecalView};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(super) enum GroundDecalRevisionEntry {
    Created { id: u32 },
    Discovered { player: u32, id: u32 },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub(super) struct GroundDecalRevisionLog(Vec<GroundDecalRevisionEntry>);

impl GroundDecalRevisionLog {
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

    pub(super) fn recent_views(
        &self,
        after_revision: u32,
        revision: u32,
        decals: &[GroundDecal],
        id_for_entry: impl FnMut(&GroundDecalRevisionEntry) -> Option<u32>,
    ) -> Vec<GroundDecalView> {
        let start = usize::try_from(after_revision).unwrap_or(self.0.len());
        let end = usize::try_from(revision)
            .unwrap_or(self.0.len())
            .min(self.0.len());
        let mut seen = BTreeSet::new();
        self.0
            .get(start..end)
            .into_iter()
            .flatten()
            .filter_map(id_for_entry)
            .filter(|id| seen.insert(*id))
            .filter_map(|id| decal_for_id(decals, id))
            .map(GroundDecal::to_view)
            .collect()
    }

    pub(super) fn valid(
        &self,
        revision: u32,
        decals: &[GroundDecal],
        discovered_by_player: &BTreeMap<u32, BTreeMap<u32, u32>>,
        used_revision_count: usize,
    ) -> bool {
        if usize::try_from(revision).ok() != Some(used_revision_count)
            || self.0.len() != used_revision_count
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
                GroundDecalRevisionEntry::Created { id } => decal_for_id(decals, *id)
                    .is_some_and(|decal| decal.created_revision == entry_revision),
                GroundDecalRevisionEntry::Discovered { player, id } => discovered_by_player
                    .get(player)
                    .and_then(|known| known.get(id))
                    .is_some_and(|known_revision| *known_revision == entry_revision),
            }
        })
    }
}

impl GroundDecalStore {
    pub(crate) fn revision_for_players(&self, players: &[u32]) -> u32 {
        players
            .iter()
            .filter_map(|player| self.discovered_by_player.get(player))
            .flat_map(|known| known.values().copied())
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn views_for_players_after(
        &self,
        players: &[u32],
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>) {
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
        (revision, decals)
    }

    pub(crate) fn recent_views_for_players(
        &self,
        players: &[u32],
        max_revisions: usize,
    ) -> (u32, u32, Vec<GroundDecalView>) {
        let revision = self.revision_for_players(players);
        let after_revision = revision.saturating_sub(max_revisions.min(u32::MAX as usize) as u32);
        let player_set = players.iter().copied().collect::<BTreeSet<_>>();
        let decals =
            self.revision_log
                .recent_views(
                    after_revision,
                    revision,
                    &self.decals,
                    |entry| match entry {
                        GroundDecalRevisionEntry::Discovered { player, id }
                            if player_set.contains(player) =>
                        {
                            Some(*id)
                        }
                        _ => None,
                    },
                );
        (revision, after_revision, decals)
    }

    pub(crate) fn full_world_views_after(
        &self,
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>) {
        (
            self.revision,
            self.decals
                .iter()
                .filter(|decal| decal.created_revision > after_revision)
                .map(GroundDecal::to_view)
                .collect(),
        )
    }

    pub(crate) fn recent_full_world_views(
        &self,
        max_revisions: usize,
    ) -> (u32, u32, Vec<GroundDecalView>) {
        let revision = self.revision;
        let after_revision = revision.saturating_sub(max_revisions.min(u32::MAX as usize) as u32);
        let decals =
            self.revision_log
                .recent_views(
                    after_revision,
                    revision,
                    &self.decals,
                    |entry| match entry {
                        GroundDecalRevisionEntry::Created { id } => Some(*id),
                        GroundDecalRevisionEntry::Discovered { .. } => None,
                    },
                );
        (revision, after_revision, decals)
    }

    pub(super) fn record_revision(&mut self, entry: GroundDecalRevisionEntry) -> Option<u32> {
        let next = self.revision_log.record(self.revision, entry)?;
        self.revision = next;
        Some(next)
    }
}
