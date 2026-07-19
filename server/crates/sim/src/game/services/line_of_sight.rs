//! Line-of-sight queries over map terrain plus optional dynamic blockers.
//!
//! Stone blocks both vision and ranged attacks. Fog can also supply current building-footprint
//! blockers, while smoke supplies dynamic cloud blockers. This service owns the tile raycast so
//! fog, combat, and future terrain features share one interpretation of "can see/shoot through".

use crate::config;
use crate::game::map::Map;
use crate::game::smoke::SmokeCloudStore;
use crate::rules::terrain;

#[derive(Clone, Copy)]
pub(crate) struct LineOfSight<'a> {
    map: &'a Map,
    smokes: Option<&'a SmokeCloudStore>,
    building_blockers: Option<&'a [bool]>,
    static_blockers: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RaycastStep {
    Clear,
    ReachedTarget,
    Blocked,
}

impl<'a> LineOfSight<'a> {
    #[cfg(test)]
    pub(crate) fn new(map: &'a Map) -> Self {
        LineOfSight {
            map,
            smokes: None,
            building_blockers: None,
            static_blockers: true,
        }
    }

    pub(crate) fn with_smoke(map: &'a Map, smokes: &'a SmokeCloudStore) -> Self {
        LineOfSight {
            map,
            smokes: Some(smokes),
            building_blockers: None,
            static_blockers: true,
        }
    }

    pub(crate) fn with_smoke_only(map: &'a Map, smokes: &'a SmokeCloudStore) -> Self {
        LineOfSight {
            map,
            smokes: Some(smokes),
            building_blockers: None,
            static_blockers: false,
        }
    }

    pub(crate) fn with_building_blockers(map: &'a Map, building_blockers: &'a [bool]) -> Self {
        LineOfSight {
            map,
            smokes: None,
            building_blockers: Some(building_blockers),
            static_blockers: true,
        }
    }

    pub(crate) fn with_smoke_and_building_blockers(
        map: &'a Map,
        smokes: &'a SmokeCloudStore,
        building_blockers: &'a [bool],
    ) -> Self {
        LineOfSight {
            map,
            smokes: Some(smokes),
            building_blockers: Some(building_blockers),
            static_blockers: true,
        }
    }

    /// True when no opaque terrain lies on the segment between two world-pixel points.
    /// The origin tile is ignored; the target tile is treated as blocking if it is opaque.
    pub(crate) fn clear_between_world_points(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        self.raycast_clear(from, to, false, false)
    }

    /// True when `tile` is visible from a world-pixel origin. The target tile itself may be
    /// opaque so units can reveal the face of a stone wall without seeing past it.
    pub(crate) fn tile_visible_from_world(&self, from: (f32, f32), tile: (u32, u32)) -> bool {
        if tile.0 >= self.map.size || tile.1 >= self.map.size {
            return false;
        }
        self.raycast_clear(from, self.map.tile_center(tile.0, tile.1), true, true)
    }

    fn raycast_clear(
        &self,
        from: (f32, f32),
        to: (f32, f32),
        allow_opaque_target: bool,
        allow_dynamic_target: bool,
    ) -> bool {
        let (from_x, from_y) = from;
        let (to_x, to_y) = to;
        if !from_x.is_finite() || !from_y.is_finite() || !to_x.is_finite() || !to_y.is_finite() {
            return false;
        }
        if from_x < 0.0 || from_y < 0.0 || to_x < 0.0 || to_y < 0.0 {
            return false;
        }
        let world_size = self.map.world_size_px();
        if from_x >= world_size || from_y >= world_size || to_x >= world_size || to_y >= world_size
        {
            return false;
        }
        if let Some(smokes) = self.smokes {
            let blocked = if allow_dynamic_target {
                smokes.segment_blocked_allowing_target_cloud(from, to)
            } else {
                smokes.segment_blocked(from, to)
            };
            if blocked {
                return false;
            }
        }

        let ts = config::TILE_SIZE as f32;
        let start = self.map.tile_of(from_x, from_y);
        let target = self.map.tile_of(to_x, to_y);
        if start == target {
            return allow_opaque_target || !self.tile_blocks(target);
        }

        let mut tx = start.0 as i32;
        let mut ty = start.1 as i32;
        let target_x = target.0 as i32;
        let target_y = target.1 as i32;

        let x0 = from_x / ts;
        let y0 = from_y / ts;
        let x1 = to_x / ts;
        let y1 = to_y / ts;
        let dx = x1 - x0;
        let dy = y1 - y0;

        let step_x = axis_step(dx);
        let step_y = axis_step(dy);
        let mut t_max_x = first_boundary_t(x0, tx, dx, step_x);
        let mut t_max_y = first_boundary_t(y0, ty, dy, step_y);
        let t_delta_x = if step_x == 0 {
            f32::INFINITY
        } else {
            1.0 / dx.abs()
        };
        let t_delta_y = if step_y == 0 {
            f32::INFINITY
        } else {
            1.0 / dy.abs()
        };

        while tx != target_x || ty != target_y {
            if t_max_x < t_max_y {
                tx += step_x;
                t_max_x += t_delta_x;
                match self.trace_step((tx, ty), target, allow_opaque_target) {
                    RaycastStep::Clear => {}
                    RaycastStep::ReachedTarget => return true,
                    RaycastStep::Blocked => return false,
                }
            } else if t_max_y < t_max_x {
                ty += step_y;
                t_max_y += t_delta_y;
                match self.trace_step((tx, ty), target, allow_opaque_target) {
                    RaycastStep::Clear => {}
                    RaycastStep::ReachedTarget => return true,
                    RaycastStep::Blocked => return false,
                }
            } else {
                let next_tx = tx + step_x;
                let next_ty = ty + step_y;
                match self.trace_step((next_tx, ty), target, allow_opaque_target) {
                    RaycastStep::Clear => {}
                    RaycastStep::ReachedTarget => return true,
                    RaycastStep::Blocked => return false,
                }
                match self.trace_step((tx, next_ty), target, allow_opaque_target) {
                    RaycastStep::Clear => {}
                    RaycastStep::ReachedTarget => return true,
                    RaycastStep::Blocked => return false,
                }
                tx = next_tx;
                ty = next_ty;
                match self.trace_step((tx, ty), target, allow_opaque_target) {
                    RaycastStep::Clear => {}
                    RaycastStep::ReachedTarget => return true,
                    RaycastStep::Blocked => return false,
                }
                t_max_x += t_delta_x;
                t_max_y += t_delta_y;
            }
        }

        true
    }

    fn trace_step(
        &self,
        tile: (i32, i32),
        target: (u32, u32),
        allow_opaque_target: bool,
    ) -> RaycastStep {
        if !self.map.in_bounds(tile.0, tile.1) {
            return RaycastStep::Blocked;
        }
        let current = (tile.0 as u32, tile.1 as u32);
        if current == target {
            if allow_opaque_target || !self.tile_blocks(current) {
                RaycastStep::ReachedTarget
            } else {
                RaycastStep::Blocked
            }
        } else if self.tile_blocks(current) {
            RaycastStep::Blocked
        } else {
            RaycastStep::Clear
        }
    }

    fn tile_blocks(&self, tile: (u32, u32)) -> bool {
        if self.static_blockers
            && terrain::blocks_line_of_sight(self.map.terrain_at(tile.0, tile.1))
        {
            return true;
        }
        let Some(blockers) = self.building_blockers else {
            return false;
        };
        let idx = (tile.1 * self.map.size + tile.0) as usize;
        blockers.get(idx).copied().unwrap_or(false)
    }
}

fn first_boundary_t(coord: f32, tile: i32, delta: f32, step: i32) -> f32 {
    match step {
        1 => ((tile + 1) as f32 - coord) / delta,
        -1 => (coord - tile as f32) / -delta,
        _ => f32::INFINITY,
    }
}

fn axis_step(delta: f32) -> i32 {
    if delta > 0.0 {
        1
    } else if delta < 0.0 {
        -1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::terrain as wire_terrain;

    fn map_with_rock_at(tile: (u32, u32)) -> Map {
        let size = 8;
        let mut terrain = vec![wire_terrain::GRASS; (size * size) as usize];
        terrain[(tile.1 * size + tile.0) as usize] = wire_terrain::ROCK;
        Map {
            size,
            terrain,
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        }
    }

    fn flat_map(size: u32) -> Map {
        Map {
            size,
            terrain: vec![wire_terrain::GRASS; (size * size) as usize],
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        }
    }

    #[test]
    fn stone_blocks_world_point_line_of_sight() {
        let map = map_with_rock_at((3, 2));
        let los = LineOfSight::new(&map);

        assert!(!los.clear_between_world_points(map.tile_center(1, 2), map.tile_center(5, 2),));
        assert!(los.clear_between_world_points(map.tile_center(1, 1), map.tile_center(5, 1),));
    }

    #[test]
    fn fog_can_reveal_stone_but_not_terrain_behind_it() {
        let map = map_with_rock_at((3, 2));
        let los = LineOfSight::new(&map);
        let origin = map.tile_center(1, 2);

        assert!(los.tile_visible_from_world(origin, (3, 2)));
        assert!(!los.tile_visible_from_world(origin, (4, 2)));
    }

    #[test]
    fn corner_crossing_does_not_slip_between_two_stones() {
        let size = 8;
        let mut terrain = vec![wire_terrain::GRASS; (size * size) as usize];
        terrain[2 * size as usize + 3] = wire_terrain::ROCK;
        terrain[3 * size as usize + 2] = wire_terrain::ROCK;
        let map = Map {
            size,
            terrain,
            starts: vec![(1, 1)],
            base_sites: Vec::new(),
        };
        let los = LineOfSight::new(&map);

        assert!(!los.clear_between_world_points(map.tile_center(2, 2), map.tile_center(3, 3),));
    }

    #[test]
    fn grid_corner_target_near_map_edge_does_not_step_past_endpoint() {
        let map = flat_map(126);
        let los = LineOfSight::new(&map);

        assert!(los.clear_between_world_points((213.959, 3941.309), (32.0, 3968.0)));
        assert!(los.clear_between_world_points((213.959, 3941.309), (160.0, 4000.0)));
    }
}
