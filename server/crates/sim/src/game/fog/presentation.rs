use std::collections::BTreeSet;

use super::Fog;

impl Fog {
    /// Build a temporary fog view where `viewer` can see every tile visible to any of `players`.
    pub fn union_for(&self, viewer: u32, players: &[u32]) -> Self {
        let cells = (self.size * self.size) as usize;
        let mut union = vec![false; cells];
        let mut explored_union = vec![false; cells];
        for player in players {
            if let Some(grid) = self.grids.get(player) {
                for (dst, src) in union.iter_mut().zip(grid.iter()) {
                    *dst = *dst || *src;
                }
            }
            if let Some(grid) = self.explored_grids.get(player) {
                for (dst, src) in explored_union.iter_mut().zip(grid.iter()) {
                    *dst = *dst || *src;
                }
            }
        }

        let mut fog = Fog::new(self.size);
        fog.grids.insert(viewer, union);
        fog.explored_grids.insert(viewer, explored_union);
        fog
    }

    /// Build the current-vision union used to draw fog, excluding tiles that are visible only
    /// because an enemy fired from them. Firing reveals remain in [`Self::union_for`] so combat
    /// and command validation keep using the actionable grid.
    pub(in crate::game) fn presentation_union_for(&self, viewer: u32, players: &[u32]) -> Self {
        let cells = (self.size * self.size) as usize;
        let mut union = vec![false; cells];
        let mut explored_union = vec![false; cells];
        for player in players {
            let reveal_only_tiles = self.firing_reveal_only_tiles(*player);
            if let Some(grid) = self.grids.get(player) {
                for (index, (dst, src)) in union.iter_mut().zip(grid.iter()).enumerate() {
                    if !reveal_only_tiles.contains(&(index as u32)) {
                        *dst = *dst || *src;
                    }
                }
            }
            if let Some(grid) = self.explored_grids.get(player) {
                for (dst, src) in explored_union.iter_mut().zip(grid.iter()) {
                    *dst = *dst || *src;
                }
            }
        }

        let mut fog = Fog::new(self.size);
        fog.grids.insert(viewer, union);
        fog.explored_grids.insert(viewer, explored_union);
        fog
    }

    /// Accumulate the current fog unions into each viewer's durable exploration history.
    pub(in crate::game) fn accumulate_explored_for_viewers(
        &mut self,
        viewer_sources: &[(u32, Vec<u32>)],
    ) {
        let cells = self.size.saturating_mul(self.size) as usize;
        for (viewer, sources) in viewer_sources {
            let mut current_union = vec![false; cells];
            for source in sources {
                let Some(grid) = self.grids.get(source) else {
                    continue;
                };
                let reveal_only_tiles = self.firing_reveal_only_tiles(*source);
                for (index, (dst, src)) in current_union.iter_mut().zip(grid.iter()).enumerate() {
                    if !reveal_only_tiles.contains(&(index as u32)) {
                        *dst = *dst || *src;
                    }
                }
            }
            let explored = self
                .explored_grids
                .entry(*viewer)
                .or_insert_with(|| vec![false; cells]);
            if explored.len() != cells {
                *explored = vec![false; cells];
            }
            for (dst, src) in explored.iter_mut().zip(current_union.iter()) {
                *dst = *dst || *src;
            }
        }
    }

    pub(super) fn accumulate_explored_for_players(&mut self, players: &[u32]) {
        let viewer_sources = players
            .iter()
            .map(|player| (*player, vec![*player]))
            .collect::<Vec<_>>();
        self.accumulate_explored_for_viewers(&viewer_sources);
    }

    fn firing_reveal_only_tiles(&self, player: u32) -> BTreeSet<u32> {
        self.firing_reveal_visibility
            .get(&player)
            .into_iter()
            .flat_map(|by_entity| by_entity.values())
            .filter_map(|visibility| {
                visibility
                    .reveal_only
                    .then_some(visibility.revealed_tile)
                    .flatten()
            })
            .collect()
    }
}
