use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::{tile_center_world, tile_distance2, tile_index, AiMapAnalysis, AiTile, NEIGHBORS};

const CARDINAL_COST: u32 = 10;
const DIAGONAL_COST: u32 = 14;

impl AiMapAnalysis {
    /// The ground route an attack on `player_id`'s start takes, from the nearest enemy start in.
    pub(crate) fn base_route_tiles(&self, player_id: u32) -> Option<&[AiTile]> {
        self.base_routes
            .iter()
            .find(|(id, _)| *id == player_id)
            .map(|(_, route)| route.as_slice())
    }

    pub(super) fn build_base_routes(&self) -> Vec<(u32, Vec<AiTile>)> {
        self.starts
            .iter()
            .filter_map(|start| {
                let enemy = self
                    .starts
                    .iter()
                    .filter(|other| other.team_id != start.team_id)
                    .min_by_key(|other| {
                        (
                            tile_distance2(other.start_tile, start.start_tile),
                            other.player_id,
                        )
                    })?;
                let own = tile_center_world(start.start_tile, self.tile_size);
                let from = tile_center_world(enemy.start_tile, self.tile_size);
                let tile_size = self.tile_size.max(1) as f32;
                let route = self
                    .compact_group_route(from, own, 1)
                    .into_iter()
                    .map(|point| {
                        AiTile::new((point.0 / tile_size) as u32, (point.1 / tile_size) as u32)
                    })
                    .collect::<Vec<_>>();
                (!route.is_empty()).then_some((start.player_id, route))
            })
            .collect()
    }

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

    /// Walking distance from `from` to every tile of its ground component, in tenths of a tile
    /// (`u32::MAX` elsewhere). Diagonal steps may not cut blocked corners.
    pub(super) fn ground_distance_field(&self, from: (f32, f32)) -> Option<Vec<u32>> {
        let start = self.nearest_route_tile(from, 1, None)?;
        let start_idx = tile_index(self.width, self.height, start.x, start.y)?;
        let component = self.component_by_tile[start_idx];
        let count = usize::try_from(self.width.checked_mul(self.height)?).ok()?;
        let mut costs = vec![u32::MAX; count];
        let mut open = BinaryHeap::new();
        costs[start_idx] = 0;
        open.push(Reverse((0_u32, start.x, start.y)));
        while let Some(Reverse((cost, x, y))) = open.pop() {
            let idx = tile_index(self.width, self.height, x, y)?;
            if cost != costs[idx] {
                continue;
            }
            for (dx, dy) in NEIGHBORS {
                let (Ok(nx), Ok(ny)) = (
                    u32::try_from(i64::from(x) + i64::from(dx)),
                    u32::try_from(i64::from(y) + i64::from(dy)),
                ) else {
                    continue;
                };
                let Some(next_idx) = tile_index(self.width, self.height, nx, ny) else {
                    continue;
                };
                if !self.passable[next_idx] || self.component_by_tile[next_idx] != component {
                    continue;
                }
                if dx != 0 && dy != 0 && !self.diagonal_route_clear(x, y, dx, dy, 1) {
                    continue;
                }
                let step = if dx == 0 || dy == 0 {
                    CARDINAL_COST
                } else {
                    DIAGONAL_COST
                };
                let next_cost = cost.saturating_add(step);
                if next_cost < costs[next_idx] {
                    costs[next_idx] = next_cost;
                    open.push(Reverse((next_cost, nx, ny)));
                }
            }
        }
        Some(costs)
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

    /// How long walking `route` from `from` takes, in tiles of open ground: a road tile counts for
    /// less, as units move half again as fast on roads.
    pub(crate) fn route_travel_tiles(&self, from: (f32, f32), route: &[(f32, f32)]) -> f32 {
        let tile_size = self.tile_size.max(1) as f32;
        let mut previous = from;
        let mut total = 0.0;
        for point in route {
            let length = (point.0 - previous.0).hypot(point.1 - previous.1) / tile_size;
            let (x, y) = (point.0 / tile_size, point.1 / tile_size);
            let on_road = x >= 0.0
                && y >= 0.0
                && tile_index(self.width, self.height, x as u32, y as u32)
                    .and_then(|idx| self.road.get(idx).copied())
                    .unwrap_or(false);
            total += if on_road {
                length / rts_rules::terrain::ROAD_MOVEMENT_SPEED_MULTIPLIER
            } else {
                length
            };
            previous = *point;
        }
        total
    }

    /// Whether the straight line from `from` to `to` keeps to passable tiles (sampled every quarter
    /// tile).
    pub(crate) fn line_is_open(&self, from: (f32, f32), to: (f32, f32)) -> bool {
        let ts = self.tile_size.max(1) as f32;
        let steps = ((to.0 - from.0).hypot(to.1 - from.1) / (0.25 * ts))
            .ceil()
            .max(1.0) as usize;
        (0..=steps).all(|i| {
            let t = i as f32 / steps as f32;
            let (x, y) = (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t);
            x >= 0.0 && y >= 0.0 && self.tile_is_passable((x / ts) as u32, (y / ts) as u32)
        })
    }

    /// How many ticks a unit moving `speed` pixels a tick takes to walk `route` from `from`, by the
    /// sim's movement rules: each tick it covers `speed` times 1.5 with its centre on a road tile,
    /// times 3/4 on a slow tile, and times 0.8 while the tile up to one tile ahead toward its next
    /// waypoint stands higher (1.3 lower), using what is left over at a waypoint toward the next.
    /// Collisions, traffic and vehicles' turning are not counted.
    pub(crate) fn walk_ticks(&self, from: (f32, f32), route: &[(f32, f32)], speed: f32) -> u32 {
        const MAX_WALK_TICKS: u32 = 60 * 60 * 30;
        if speed <= 0.0 {
            return MAX_WALK_TICKS;
        }
        let ts = self.tile_size.max(1) as f32;
        let index = |point: (f32, f32)| {
            (point.0 >= 0.0 && point.1 >= 0.0)
                .then(|| {
                    tile_index(
                        self.width,
                        self.height,
                        (point.0 / ts) as u32,
                        (point.1 / ts) as u32,
                    )
                })
                .flatten()
        };
        let mut at = from;
        let mut next = 0;
        let mut ticks = 0;
        while next < route.len() && ticks < MAX_WALK_TICKS {
            let target = route[next];
            let (dx, dy) = (target.0 - at.0, target.1 - at.1);
            let length = dx.hypot(dy);
            if length <= f32::EPSILON {
                next += 1;
                continue;
            }
            let here = index(at);
            let reach = length.min(ts) / length;
            let ahead = index((at.0 + dx * reach, at.1 + dy * reach));
            let mut multiplier = 1.0;
            if here
                .and_then(|idx| self.road.get(idx).copied())
                .unwrap_or(false)
            {
                multiplier *= rts_rules::terrain::ROAD_MOVEMENT_SPEED_MULTIPLIER;
            }
            if here
                .and_then(|idx| self.slow.get(idx).copied())
                .unwrap_or(false)
            {
                multiplier *= rts_rules::terrain::SLOW_MOVEMENT_TILE_SPEED_MULTIPLIER;
            }
            let height = |idx: Option<usize>| {
                idx.and_then(|idx| self.elevation.get(idx).copied())
                    .unwrap_or(0)
            };
            multiplier *= rts_rules::terrain::elevation_movement_speed_multiplier(
                height(here),
                height(ahead),
            );
            let mut budget = speed * multiplier;
            ticks += 1;
            while budget > 0.0 && next < route.len() {
                let target = route[next];
                let (dx, dy) = (target.0 - at.0, target.1 - at.1);
                let length = dx.hypot(dy);
                if length <= budget {
                    at = target;
                    budget -= length;
                    next += 1;
                } else {
                    at = (at.0 + dx / length * budget, at.1 + dy / length * budget);
                    budget = 0.0;
                }
            }
        }
        ticks
    }

    /// The centre of the open tile nearest `point` with at least `minimum_clearance`, searching at
    /// most `max_radius_tiles` out, in the same ground component as Jeff's units can reach from
    /// `from`.
    pub(crate) fn open_ground_near(
        &self,
        from: (f32, f32),
        point: (f32, f32),
        minimum_clearance: u16,
        max_radius_tiles: i32,
    ) -> Option<(f32, f32)> {
        let start = self.nearest_route_tile(from, 1, None)?;
        let component =
            self.component_by_tile[tile_index(self.width, self.height, start.x, start.y)?];
        let tile = self.nearest_route_tile(point, minimum_clearance, component)?;
        let tile_size = self.tile_size.max(1) as f32;
        let wanted = (
            (point.0 / tile_size).floor() as i32,
            (point.1 / tile_size).floor() as i32,
        );
        ((tile.x as i32 - wanted.0)
            .abs()
            .max((tile.y as i32 - wanted.1).abs())
            <= max_radius_tiles)
            .then(|| tile_center_world(tile, self.tile_size))
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
                overlays_hash: 0,
            },
            width,
            height,
            tile_size,
            passable,
            line_of_sight_blocked: vec![false; (width * height) as usize],
            road: vec![false; (width * height) as usize],
            slow: vec![false; (width * height) as usize],
            elevation: vec![0; (width * height) as usize],
            concealment: vec![false; (width * height) as usize],
            no_entrenchment: vec![false; (width * height) as usize],
            clearance: vec![2; (width * height) as usize],
            component_by_tile,
            components: Vec::new(),
            region_by_tile: vec![None; (width * height) as usize],
            regions: Vec::new(),
            chokes: Vec::new(),
            starts: Vec::new(),
            resource_clusters: Vec::new(),
            base_routes: Vec::new(),
            start_ground_distances: Vec::new(),
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

        // Walking distance goes around the wall through its gap, not straight across it.
        let field = analysis.ground_distance_field((48.0, 48.0)).unwrap();
        let across = field[(width + 6) as usize];
        assert_eq!(field[(width + 3) as usize], 20);
        assert!((90..u32::MAX).contains(&across), "{across}");
        assert_eq!(field[(width + 4) as usize], u32::MAX);

        // Walking times follow the sim: 2 pixels a tick covers 3 tiles (96 px) in 48 ticks on open
        // ground, 32 on road, 64 on slow tiles; uphill takes 0.8 of the pace until on the rise.
        let walk = [(48.0, 16.0), (144.0, 16.0)];
        assert_eq!(analysis.walk_ticks(walk[0], &walk[1..], 2.0), 48);
        let mut road = analysis.clone();
        road.road = vec![true; (width * height) as usize];
        assert_eq!(road.walk_ticks(walk[0], &walk[1..], 2.0), 32);
        let mut slow = analysis.clone();
        slow.slow = vec![true; (width * height) as usize];
        assert_eq!(slow.walk_ticks(walk[0], &walk[1..], 2.0), 64);
        let mut hill = analysis.clone();
        hill.elevation = (0..width * height)
            .map(|i| u8::from(i % width >= 3))
            .collect();
        let uphill = hill.walk_ticks(walk[0], &walk[1..], 2.0);
        assert!((49..60).contains(&uphill), "{uphill}");
    }
}
