use serde::{Deserialize, Serialize};

use crate::game::entity::{EntityStore, FiringRevealEpisode};
use crate::game::firing_reveal::FiringRevealSource;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;

use super::{stamp_point, Fog};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(in crate::game) struct FiringRevealVisibility {
    pub(in crate::game) episode_started_at_tick: u32,
    pub(in crate::game) reveal_only: bool,
    /// Row-major tile stamped by this source in the current fog result.
    ///
    /// Provenance must follow the tile, not just the firing entity: another entity standing on
    /// this tile is actionable through the same reveal, while the firer can move onto a tile that
    /// ordinary sight already covers before the next fog rebuild.
    pub(in crate::game) revealed_tile: Option<u32>,
}

impl Fog {
    pub(in crate::game) fn stamp_firing_reveal_sources_with_smoke(
        &mut self,
        sources: &[FiringRevealSource],
        store: &EntityStore,
        smokes: &SmokeCloudStore,
    ) {
        self.firing_reveal_visibility.clear();
        // Classify every source against the same pre-reveal grids. Stamping in this pass would
        // let one revealed entity make a colocated reveal look like ordinary sight.
        for source in sources {
            let visibility = FiringRevealVisibility {
                episode_started_at_tick: source.started_at_tick(),
                reveal_only: false,
                revealed_tile: None,
            };
            self.firing_reveal_visibility
                .entry(source.viewer())
                .or_default()
                .insert(source.entity_id(), visibility);
            let Some(entity) = store.get(source.entity_id()) else {
                continue;
            };
            if entity.hp == 0 || smokes.point_inside(entity.pos_x, entity.pos_y) {
                continue;
            }
            let Some(tile) = super::world_tile_index(self.size, entity.pos_x, entity.pos_y) else {
                continue;
            };
            let Some(visibility) = self
                .firing_reveal_visibility
                .get_mut(&source.viewer())
                .and_then(|by_entity| by_entity.get_mut(&source.entity_id()))
            else {
                continue;
            };
            visibility.revealed_tile = Some(tile);
            visibility.reveal_only = !self
                .grids
                .get(&source.viewer())
                .and_then(|grid| grid.get(tile as usize))
                .copied()
                .unwrap_or(false);
        }

        let size = self.size;
        for source in sources {
            let Some(entity) = store.get(source.entity_id()) else {
                continue;
            };
            if entity.hp == 0 || smokes.point_inside(entity.pos_x, entity.pos_y) {
                continue;
            }
            let Some(grid) = self.grids.get_mut(&source.viewer()) else {
                continue;
            };
            stamp_point(grid, size, entity.pos_x, entity.pos_y);
        }
    }

    /// The active reveal episode for `entity_id`, whether or not ordinary sight also sees it.
    pub(in crate::game) fn active_firing_reveal_episode(
        &self,
        viewer: u32,
        entity_id: u32,
    ) -> Option<u32> {
        self.firing_reveal_visibility
            .get(&viewer)?
            .get(&entity_id)
            .map(|visibility| visibility.episode_started_at_tick)
    }

    /// The active reveal episode only when firing reveal is necessary for actionable sight.
    #[cfg(test)]
    pub(in crate::game) fn firing_reveal_only_episode(
        &self,
        viewer: u32,
        entity_id: u32,
    ) -> Option<u32> {
        let visibility = self
            .firing_reveal_visibility
            .get(&viewer)?
            .get(&entity_id)?;
        visibility
            .reveal_only
            .then_some(visibility.episode_started_at_tick)
    }

    /// The firing-reveal source that makes this world point actionable, if ordinary sight does
    /// not cover the tile. Multiple reveal sources on one tile resolve deterministically to the
    /// oldest episode so adding another source cannot restart an in-progress reaction gate.
    pub(in crate::game) fn firing_reveal_only_source_at_world(
        &self,
        viewer: u32,
        x: f32,
        y: f32,
    ) -> Option<FiringRevealEpisode> {
        let tile = super::world_tile_index(self.size, x, y)?;
        if !self.is_visible(viewer, tile % self.size, tile / self.size) {
            return None;
        }
        self.firing_reveal_visibility
            .get(&viewer)?
            .iter()
            .filter(|(_, visibility)| {
                visibility.reveal_only && visibility.revealed_tile == Some(tile)
            })
            .min_by_key(|(source_entity, visibility)| {
                (visibility.episode_started_at_tick, **source_entity)
            })
            .map(|(&source_entity, visibility)| FiringRevealEpisode {
                viewer,
                source_entity,
                started_at_tick: visibility.episode_started_at_tick,
            })
    }

    pub(in crate::game) fn is_visible_without_firing_reveal_world(
        &self,
        viewer: u32,
        x: f32,
        y: f32,
    ) -> bool {
        self.is_visible_world(viewer, x, y)
            && self
                .firing_reveal_only_source_at_world(viewer, x, y)
                .is_none()
    }

    /// Resolve reveal-only provenance through the same team visibility scope as explicit fire.
    pub(in crate::game) fn team_firing_reveal_only_source(
        &self,
        viewer: u32,
        target_pos: (f32, f32),
        teams: &TeamRelations,
    ) -> Option<FiringRevealEpisode> {
        let mut reveal_source = None;
        let mut contributors = teams.same_team_player_ids(viewer);
        if contributors.is_empty() {
            contributors.push(viewer);
        }
        for contributor in contributors {
            if !self.is_visible_world(contributor, target_pos.0, target_pos.1) {
                continue;
            }
            let mut candidate =
                self.firing_reveal_only_source_at_world(contributor, target_pos.0, target_pos.1)?;
            candidate.viewer = contributor;
            if reveal_source.is_none_or(|current: FiringRevealEpisode| {
                (
                    candidate.started_at_tick,
                    candidate.viewer,
                    candidate.source_entity,
                ) < (
                    current.started_at_tick,
                    current.viewer,
                    current.source_entity,
                )
            }) {
                reveal_source = Some(candidate);
            }
        }
        reveal_source
    }
}
