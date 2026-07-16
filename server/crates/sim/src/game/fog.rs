//! Per-player fog of war. See `docs/design/server-sim.md` (`fog.rs`).
//!
//! The server is authoritative about visibility: at 15 Hz we recompute, for every player, a
//! boolean grid of which tiles that player can see in the latest visibility sample. A tile is
//! visible if it falls
//! within the sight area of any of that player's entities (`sight_tiles`) and the line from
//! the entity to that tile is not blocked by stone, smoke, or sight-blocking building footprints.
//! Units stamp a circle from their body center; buildings stamp their full footprint plus
//! `sight_tiles` around the footprint edge. Scout Planes add a separate team aerial sight pass
//! that ignores stone and building blockers but still respects active smoke clouds. The snapshot
//! layer uses this to withhold neutral/enemy entities standing on non-visible tiles, making the fog
//! cheat-proof. During the same rebuild, firing-reveal stamps retain bounded entity-level
//! provenance so combat can distinguish ordinary visibility from reveal-only visibility without
//! maintaining a duplicate fog grid.
//!
//! Note the server only needs *currently visible* — the client maintains the "explored but
//! not currently visible" dimming locally (see `docs/design/client-ui.md`). So this module tracks only
//! the sampled visible set.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::config;
use crate::game::entity::{blocks_line_of_sight, Entity, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::line_of_sight::LineOfSight;
use crate::game::services::occupancy::building_footprint;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use serde::{Deserialize, Serialize};

mod reveal_provenance;
pub(in crate::game) use reveal_provenance::FiringRevealVisibility;

/// Temporary sight left behind by an owned unit/building after it dies.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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

/// Visible-tile grids, one per player. Recomputed from scratch at 15 Hz and held between samples.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Fog {
    size: u32,
    /// player id -> row-major visibility grid (`true` = visible in the latest sample).
    grids: HashMap<u32, Vec<bool>>,
    /// Sampled firing-reveal provenance, rebuilt atomically with `grids`.
    ///
    /// The nested keys are viewer id -> revealed entity id. `reveal_only` means the entity's
    /// tile was dark immediately before its firing reveal was stamped. Combat uses this instead
    /// of trying to infer provenance from the flattened actionable grid.
    firing_reveal_visibility: BTreeMap<u32, BTreeMap<u32, FiringRevealVisibility>>,
}

impl Fog {
    pub fn new(size: u32) -> Self {
        Fog {
            size,
            grids: HashMap::new(),
            firing_reveal_visibility: BTreeMap::new(),
        }
    }

    pub(in crate::game) fn from_checkpoint_grids(
        size: u32,
        grids: BTreeMap<u32, Vec<bool>>,
        firing_reveal_visibility: BTreeMap<u32, BTreeMap<u32, FiringRevealVisibility>>,
    ) -> Self {
        Fog {
            size,
            grids: grids.into_iter().collect(),
            firing_reveal_visibility,
        }
    }

    pub(in crate::game) fn checkpoint_size(&self) -> u32 {
        self.size
    }

    pub(in crate::game) fn checkpoint_grids(&self) -> BTreeMap<u32, Vec<bool>> {
        self.grids
            .iter()
            .map(|(&player, grid)| (player, grid.clone()))
            .collect()
    }

    pub(in crate::game) fn checkpoint_firing_reveal_visibility(
        &self,
    ) -> BTreeMap<u32, BTreeMap<u32, FiringRevealVisibility>> {
        self.firing_reveal_visibility.clone()
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
        self.firing_reveal_visibility.clear();
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

        let building_mask = BuildingLosMask::new(store, map);
        let los = match smokes {
            Some(smokes) => {
                LineOfSight::with_smoke_and_building_blockers(map, smokes, &building_mask.blockers)
            }
            None => LineOfSight::with_building_blockers(map, &building_mask.blockers),
        };
        for e in store.iter() {
            if e.owner == 0 {
                continue; // neutral resource nodes do not grant a player vision
            }
            if !entity_grants_standard_sight(e) {
                continue;
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
            stamp_sight(grid, size, e, map, &los);
        }
        reveal_visible_building_footprints(&mut self.grids, &building_mask);
    }

    /// Add temporary death-vision sight sources to already-recomputed grids.
    #[allow(dead_code)]
    pub(crate) fn stamp_lingering_sources(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        store: &EntityStore,
    ) {
        self.stamp_lingering_sources_inner(sources, map, store, None);
    }

    pub(in crate::game) fn stamp_lingering_sources_for_teams_with_smoke(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        store: &EntityStore,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
    ) {
        self.stamp_lingering_sources_for_teams_inner(sources, map, store, smokes, teams);
    }

    fn stamp_lingering_sources_inner(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        store: &EntityStore,
        smokes: Option<&SmokeCloudStore>,
    ) {
        let size = self.size;
        let building_mask = BuildingLosMask::new(store, map);
        let los = match smokes {
            Some(smokes) => {
                LineOfSight::with_smoke_and_building_blockers(map, smokes, &building_mask.blockers)
            }
            None => LineOfSight::with_building_blockers(map, &building_mask.blockers),
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
        reveal_visible_building_footprints(&mut self.grids, &building_mask);
    }

    fn stamp_lingering_sources_for_teams_inner(
        &mut self,
        sources: &[LingeringSightSource],
        map: &Map,
        store: &EntityStore,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
    ) {
        let size = self.size;
        let building_mask = BuildingLosMask::new(store, map);
        let los =
            LineOfSight::with_smoke_and_building_blockers(map, smokes, &building_mask.blockers);
        for source in sources {
            if smokes.point_inside(source.x, source.y) {
                continue;
            }
            let mut recipients = teams.same_team_player_ids(source.owner);
            if recipients.is_empty() {
                recipients.push(source.owner);
            }
            for recipient in recipients {
                let Some(grid) = self.grids.get_mut(&recipient) else {
                    continue;
                };
                stamp_sight_at(grid, size, source.x, source.y, source.sight_tiles, &los);
            }
        }
        reveal_visible_building_footprints(&mut self.grids, &building_mask);
    }

    pub(in crate::game) fn stamp_scout_plane_sources_for_teams_with_smoke(
        &mut self,
        map: &Map,
        store: &EntityStore,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
    ) {
        let size = self.size;
        let building_mask = BuildingLosMask::new(store, map);
        let los = LineOfSight::with_smoke_only(map, smokes);
        let mut seen_owners = BTreeSet::new();
        for plane in store.iter() {
            if plane.kind != EntityKind::ScoutPlane
                || plane.owner == 0
                || plane.hp == 0
                || smokes.point_inside(plane.pos_x, plane.pos_y)
                || !seen_owners.insert(plane.owner)
            {
                continue;
            }
            let mut recipients = teams.same_team_player_ids(plane.owner);
            if recipients.is_empty() {
                recipients.push(plane.owner);
            }
            for recipient in recipients {
                let Some(grid) = self.grids.get_mut(&recipient) else {
                    continue;
                };
                stamp_sight_at(
                    grid,
                    size,
                    plane.pos_x,
                    plane.pos_y,
                    plane.sight_tiles(),
                    &los,
                );
            }
        }
        reveal_visible_building_footprints(&mut self.grids, &building_mask);
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

fn entity_grants_standard_sight(entity: &Entity) -> bool {
    // Scout Plane aerial fog has its own smoke-only team sight pass, so it must not also fall
    // through to ordinary ground line-of-sight stamping.
    entity.kind != EntityKind::ScoutPlane
}

fn stamp_point(grid: &mut [bool], size: u32, x: f32, y: f32) {
    let Some(tile) = world_tile_index(size, x, y) else {
        return;
    };
    if let Some(visible) = grid.get_mut(tile as usize) {
        *visible = true;
    }
}

fn world_tile_index(size: u32, x: f32, y: f32) -> Option<u32> {
    let ts = config::TILE_SIZE as f32;
    if !x.is_finite() || !y.is_finite() || x < 0.0 || y < 0.0 {
        return None;
    }
    let tx = (x / ts).floor() as i64;
    let ty = (y / ts).floor() as i64;
    if tx < 0 || ty < 0 || tx as u32 >= size || ty as u32 >= size {
        return None;
    }
    Some(ty as u32 * size + tx as u32)
}

/// Mark every tile within an entity's sight area as visible.
fn stamp_sight(grid: &mut [bool], size: u32, e: &Entity, map: &Map, los: &LineOfSight<'_>) {
    if e.is_building() {
        stamp_building_sight(grid, size, e, map, los);
        return;
    }
    stamp_sight_at(grid, size, e.pos_x, e.pos_y, e.sight_tiles(), los);
}

fn stamp_building_sight(
    grid: &mut [bool],
    size: u32,
    e: &Entity,
    map: &Map,
    los: &LineOfSight<'_>,
) {
    let r = e.sight_tiles() as i32;
    if r <= 0 {
        return;
    }
    let footprint = building_footprint(map, e);
    for (origin_tx, origin_ty) in footprint {
        if origin_tx >= size || origin_ty >= size {
            continue;
        }
        let origin = map.tile_center(origin_tx, origin_ty);
        for dy in -r..=r {
            for dx in -r..=r {
                let tx = origin_tx as i32 + dx;
                let ty = origin_ty as i32 + dy;
                if tx < 0 || ty < 0 || tx as u32 >= size || ty as u32 >= size {
                    continue;
                }
                if !los.tile_visible_from_world(origin, (tx as u32, ty as u32)) {
                    continue;
                }
                grid[(ty as u32 * size + tx as u32) as usize] = true;
            }
        }
    }
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

struct BuildingLosMask {
    blockers: Vec<bool>,
    footprints: Vec<Vec<usize>>,
}

impl BuildingLosMask {
    fn new(store: &EntityStore, map: &Map) -> Self {
        let cells = (map.size * map.size) as usize;
        let mut blockers = vec![false; cells];
        let mut footprints = Vec::new();
        for entity in store.iter() {
            if entity.hp == 0 || !blocks_line_of_sight(entity.kind) {
                continue;
            }
            let footprint = building_footprint(map, entity)
                .into_iter()
                .filter(|(tx, ty)| *tx < map.size && *ty < map.size)
                .map(|(tx, ty)| (ty * map.size + tx) as usize)
                .collect::<Vec<_>>();
            if footprint.is_empty() {
                continue;
            }
            for idx in &footprint {
                blockers[*idx] = true;
            }
            footprints.push(footprint);
        }
        Self {
            blockers,
            footprints,
        }
    }
}

fn reveal_visible_building_footprints(
    grids: &mut HashMap<u32, Vec<bool>>,
    building_mask: &BuildingLosMask,
) {
    if building_mask.footprints.is_empty() {
        return;
    }
    for grid in grids.values_mut() {
        for footprint in &building_mask.footprints {
            if footprint
                .iter()
                .any(|idx| grid.get(*idx).copied().unwrap_or(false))
            {
                for idx in footprint {
                    if let Some(visible) = grid.get_mut(*idx) {
                        *visible = true;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
