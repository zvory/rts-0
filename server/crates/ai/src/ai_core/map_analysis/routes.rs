use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{tile_center_world, tile_index, AiMapAnalysis, AiTile, NEIGHBORS};

const CARDINAL_COST: u32 = 10;
const DIAGONAL_COST: u32 = 14;

impl AiMapAnalysis {
    /// Returns sparse, passable world-space waypoints for a compact vehicle group.
    /// Two-tile clearance is preferred; narrow one-tile passages remain a fallback.
    pub(crate) fn compact_group_route(
        &self,
        from: (f32, f32),
        destination: (f32, f32),
        stride_tiles: usize,
    ) -> Vec<(f32, f32)> {
        self.route_with_clearance(from, destination, stride_tiles, 2)
            .or_else(|| self.route_with_clearance(from, destination, stride_tiles, 1))
            .unwrap_or_else(|| vec![destination])
    }

    fn route_with_clearance(
        &self,
        from: (f32, f32),
        destination: (f32, f32),
        stride_tiles: usize,
        minimum_clearance: u16,
    ) -> Option<Vec<(f32, f32)>> {
        let start = self.nearest_route_tile(from, minimum_clearance, None)?;
        let component =
            self.component_by_tile[tile_index(self.width, self.height, start.x, start.y)?];
        let goal = self.nearest_route_tile(destination, minimum_clearance, component)?;
        let start_idx = tile_index(self.width, self.height, start.x, start.y)?;
        let goal_idx = tile_index(self.width, self.height, goal.x, goal.y)?;
        let count = usize::try_from(self.width.checked_mul(self.height)?).ok()?;
        let mut costs = vec![u32::MAX; count];
        let mut parents = vec![None; count];
        let mut open = BinaryHeap::new();
        costs[start_idx] = 0;
        open.push(Reverse((
            route_heuristic(start, goal),
            0_u32,
            start.x,
            start.y,
        )));

        while let Some(Reverse((_, cost, x, y))) = open.pop() {
            let idx = tile_index(self.width, self.height, x, y)?;
            if cost != costs[idx] {
                continue;
            }
            if idx == goal_idx {
                break;
            }
            for (dx, dy) in NEIGHBORS {
                let nx = i64::from(x) + i64::from(dx);
                let ny = i64::from(y) + i64::from(dy);
                if nx < 0 || ny < 0 {
                    continue;
                }
                let (nx, ny) = (u32::try_from(nx).ok()?, u32::try_from(ny).ok()?);
                let Some(next_idx) = tile_index(self.width, self.height, nx, ny) else {
                    continue;
                };
                if !self.passable[next_idx]
                    || self.clearance[next_idx] < minimum_clearance
                    || self.component_by_tile[next_idx] != component
                {
                    continue;
                }
                if dx != 0 && dy != 0 && !self.diagonal_route_clear(x, y, dx, dy, minimum_clearance)
                {
                    continue;
                }
                let step = if dx == 0 || dy == 0 {
                    CARDINAL_COST
                } else {
                    DIAGONAL_COST
                };
                let clearance_penalty = u32::from(4_u16.saturating_sub(self.clearance[next_idx]));
                let next_cost = cost.saturating_add(step + clearance_penalty);
                if next_cost >= costs[next_idx] {
                    continue;
                }
                costs[next_idx] = next_cost;
                parents[next_idx] = Some(idx);
                let next = AiTile { x: nx, y: ny };
                open.push(Reverse((
                    next_cost + route_heuristic(next, goal),
                    next_cost,
                    nx,
                    ny,
                )));
            }
        }
        if costs[goal_idx] == u32::MAX {
            return None;
        }
        let mut indices = vec![goal_idx];
        let mut cursor = goal_idx;
        while cursor != start_idx {
            cursor = parents[cursor]?;
            indices.push(cursor);
        }
        indices.reverse();
        let stride = stride_tiles.max(1);
        let mut route = indices
            .iter()
            .skip(stride)
            .step_by(stride)
            .map(|idx| {
                let x = *idx as u32 % self.width;
                let y = *idx as u32 / self.width;
                tile_center_world(AiTile { x, y }, self.tile_size)
            })
            .collect::<Vec<_>>();
        let goal_world = tile_center_world(goal, self.tile_size);
        if route.last().copied() != Some(goal_world) {
            route.push(goal_world);
        }
        Some(route)
    }

    fn nearest_route_tile(
        &self,
        point: (f32, f32),
        minimum_clearance: u16,
        component: Option<u32>,
    ) -> Option<AiTile> {
        let tile_size = self.tile_size.max(1) as f32;
        let center_x = (point.0 / tile_size)
            .floor()
            .clamp(0.0, self.width.saturating_sub(1) as f32) as i32;
        let center_y = (point.1 / tile_size)
            .floor()
            .clamp(0.0, self.height.saturating_sub(1) as f32) as i32;
        for radius in 0_i32..=12 {
            let mut candidates = Vec::new();
            for y in center_y - radius..=center_y + radius {
                for x in center_x - radius..=center_x + radius {
                    if x < 0 || y < 0 || (x - center_x).abs().max((y - center_y).abs()) != radius {
                        continue;
                    }
                    let (x, y) = (u32::try_from(x).ok()?, u32::try_from(y).ok()?);
                    let Some(idx) = tile_index(self.width, self.height, x, y) else {
                        continue;
                    };
                    if self.passable[idx]
                        && self.clearance[idx] >= minimum_clearance
                        && component
                            .is_none_or(|wanted| self.component_by_tile[idx] == Some(wanted))
                    {
                        candidates.push(AiTile { x, y });
                    }
                }
            }
            candidates.sort_by_key(|tile| {
                (
                    (tile.x as i32 - center_x).pow(2) + (tile.y as i32 - center_y).pow(2),
                    tile.y,
                    tile.x,
                )
            });
            if let Some(tile) = candidates.first() {
                return Some(*tile);
            }
        }
        None
    }

    fn diagonal_route_clear(&self, x: u32, y: u32, dx: i32, dy: i32, clearance: u16) -> bool {
        [
            (i64::from(x) + i64::from(dx), i64::from(y)),
            (i64::from(x), i64::from(y) + i64::from(dy)),
        ]
        .into_iter()
        .all(|(x, y)| {
            let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
                return false;
            };
            tile_index(self.width, self.height, x, y)
                .is_some_and(|idx| self.passable[idx] && self.clearance[idx] >= clearance)
        })
    }
}

fn route_heuristic(from: AiTile, to: AiTile) -> u32 {
    let dx = from.x.abs_diff(to.x);
    let dy = from.y.abs_diff(to.y);
    let diagonal = dx.min(dy);
    DIAGONAL_COST * diagonal + CARDINAL_COST * (dx.max(dy) - diagonal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_core::map_analysis::AiMapAnalysisKey;

    #[test]
    fn compact_group_route_uses_the_gap_in_a_blocking_wall() {
        let (width, height, tile_size) = (9_u32, 7_u32, 32_u32);
        let mut passable = vec![true; (width * height) as usize];
        for y in 0..height {
            if y != 5 {
                passable[(y * width + 4) as usize] = false;
            }
        }
        let component_by_tile = passable
            .iter()
            .map(|open| open.then_some(0))
            .collect::<Vec<_>>();
        let analysis = AiMapAnalysis {
            key: AiMapAnalysisKey {
                width,
                height,
                tile_size,
                terrain_hash: 0,
                starts_hash: 0,
                resources_hash: 0,
            },
            width,
            height,
            tile_size,
            passable,
            line_of_sight_blocked: vec![false; (width * height) as usize],
            clearance: vec![2; (width * height) as usize],
            component_by_tile,
            components: Vec::new(),
            region_by_tile: vec![None; (width * height) as usize],
            regions: Vec::new(),
            chokes: Vec::new(),
            starts: Vec::new(),
            resource_clusters: Vec::new(),
        };

        let route = analysis.compact_group_route((48.0, 48.0), (240.0, 48.0), 1);

        assert!(route.iter().any(|point| {
            (point.0 / tile_size as f32).floor() as u32 == 4
                && (point.1 / tile_size as f32).floor() as u32 == 5
        }));
        assert!(route.iter().all(|point| analysis.tile_is_passable(
            (point.0 / tile_size as f32).floor() as u32,
            (point.1 / tile_size as f32).floor() as u32,
        )));
    }
}
