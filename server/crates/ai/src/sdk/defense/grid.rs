use super::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
pub(super) const FAR: u32 = u32::MAX / 4;
#[derive(Clone)]
pub(super) struct Grid {
    pub w: usize,
    pub size: f32,
    pub walk: Vec<bool>,
    pub entrench: Vec<bool>,
    pub rock: Vec<bool>,
    pub forest: Vec<bool>,
    pub speed: Vec<u32>,
}
impl Grid {
    pub fn new(map: &AiMap) -> Self {
        let w = map.width as usize;
        let len = (map.width as usize)
            .saturating_mul(map.height as usize)
            .min(map.terrain.len());
        let mut g = Self {
            w,
            size: map.tile_size.max(1) as f32,
            walk: vec![false; len],
            entrench: vec![false; len],
            rock: vec![false; len],
            forest: vec![false; len],
            speed: vec![120; len],
        };
        for i in 0..len {
            let t = (i as u32 % map.width.max(1), i as u32 / map.width.max(1));
            let terrain = map.terrain[i];
            g.walk[i] = !matches!(
                terrain,
                AiTerrain::Rock | AiTerrain::Water | AiTerrain::Unknown(_)
            ) && !map.unwalkable_tiles.contains(&t);
            g.forest[i] = map.concealment_tiles.contains(&t);
            g.rock[i] = matches!(terrain, AiTerrain::Rock | AiTerrain::Unknown(_));
            let road = matches!(
                terrain,
                AiTerrain::RoadBare
                    | AiTerrain::RoadHorizontal
                    | AiTerrain::RoadVertical
                    | AiTerrain::RoadDiagonalNwSe
                    | AiTerrain::RoadDiagonalNeSw
            );
            g.entrench[i] = g.walk[i] && !road && !map.no_entrenchment_tiles.contains(&t);
            g.speed[i] = match (road, map.slow_movement_tiles.contains(&t)) {
                (true, false) => 80,
                (true, true) => 107,
                (false, true) => 160,
                _ => 120,
            };
        }
        g
    }
    pub fn tile(&self, i: usize) -> (u32, u32) {
        ((i % self.w.max(1)) as u32, (i / self.w.max(1)) as u32)
    }
    pub fn point(&self, i: usize) -> (f32, f32) {
        let (x, y) = self.tile(i);
        ((x as f32 + 0.5) * self.size, (y as f32 + 0.5) * self.size)
    }
    pub fn index(&self, p: (f32, f32)) -> Option<usize> {
        if !p.0.is_finite() || !p.1.is_finite() || p.0 < 0. || p.1 < 0. {
            return None;
        }
        let x = (p.0 / self.size) as usize;
        let y = (p.1 / self.size) as usize;
        (x < self.w)
            .then_some(y.saturating_mul(self.w).saturating_add(x))
            .filter(|i| *i < self.walk.len())
    }
    pub fn near(&self, p: (f32, f32)) -> Option<usize> {
        self.index(p).filter(|i| self.walk[*i]).or_else(|| {
            (0..self.walk.len())
                .filter(|i| self.walk[*i])
                .min_by(|a, b| dist2(self.point(*a), p).total_cmp(&dist2(self.point(*b), p)))
        })
    }
    pub fn neighbors(&self, i: usize) -> impl Iterator<Item = (usize, u32)> + '_ {
        let (x, y) = self.tile(i);
        let x = x as i32;
        let y = y as i32;
        [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (-1, 1),
            (1, -1),
            (1, 1),
        ]
        .into_iter()
        .filter_map(move |(dx, dy)| {
            let nx = x + dx;
            let ny = y + dy;
            if nx < 0 || ny < 0 || nx as usize >= self.w {
                return None;
            }
            let j = ny as usize * self.w + nx as usize;
            if !self.walk.get(j).copied().unwrap_or(false) {
                return None;
            }
            let diag = dx != 0 && dy != 0;
            if diag
                && (!self.walk[y as usize * self.w + nx as usize]
                    || !self.walk[ny as usize * self.w + x as usize])
            {
                return None;
            }
            Some((j, if diag { 14 } else { 10 }))
        })
    }
    pub fn field(&self, starts: &[usize], timed: bool) -> Vec<u32> {
        let mut d = vec![FAR; self.walk.len()];
        let mut q = BinaryHeap::new();
        for &i in starts {
            if self.walk.get(i).copied().unwrap_or(false) {
                d[i] = 0;
                q.push(Reverse((0, i)));
            }
        }
        while let Some(Reverse((c, i))) = q.pop() {
            if d[i] != c {
                continue;
            }
            for (j, step) in self.neighbors(i) {
                let nc = c + if timed {
                    step * self.speed[j] / 10
                } else {
                    step
                };
                if nc < d[j] {
                    d[j] = nc;
                    q.push(Reverse((nc, j)));
                }
            }
        }
        d
    }
    #[cfg(test)]
    pub fn ray(&self, a: usize, b: usize) -> bool {
        self.ray_blocked(a, b, &[])
    }
    /// Tile-center DDA with opaque corner checks, matching authoritative LOS traversal.
    /// Conservative known footprint/resource blockers additionally prevent firing through assets.
    pub fn ray_blocked(&self, a: usize, b: usize, blockers: &[bool]) -> bool {
        let (ax, ay) = self.tile(a);
        let (bx, by) = self.tile(b);
        let dx = bx as f32 - ax as f32;
        let dy = by as f32 - ay as f32;
        let sx = dx.signum() as i32;
        let sy = dy.signum() as i32;
        let ddx = if dx == 0. {
            f32::INFINITY
        } else {
            1. / dx.abs()
        };
        let ddy = if dy == 0. {
            f32::INFINITY
        } else {
            1. / dy.abs()
        };
        let mut tx = ax as i32;
        let mut ty = ay as i32;
        let mut mx = ddx * 0.5;
        let mut my = ddy * 0.5;
        let clear = |x: i32, y: i32| {
            if x < 0 || y < 0 || x as usize >= self.w {
                return false;
            }
            let i = y as usize * self.w + x as usize;
            !self.rock.get(i).copied().unwrap_or(true) && !blockers.get(i).copied().unwrap_or(false)
        };
        let mut forest = self.forest[a];
        let mut entered = false;
        let mut depth = 0;
        while tx != bx as i32 || ty != by as i32 {
            if mx < my {
                tx += sx;
                mx += ddx;
            } else if my < mx {
                ty += sy;
                my += ddy;
            } else {
                if !clear(tx + sx, ty) || !clear(tx, ty + sy) {
                    return false;
                }
                tx += sx;
                ty += sy;
                mx += ddx;
                my += ddy;
            }
            if !clear(tx, ty) {
                return false;
            }
            let inside = self.forest[ty as usize * self.w + tx as usize];
            if inside {
                entered |= !forest;
                depth += 1;
                if depth > rts_rules::terrain::CONCEALMENT_SIGHT_DEPTH_TILES {
                    return false;
                }
            } else if entered {
                return false;
            }
            forest = inside;
        }
        true
    }
    pub fn components(&self, tiles: &[usize]) -> Vec<Vec<usize>> {
        let mut left: BTreeSet<_> = tiles.iter().copied().collect();
        let mut out = vec![];
        while let Some(i) = left.pop_first() {
            let mut q = VecDeque::from([i]);
            let mut c = vec![i];
            while let Some(i) = q.pop_front() {
                let (x, y) = self.tile(i);
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        if nx < 0 || ny < 0 || nx as usize >= self.w {
                            continue;
                        }
                        let j = ny as usize * self.w + nx as usize;
                        if left.remove(&j) {
                            q.push_back(j);
                            c.push(j);
                        }
                    }
                }
            }
            c.sort_unstable();
            out.push(c);
        }
        out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        out
    }
}
pub(super) fn dist2(a: (f32, f32), b: (f32, f32)) -> f32 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}
