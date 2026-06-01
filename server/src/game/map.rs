//! The tile map: terrain grid, generation, and passability. See `DESIGN.md` §3 (`map.rs`).
//!
//! The map is square with side `config::map_size_for(players)`. It is mostly GRASS with a
//! few ROCK/WATER obstacle clusters placed with rotational symmetry so no start position is
//! advantaged. Start tiles are chosen symmetrically (corners / edge midpoints) and we never
//! place an obstacle on a start area or its adjacent resource cluster.
//!
//! Terrain passability here is purely about *terrain* — building footprints are tracked
//! dynamically by the simulation (a separate occupancy grid in `systems`/`pathfinding`),
//! not baked into the map.

use crate::config;
use crate::protocol::terrain;

/// A tiny deterministic xorshift32 PRNG. We avoid pulling in `rand` (not a dependency) and
/// want fully reproducible maps for a given seed so matches are deterministic and testable.
pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    /// Seed must be non-zero; zero is remapped to a fixed constant.
    pub fn new(seed: u32) -> Self {
        XorShift32 {
            state: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// Next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Uniform integer in `[0, n)` (n > 0).
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next_u32() % n
        }
    }

    /// Uniform float in `[0.0, 1.0)`.
    pub fn unit_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// The terrain grid plus the chosen symmetric start tiles.
pub struct Map {
    /// Side length in tiles (square map).
    pub size: u32,
    /// Row-major terrain codes, length `size * size`.
    pub terrain: Vec<u8>,
    /// One start tile `(tile_x, tile_y)` per player, in player-index order.
    pub starts: Vec<(u32, u32)>,
    /// One neutral expansion site `(tile_x, tile_y)` per player.
    pub expansion_sites: Vec<(u32, u32)>,
}

impl Map {
    /// Generate a symmetric map for `player_count` players using a deterministic seed.
    pub fn generate(player_count: usize, seed: u32) -> Map {
        let size = config::map_size_for(player_count);
        let mut terrain = vec![terrain::GRASS; (size * size) as usize];

        let mut rng = XorShift32::new(seed);
        let starts = symmetric_starts(size, player_count, &mut rng);
        let expansion_sites = expansion_sites(size, player_count, &starts);

        // Tiles we must keep clear: each start area and its resource cluster footprint.
        // We protect a generous square around every start tile plus every neutral expansion site.
        let protected = protected_tiles(size, &starts, &expansion_sites);

        scatter_symmetric_obstacles(&mut terrain, size, &mut rng, &protected);

        Map {
            size,
            terrain,
            starts,
            expansion_sites,
        }
    }

    #[inline]
    pub fn index(&self, x: u32, y: u32) -> usize {
        (y * self.size + x) as usize
    }

    /// Whether a tile coordinate is inside the map.
    #[inline]
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.size && (y as u32) < self.size
    }

    /// Terrain code at a tile (GRASS for out-of-bounds, treated impassable elsewhere).
    #[inline]
    pub fn terrain_at(&self, x: u32, y: u32) -> u8 {
        if x < self.size && y < self.size {
            self.terrain[self.index(x, y)]
        } else {
            terrain::ROCK
        }
    }

    /// Whether a tile is passable terrain (GRASS). Out-of-bounds is impassable. This does NOT
    /// account for building footprints — callers combine this with the dynamic occupancy grid.
    #[inline]
    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        self.terrain_at(x as u32, y as u32) == terrain::GRASS
    }

    /// World-pixel center of a tile.
    #[inline]
    pub fn tile_center(&self, tx: u32, ty: u32) -> (f32, f32) {
        let ts = config::TILE_SIZE as f32;
        (tx as f32 * ts + ts * 0.5, ty as f32 * ts + ts * 0.5)
    }

    /// Tile containing a world-pixel point (clamped into bounds).
    #[inline]
    pub fn tile_of(&self, x: f32, y: f32) -> (u32, u32) {
        let ts = config::TILE_SIZE as f32;
        let tx = (x / ts).floor().max(0.0) as u32;
        let ty = (y / ts).floor().max(0.0) as u32;
        (tx.min(self.size - 1), ty.min(self.size - 1))
    }

    /// World size in pixels (square).
    pub fn world_size_px(&self) -> f32 {
        self.size as f32 * config::TILE_SIZE as f32
    }
}

/// Choose symmetric start tiles for `player_count` players.
///
/// We inset the starts from the very edge so the Industrial Center footprint and its resource cluster fit.
/// - 2 players: any two distinct corners.
/// - 3 players: any three distinct corners.
/// - 4 players: the four corners.
fn symmetric_starts(size: u32, player_count: usize, rng: &mut XorShift32) -> Vec<(u32, u32)> {
    // Inset keeps the start area + resource cluster fully on-map.
    let inset = 11u32.min(size / 4);
    let lo = inset;
    let hi = size - 1 - inset;
    let nw = (lo, lo);
    let ne = (hi, lo);
    let sw = (lo, hi);
    let se = (hi, hi);
    let mut corners = vec![nw, ne, sw, se];
    shuffle(&mut corners, rng);
    corners.truncate(player_count.min(corners.len()));
    corners
}

/// Tiles that must never be made impassable: a square around each start tile big enough to
/// hold the Industrial Center footprint, the worker spawn ring, and the steel/oil clusters.
/// All four corners are always protected because resource patches always spawn there.
fn protected_tiles(size: u32, starts: &[(u32, u32)], expansion_sites: &[(u32, u32)]) -> Vec<bool> {
    let mut prot = vec![false; (size * size) as usize];
    let r: i32 = 7;

    // Collect all tiles to protect: player starts + all four corners.
    let inset = 11u32.min(size / 4);
    let lo = inset;
    let hi = size - 1 - inset;
    let all_corners = [(lo, lo), (hi, lo), (lo, hi), (hi, hi)];
    let to_protect: Vec<(u32, u32)> = starts
        .iter()
        .copied()
        .chain(all_corners)
        .chain(expansion_sites.iter().copied())
        .collect();

    for (sx, sy) in to_protect {
        for dy in -r..=r {
            for dx in -r..=r {
                let tx = sx as i32 + dx;
                let ty = sy as i32 + dy;
                if tx >= 0 && ty >= 0 && (tx as u32) < size && (ty as u32) < size {
                    prot[(ty as u32 * size + tx as u32) as usize] = true;
                }
            }
        }
    }
    prot
}

/// One neutral expansion site per player.
fn expansion_sites(size: u32, player_count: usize, starts: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let inset = 11u32.min(size / 4);
    let lo = inset;
    let hi = size - 1 - inset;
    let mid = size / 2;
    let corners = [(lo, lo), (hi, lo), (lo, hi), (hi, hi)];
    let edge_midpoints = [(mid, lo), (hi, mid), (mid, hi), (lo, mid)];
    let candidates = match player_count {
        0 => Vec::new(),
        1..=3 => corners
            .into_iter()
            .chain(edge_midpoints)
            .collect::<Vec<_>>(),
        _ => edge_midpoints.to_vec(),
    };
    candidates
        .into_iter()
        .filter(|site| !starts.contains(site))
        .take(player_count)
        .collect()
}

/// Scatter a handful of obstacle clusters and mirror each one under 180° rotational symmetry
/// so the layout is fair. Never writes onto a protected tile (its mirror is protected too, by
/// symmetry of the start placement, but we guard both ends regardless).
fn scatter_symmetric_obstacles(
    terrain: &mut [u8],
    size: u32,
    rng: &mut XorShift32,
    protected: &[bool],
) {
    // Number of *base* clusters; each is mirrored, so total obstacle area is doubled.
    let clusters = 3 + (size / 32); // scales mildly with map size
    let inner_lo = 6i32;
    let inner_hi = size as i32 - 1 - 6;
    if inner_hi <= inner_lo {
        return;
    }

    for _ in 0..clusters {
        // Pick a seed tile somewhere in the interior.
        let cx = inner_lo + rng.below((inner_hi - inner_lo) as u32) as i32;
        let cy = inner_lo + rng.below((inner_hi - inner_lo) as u32) as i32;
        // Choose ROCK or WATER for the whole cluster.
        let kind = if rng.unit_f32() < 0.5 {
            terrain::ROCK
        } else {
            terrain::WATER
        };
        // Blob radius 1..=3.
        let radius = 1 + rng.below(3) as i32;

        for dy in -radius..=radius {
            for dx in -radius..=radius {
                // Rough circle so blobs look organic, not blocky.
                if dx * dx + dy * dy > radius * radius {
                    continue;
                }
                let tx = cx + dx;
                let ty = cy + dy;
                paint_obstacle(terrain, size, protected, tx, ty, kind);
                // 180° rotational mirror: (size-1-tx, size-1-ty).
                let mx = size as i32 - 1 - tx;
                let my = size as i32 - 1 - ty;
                paint_obstacle(terrain, size, protected, mx, my, kind);
            }
        }
    }
}

/// Paint one obstacle tile if it is in bounds and not protected.
fn paint_obstacle(terrain: &mut [u8], size: u32, protected: &[bool], tx: i32, ty: i32, kind: u8) {
    if tx < 0 || ty < 0 || (tx as u32) >= size || (ty as u32) >= size {
        return;
    }
    let idx = (ty as u32 * size + tx as u32) as usize;
    if protected[idx] {
        return;
    }
    terrain[idx] = kind;
}

fn shuffle<T>(items: &mut [T], rng: &mut XorShift32) {
    if items.len() < 2 {
        return;
    }
    for i in (1..items.len()).rev() {
        let j = rng.below((i + 1) as u32) as usize;
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corner_tiles(player_count: usize) -> Vec<(u32, u32)> {
        let size = config::map_size_for(player_count);
        let inset = 11u32.min(size / 4);
        let lo = inset;
        let hi = size - 1 - inset;
        vec![(lo, lo), (lo, hi), (hi, lo), (hi, hi)]
    }

    #[test]
    fn generate_shuffles_starts_by_seed() {
        let a = Map::generate(4, 1);
        let b = Map::generate(4, 2);
        assert_eq!(a.starts.len(), 4);
        assert_eq!(b.starts.len(), 4);
        assert_ne!(a.starts, b.starts);

        let mut a_sorted = a.starts.clone();
        let mut b_sorted = b.starts.clone();
        a_sorted.sort_unstable();
        b_sorted.sort_unstable();
        assert_eq!(a_sorted, b_sorted);
    }

    #[test]
    fn starts_are_random_distinct_corners_by_seed() {
        for player_count in 1..=4 {
            let corners = corner_tiles(player_count);

            for seed in 1..=64 {
                let mut starts = Map::generate(player_count, seed).starts;
                assert_eq!(starts.len(), player_count);
                assert!(starts.iter().all(|start| corners.contains(start)));

                starts.sort_unstable();
                starts.dedup();
                assert_eq!(starts.len(), player_count);
            }
        }
    }

    #[test]
    fn two_player_starts_cover_every_ordered_corner_pair_by_seed() {
        let corners = corner_tiles(2);
        let mut observed = Vec::new();

        for seed in 1..=1024 {
            let starts = Map::generate(2, seed).starts;
            if !observed.contains(&starts) {
                observed.push(starts);
            }
        }
        observed.sort_unstable();

        let mut expected = Vec::new();
        for a in &corners {
            for b in &corners {
                if a != b {
                    expected.push(vec![*a, *b]);
                }
            }
        }
        expected.sort_unstable();

        assert_eq!(observed, expected);
    }

    #[test]
    fn two_player_expansions_are_the_unused_corners() {
        let all_corners = corner_tiles(2);

        for seed in 1..=64 {
            let map = Map::generate(2, seed);
            assert_eq!(map.starts.len(), 2);
            assert_eq!(map.expansion_sites.len(), 2);

            let mut all_sites = map.starts.clone();
            all_sites.extend(map.expansion_sites.iter().copied());
            all_sites.sort_unstable();
            assert_eq!(all_sites, all_corners);

            for start in &map.starts {
                assert!(!map.expansion_sites.contains(start));
            }
        }
    }
}
