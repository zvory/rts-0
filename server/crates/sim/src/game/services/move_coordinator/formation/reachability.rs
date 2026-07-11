use std::collections::{BTreeMap, VecDeque};

use super::super::{Occupancy, StaticPathingRelation};
use super::FormationUnit;
use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, EntityKind};
use crate::game::map::Map;
use crate::rules::terrain::{self, TerrainKind};

const NEIGHBORS: [(i32, i32); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

pub(in crate::game::services::move_coordinator) struct FormationReachability<'a> {
    map: &'a Map,
    occ: &'a Occupancy<'a>,
    relation: StaticPathingRelation,
    by_kind: BTreeMap<EntityKind, ReachabilityGrid>,
}

impl<'a> FormationReachability<'a> {
    pub(in crate::game::services::move_coordinator) fn new(
        map: &'a Map,
        occ: &'a Occupancy<'a>,
        relation: StaticPathingRelation,
    ) -> Self {
        Self {
            map,
            occ,
            relation,
            by_kind: BTreeMap::new(),
        }
    }

    pub(in crate::game::services::move_coordinator) fn can_reach(
        &mut self,
        unit: &FormationUnit,
        tile: (u32, u32),
    ) -> bool {
        let start = self.map.tile_of(unit.pos.0, unit.pos.1);
        if start == tile {
            return true;
        }
        let map = self.map;
        let occ = self.occ;
        let relation = &self.relation;
        let grid = self
            .by_kind
            .entry(unit.kind)
            .or_insert_with(|| ReachabilityGrid::build(map, occ, relation, unit.kind));
        let Some(can_reach) = grid.can_reach(start, tile) else {
            // If the unit cannot make useful progress at all, keep the legacy goal so the
            // ordinary path phase can surface PathFailed instead of turning the order into a no-op.
            return true;
        };
        can_reach
    }
}

struct ReachabilityGrid {
    size: u32,
    passable: Vec<bool>,
    components: Vec<u32>,
}

impl ReachabilityGrid {
    fn build(
        map: &Map,
        occ: &Occupancy<'_>,
        relation: &StaticPathingRelation,
        kind: EntityKind,
    ) -> Self {
        let len = (map.size * map.size) as usize;
        let mut passable = vec![false; len];
        for ty in 0..map.size {
            for tx in 0..map.size {
                let idx = map.index(tx, ty);
                passable[idx] =
                    reachability_tile_passable(map, occ, relation, kind, tx as i32, ty as i32);
            }
        }

        let mut components = vec![0; len];
        let mut next_component = 1u32;
        let mut queue = VecDeque::new();
        for ty in 0..map.size {
            for tx in 0..map.size {
                let idx = map.index(tx, ty);
                if !passable[idx] || components[idx] != 0 {
                    continue;
                }
                components[idx] = next_component;
                queue.push_back((tx as i32, ty as i32));

                while let Some((cx, cy)) = queue.pop_front() {
                    for (dx, dy) in NEIGHBORS {
                        let nx = cx + dx;
                        let ny = cy + dy;
                        if !in_bounds(map.size, nx, ny)
                            || !step_allowed(map, &passable, cx, cy, dx, dy)
                        {
                            continue;
                        }
                        let next_idx = map.index(nx as u32, ny as u32);
                        if components[next_idx] != 0 {
                            continue;
                        }
                        components[next_idx] = next_component;
                        queue.push_back((nx, ny));
                    }
                }

                next_component = next_component.saturating_add(1);
            }
        }

        Self {
            size: map.size,
            passable,
            components,
        }
    }

    fn component(&self, tile: (u32, u32)) -> Option<u32> {
        if tile.0 >= self.size || tile.1 >= self.size {
            return None;
        }
        let component = self.components[self.index(tile.0, tile.1)];
        (component != 0).then_some(component)
    }

    fn can_reach(&self, start: (u32, u32), target: (u32, u32)) -> Option<bool> {
        let Some(target_component) = self.component(target) else {
            return Some(false);
        };
        if let Some(start_component) = self.component(start) {
            return Some(start_component == target_component);
        }
        let (sx, sy) = (start.0 as i32, start.1 as i32);
        let mut found_progress_component = false;
        for (dx, dy) in NEIGHBORS {
            let nx = sx + dx;
            let ny = sy + dy;
            if !in_bounds(self.size, nx, ny) || !step_allowed_grid(self, sx, sy, dx, dy) {
                continue;
            }
            if let Some(component) = self.component((nx as u32, ny as u32)) {
                found_progress_component = true;
                if component == target_component {
                    return Some(true);
                }
            }
        }
        found_progress_component.then_some(false)
    }

    fn index(&self, tx: u32, ty: u32) -> usize {
        (ty * self.size + tx) as usize
    }
}

fn reachability_tile_passable(
    map: &Map,
    occ: &Occupancy<'_>,
    relation: &StaticPathingRelation,
    kind: EntityKind,
    tx: i32,
    ty: i32,
) -> bool {
    let radius = config::unit_radius_tiles(kind) as i32;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if !single_tile_passable(map, occ, relation, kind, tx + dx, ty + dy) {
                return false;
            }
        }
    }

    if uses_oriented_vehicle_body(kind) {
        let nw = !single_tile_passable(map, occ, relation, kind, tx - 1, ty - 1);
        let ne = !single_tile_passable(map, occ, relation, kind, tx + 1, ty - 1);
        let sw = !single_tile_passable(map, occ, relation, kind, tx - 1, ty + 1);
        let se = !single_tile_passable(map, occ, relation, kind, tx + 1, ty + 1);
        if (nw && se) || (ne && sw) {
            return false;
        }
    }

    true
}

fn single_tile_passable(
    map: &Map,
    occ: &Occupancy<'_>,
    relation: &StaticPathingRelation,
    kind: EntityKind,
    tx: i32,
    ty: i32,
) -> bool {
    if !map.in_bounds(tx, ty) {
        return false;
    }
    let Some(terrain_kind) = TerrainKind::from_map_code(map.terrain_at(tx as u32, ty as u32))
    else {
        return false;
    };
    terrain::movement_allowed(kind, terrain_kind)
        && occ.passable_for_kind_and_relation(tx, ty, kind, relation)
}

fn step_allowed(map: &Map, passable: &[bool], tx: i32, ty: i32, dx: i32, dy: i32) -> bool {
    let nx = tx + dx;
    let ny = ty + dy;
    if !passable[map.index(nx as u32, ny as u32)] {
        return false;
    }
    dx == 0
        || dy == 0
        || (passable[map.index((tx + dx) as u32, ty as u32)]
            && passable[map.index(tx as u32, (ty + dy) as u32)])
}

fn step_allowed_grid(grid: &ReachabilityGrid, tx: i32, ty: i32, dx: i32, dy: i32) -> bool {
    let nx = tx + dx;
    let ny = ty + dy;
    if !grid.passable[grid.index(nx as u32, ny as u32)] {
        return false;
    }
    dx == 0
        || dy == 0
        || (grid.passable[grid.index((tx + dx) as u32, ty as u32)]
            && grid.passable[grid.index(tx as u32, (ty + dy) as u32)])
}

fn in_bounds(size: u32, tx: i32, ty: i32) -> bool {
    tx >= 0 && ty >= 0 && (tx as u32) < size && (ty as u32) < size
}
