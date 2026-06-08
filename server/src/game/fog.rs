//! Per-player fog of war. See `docs/design/server-sim.md` (`fog.rs`).
//!
//! The server is authoritative about visibility: each tick we recompute, for every player, a
//! boolean grid of which tiles that player can currently see. A tile is visible if it falls
//! within the sight circle of any of that player's entities (`sight_tiles`) and the line from
//! the entity to that tile is not blocked by stone. The snapshot layer uses this to withhold
//! neutral/enemy entities standing on non-visible tiles, making the fog cheat-proof.
//!
//! Note the server only needs *currently visible* — the client maintains the "explored but
//! not currently visible" dimming locally (see `docs/design/client-ui.md`). So this module tracks only
//! the per-tick visible set.

use std::collections::HashMap;

use crate::config;
use crate::game::entity::{Entity, EntityStore};
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::smoke::SmokeCloudStore;

/// Temporary sight left behind by an owned unit/building after it dies. This is used only by
/// snapshot projection; command validation and combat still use live fog.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LingeringSightSource {
    owner: u32,
    x: f32,
    y: f32,
    sight_tiles: u32,
    expires_at_tick: u32,
}

impl LingeringSightSource {
    pub(crate) fn new(
        owner: u32,
        x: f32,
        y: f32,
        sight_tiles: u32,
        expires_at_tick: u32,
    ) -> Option<Self> {
        if owner == 0 || sight_tiles == 0 || !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some(Self {
            owner,
            x,
            y,
            sight_tiles,
            expires_at_tick,
        })
    }

    pub(crate) fn is_active_at(self, tick: u32) -> bool {
        self.expires_at_tick > tick
    }

    pub(crate) fn owner(self) -> u32 {
        self.owner
    }
}

/// Visible-tile grids, one per player. Recomputed every tick from scratch (cheap at our map
/// sizes) so it always reflects current entity positions and never leaks stale visibility.
#[derive(Clone, Default)]
pub struct Fog {
    size: u32,
    /// player id -> row-major visibility grid (`true` = visible this tick).
    grids: HashMap<u32, Vec<bool>>,
}

impl Fog {
    pub fn new(size: u32) -> Self {
        Fog {
            size,
            grids: HashMap::new(),
        }
    }

    /// Recompute visibility for all `players` from the union of their entities' sight circles.
    /// Players with no entities get an all-dark grid.
    #[allow(dead_code)]
    pub fn recompute(&mut self, players: &[u32], store: &EntityStore, map: &Map) {
        self.recompute_inner(players, store, map, None);
    }

    pub(crate) fn recompute_with_smoke(
        &mut self,
        players: &[u32],
        store: &EntityStore,
        map: &Map,
        smokes: &SmokeCloudStore,
    ) {
        self.recompute_inner(players, store, map, Some(smokes));
    }

    fn recompute_inner(
        &mut self,
        players: &[u32],
        store: &EntityStore,
        map: &Map,
        smokes: Option<&SmokeCloudStore>,
    ) {
        let size = self.size;
        let cells = (self.size * self.size) as usize;
        // Reset / allocate a grid per player.
        for &p in players {
            let g = self.grids.entry(p).or_insert_with(|| vec![false; cells]);
            if g.len() != cells {
                *g = vec![false; cells];
            } else {
                g.iter_mut().for_each(|v| *v = false);
            }
        }

        let los = match smokes {
            Some(smokes) => LineOfSight::with_smoke(map, smokes),
            None => LineOfSight::new(map),
        };
        for e in store.iter() {
            if e.owner == 0 {
                continue; // neutral resource nodes do not grant a player vision
            }
            if smokes
                .map(|smokes| smokes.point_inside(e.pos_x, e.pos_y))
                .unwrap_or(false)
            {
                continue;
            }
            // Only stamp sight for players we are tracking this tick.
            let Some(grid) = self.grids.get_mut(&e.owner) else {
                continue;
            };
            stamp_sight(grid, size, e, &los);
        }
    }

    /// Add temporary death-vision sight sources to already-recomputed grids.
    #[allow(dead_code)]
    pub(crate) fn stamp_lingering_sources(&mut self, sources: &[LingeringSightSource], map: &Map) {
        self.stamp_lingering_sources_inner(sources, map, None);
    }

    pub(crate) fn stamp_lingering_sources_with_smoke(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        smokes: &SmokeCloudStore,
    ) {
        self.stamp_lingering_sources_inner(sources, map, Some(smokes));
    }

    fn stamp_lingering_sources_inner(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        smokes: Option<&SmokeCloudStore>,
    ) {
        let size = self.size;
        let los = match smokes {
            Some(smokes) => LineOfSight::with_smoke(map, smokes),
            None => LineOfSight::new(map),
        };
        for source in sources {
            if smokes
                .map(|smokes| smokes.point_inside(source.x, source.y))
                .unwrap_or(false)
            {
                continue;
            }
            let Some(grid) = self.grids.get_mut(&source.owner) else {
                continue;
            };
            stamp_sight_at(grid, size, source.x, source.y, source.sight_tiles, &los);
        }
    }

    /// Whether `player` can currently see the tile `(tx, ty)`.
    pub fn is_visible(&self, player: u32, tx: u32, ty: u32) -> bool {
        if tx >= self.size || ty >= self.size {
            return false;
        }
        match self.grids.get(&player) {
            Some(g) => g[(ty * self.size + tx) as usize],
            None => false,
        }
    }

    /// Build a temporary fog view where `viewer` can see every tile visible to any of `players`.
    pub fn union_for(&self, viewer: u32, players: &[u32]) -> Self {
        let cells = (self.size * self.size) as usize;
        let mut union = vec![false; cells];
        for player in players {
            let Some(grid) = self.grids.get(player) else {
                continue;
            };
            for (dst, src) in union.iter_mut().zip(grid.iter()) {
                *dst = *dst || *src;
            }
        }

        let mut fog = Fog::new(self.size);
        fog.grids.insert(viewer, union);
        fog
    }

    /// Whether a grid has been allocated for `player`.
    pub fn has_grid(&self, player: u32) -> bool {
        self.grids.contains_key(&player)
    }

    pub(crate) fn visible_tiles_for(&self, player: u32) -> Vec<u8> {
        self.grids
            .get(&player)
            .map(|grid| grid.iter().map(|visible| u8::from(*visible)).collect())
            .unwrap_or_default()
    }

    /// Whether `player` can currently see the world-pixel point `(x, y)`.
    pub fn is_visible_world(&self, player: u32, x: f32, y: f32) -> bool {
        let ts = config::TILE_SIZE as f32;
        if x < 0.0 || y < 0.0 {
            return false;
        }
        let tx = (x / ts).floor() as i64;
        let ty = (y / ts).floor() as i64;
        if tx < 0 || ty < 0 || tx as u32 >= self.size || ty as u32 >= self.size {
            return false;
        }
        self.is_visible(player, tx as u32, ty as u32)
    }
}

/// Mark every tile within an entity's sight radius (a filled circle in tile space) as visible.
fn stamp_sight(grid: &mut [bool], size: u32, e: &Entity, los: &LineOfSight<'_>) {
    stamp_sight_at(grid, size, e.pos_x, e.pos_y, e.sight_tiles(), los);
}

fn stamp_sight_at(
    grid: &mut [bool],
    size: u32,
    x: f32,
    y: f32,
    sight_tiles: u32,
    los: &LineOfSight<'_>,
) {
    let r = sight_tiles as i32;
    if r <= 0 {
        return;
    }
    let ts = config::TILE_SIZE as f32;
    let cx = (x / ts).floor() as i32;
    let cy = (y / ts).floor() as i32;
    let r2 = r * r;
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let tx = cx + dx;
            let ty = cy + dy;
            if tx < 0 || ty < 0 || tx as u32 >= size || ty as u32 >= size {
                continue;
            }
            if !los.tile_visible_from_world((x, y), (tx as u32, ty as u32)) {
                continue;
            }
            grid[(ty as u32 * size + tx as u32) as usize] = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::{EntityKind, EntityStore};
    use crate::protocol::terrain;

    fn map_with_rock_at(tile: (u32, u32)) -> Map {
        let size = 8;
        let mut terrain = vec![terrain::GRASS; (size * size) as usize];
        terrain[(tile.1 * size + tile.0) as usize] = terrain::ROCK;
        Map {
            size,
            terrain,
            starts: vec![(1, 1)],
            expansion_sites: Vec::new(),
        }
    }

    #[test]
    fn stone_blocks_authoritative_fog_behind_it() {
        let map = map_with_rock_at((3, 2));
        let mut entities = EntityStore::new();
        let origin = map.tile_center(1, 2);
        entities
            .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
            .expect("worker should spawn");
        let mut fog = Fog::new(map.size);

        fog.recompute(&[1], &entities, &map);

        assert!(fog.is_visible(1, 3, 2));
        assert!(!fog.is_visible(1, 4, 2));
    }

    #[test]
    fn smoke_blocks_authoritative_fog_behind_it_but_reveals_cloud_edge() {
        let map = map_with_rock_at((7, 7));
        let mut entities = EntityStore::new();
        let origin = map.tile_center(1, 2);
        entities
            .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
            .expect("worker should spawn");
        let mut smokes = SmokeCloudStore::new();
        let smoke = map.tile_center(3, 2);
        smokes
            .spawn(smoke.0, smoke.1, 1.0, 100, 0)
            .expect("smoke should spawn");
        let mut fog = Fog::new(map.size);

        fog.recompute_with_smoke(&[1], &entities, &map, &smokes);

        assert!(fog.is_visible(1, 3, 2));
        assert!(!fog.is_visible(1, 5, 2));
    }

    #[test]
    fn unit_inside_smoke_does_not_stamp_vision() {
        let map = map_with_rock_at((7, 7));
        let mut entities = EntityStore::new();
        let origin = map.tile_center(2, 2);
        entities
            .spawn_unit(1, EntityKind::Worker, origin.0, origin.1)
            .expect("worker should spawn");
        let mut smokes = SmokeCloudStore::new();
        smokes
            .spawn(origin.0, origin.1, 1.0, 100, 0)
            .expect("smoke should spawn");
        let mut fog = Fog::new(map.size);

        fog.recompute_with_smoke(&[1], &entities, &map, &smokes);

        assert!(!fog.is_visible(1, 2, 2));
        assert!(!fog.is_visible(1, 3, 2));
    }
}
