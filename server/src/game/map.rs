//! The tile map: terrain grid, authored map loading, and passability. See `DESIGN.md` §3 (`map.rs`).
//!
//! The live game currently uses one deterministic handcrafted map embedded in the server binary.
//! The map file defines terrain and ordered base sites; the simulation still derives player
//! starts, expansion sites, starting buildings, workers, and resource clusters from those sites.
//!
//! Terrain passability here is purely about *terrain* — building footprints are tracked
//! dynamically by the simulation (a separate occupancy grid in `systems`/`pathfinding`),
//! not baked into the map.

use std::collections::HashSet;

use crate::config;
use crate::protocol::terrain;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;

const DEFAULT_MAP_JSON: &str = include_str!("../../assets/maps/default.json");

/// Radius around every authored base site that must remain passable. This covers the starting
/// Industrial Center footprint, worker ring, and the deterministic steel/oil cluster derived from
/// that site.
pub const BASE_PROTECTION_RADIUS_TILES: i32 = 7;

/// The terrain grid plus the selected start and expansion tiles.
#[derive(Debug)]
pub struct Map {
    /// Side length in tiles (square map).
    pub size: u32,
    /// Row-major terrain codes, length `size * size`.
    pub terrain: Vec<u8>,
    /// One start tile `(tile_x, tile_y)` per player, in player-index order.
    pub starts: Vec<(u32, u32)>,
    /// Neutral expansion sites. These receive resource clusters but no starting buildings.
    pub expansion_sites: Vec<(u32, u32)>,
}

impl Map {
    /// Load the deterministic handcrafted map for `player_count` players.
    ///
    /// The `seed` is used to shuffle which authored base pair each player draws, so the
    /// human/AI seating in the lobby does not pin them to the same corner every match.
    pub fn generate(player_count: usize, seed: u32) -> Map {
        Self::from_authored_json(player_count, DEFAULT_MAP_JSON, seed)
            .unwrap_or_else(|err| panic!("invalid hardcoded map asset: {err}"))
    }

    fn from_authored_json(player_count: usize, json: &str, seed: u32) -> Result<Map, String> {
        let authored: AuthoredMap =
            serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
        let (size, terrain) = parse_terrain(&authored.terrain)?;
        let base_sites = parse_base_sites(size, &authored.base_sites)?;

        if player_count == 0 {
            return Err("player_count must be at least 1".to_string());
        }
        if 2 * player_count > base_sites.len() {
            return Err(format!(
                "map has {} base sites but needs {} (2 per player) for {player_count} players",
                base_sites.len(),
                2 * player_count,
            ));
        }

        validate_base_clearance(size, &terrain, &base_sites)?;

        // baseSites are interleaved pairs: [start0, expansion0, start1, expansion1, ...].
        // Even indices are player starts; odd indices are the paired neutral expansion bases.
        // Shuffle the pairs (keeping each start with its expansion) so the lobby seat order does
        // not pin players to the same corner every match. Only the first N shuffled pairs are
        // active; the remainder of the authored sites are unused for this player count.
        let total_pairs = base_sites.len() / 2;
        let mut pairs: Vec<((u32, u32), (u32, u32))> = (0..total_pairs)
            .map(|i| (base_sites[2 * i], base_sites[2 * i + 1]))
            .collect();
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        pairs.shuffle(&mut rng);
        let all_expansions: Vec<(u32, u32)> = pairs.iter().map(|&(_, exp)| exp).collect();
        let (starts, _): (Vec<_>, Vec<_>) = pairs.into_iter().take(player_count).unzip();
        let expansion_sites = assign_expansions_fair(&starts, &all_expansions);

        Ok(Map {
            size,
            terrain,
            starts,
            expansion_sites,
        })
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredMap {
    #[allow(dead_code)]
    name: String,
    terrain: Vec<String>,
    base_sites: Vec<AuthoredSite>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct AuthoredSite {
    x: u32,
    y: u32,
}

fn parse_terrain(rows: &[String]) -> Result<(u32, Vec<u8>), String> {
    if rows.is_empty() {
        return Err("terrain must contain at least one row".to_string());
    }

    let size = rows[0].chars().count();
    if size == 0 {
        return Err("terrain rows must not be empty".to_string());
    }
    if rows.len() != size {
        return Err(format!(
            "terrain must be square; got {} rows by {size} columns",
            rows.len()
        ));
    }

    let size_u32 =
        u32::try_from(size).map_err(|_| "terrain size does not fit in u32".to_string())?;
    let mut out = Vec::with_capacity(size * size);
    for (y, row) in rows.iter().enumerate() {
        let width = row.chars().count();
        if width != size {
            return Err(format!(
                "terrain row {y} has width {width}; expected {size}"
            ));
        }
        for (x, ch) in row.chars().enumerate() {
            let code = match ch {
                '.' => terrain::GRASS,
                '#' => terrain::ROCK,
                '~' => terrain::WATER,
                _ => {
                    return Err(format!(
                        "unknown terrain character '{ch}' at tile ({x},{y})"
                    ))
                }
            };
            out.push(code);
        }
    }

    Ok((size_u32, out))
}

fn parse_base_sites(size: u32, authored: &[AuthoredSite]) -> Result<Vec<(u32, u32)>, String> {
    if authored.is_empty() {
        return Err("baseSites must contain at least one site".to_string());
    }

    let mut seen = HashSet::with_capacity(authored.len());
    let mut out = Vec::with_capacity(authored.len());
    for (i, site) in authored.iter().enumerate() {
        if site.x >= size || site.y >= size {
            return Err(format!(
                "baseSites[{i}] = ({},{}) is outside the {size}x{size} map",
                site.x, site.y
            ));
        }
        if !seen.insert((site.x, site.y)) {
            return Err(format!(
                "baseSites[{i}] duplicates an earlier site at ({},{})",
                site.x, site.y
            ));
        }
        out.push((site.x, site.y));
    }
    Ok(out)
}

fn validate_base_clearance(
    size: u32,
    terrain_grid: &[u8],
    base_sites: &[(u32, u32)],
) -> Result<(), String> {
    for (i, &(sx, sy)) in base_sites.iter().enumerate() {
        for dy in -BASE_PROTECTION_RADIUS_TILES..=BASE_PROTECTION_RADIUS_TILES {
            for dx in -BASE_PROTECTION_RADIUS_TILES..=BASE_PROTECTION_RADIUS_TILES {
                let tx = sx as i32 + dx;
                let ty = sy as i32 + dy;
                if tx < 0 || ty < 0 || tx >= size as i32 || ty >= size as i32 {
                    return Err(format!(
                        "baseSites[{i}] at ({sx},{sy}) is too close to the map edge"
                    ));
                }
                let idx = (ty as u32 * size + tx as u32) as usize;
                if terrain_grid[idx] != terrain::GRASS {
                    return Err(format!(
                        "baseSites[{i}] at ({sx},{sy}) has impassable terrain in its protected area at ({tx},{ty})"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn gen_perms(
    n: usize,
    m: usize,
    used: &mut Vec<bool>,
    current: &mut Vec<usize>,
    result: &mut Vec<Vec<usize>>,
) {
    if current.len() == n {
        result.push(current.clone());
        return;
    }
    for i in 0..m {
        if !used[i] {
            used[i] = true;
            current.push(i);
            gen_perms(n, m, used, current, result);
            current.pop();
            used[i] = false;
        }
    }
}

fn permutation_indices(n: usize, m: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut used = vec![false; m];
    let mut current = Vec::with_capacity(n);
    gen_perms(n, m, &mut used, &mut current, &mut result);
    result
}

fn assign_expansions_fair(starts: &[(u32, u32)], candidates: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let n = starts.len();
    let m = candidates.len();
    debug_assert!(
        m >= n,
        "need at least as many expansion candidates as players"
    );

    let dist = |a: (u32, u32), b: (u32, u32)| -> f64 {
        let dx = a.0 as f64 - b.0 as f64;
        let dy = a.1 as f64 - b.1 as f64;
        (dx * dx + dy * dy).sqrt()
    };

    let score_perm = |idxs: &[usize]| -> f64 {
        (0..n)
            .map(|i| {
                let exp = candidates[idxs[i]];
                let own = dist(starts[i], exp);
                let threat = (0..n)
                    .filter(|&j| j != i)
                    .map(|j| dist(starts[j], exp))
                    .fold(f64::MAX, f64::min);
                // 1-player game: no enemy threat — treat as always-safe
                let threat = if threat == f64::MAX {
                    own * 10.0
                } else {
                    threat
                };
                threat - own
            })
            .fold(f64::MAX, f64::min)
    };

    permutation_indices(n, m)
        .into_iter()
        .max_by(|a, b| {
            score_perm(a)
                .partial_cmp(&score_perm(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|idxs| idxs.iter().map(|&i| candidates[i]).collect())
        .unwrap_or_else(|| candidates[..n].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardcoded_map_loads_for_every_supported_player_count() {
        for player_count in 1..=4 {
            let map = Map::generate(player_count, 0x1234_5678);
            assert_eq!(map.terrain.len(), (map.size * map.size) as usize);
            assert_eq!(map.starts.len(), player_count);
            assert!(!map.expansion_sites.is_empty());

            for start in &map.starts {
                assert!(map.is_passable(start.0 as i32, start.1 as i32));
            }
            for expansion in &map.expansion_sites {
                assert!(map.is_passable(expansion.0 as i32, expansion.1 as i32));
            }
        }
    }

    #[test]
    fn same_seed_produces_identical_layout() {
        let a = Map::generate(4, 0xdead_beef);
        let b = Map::generate(4, 0xdead_beef);

        assert_eq!(a.size, b.size);
        assert_eq!(a.terrain, b.terrain);
        assert_eq!(a.starts, b.starts);
        assert_eq!(a.expansion_sites, b.expansion_sites);
    }

    #[test]
    fn different_seeds_produce_different_start_orderings() {
        // With 4 authored pairs and at least one differing 2-player ordering, scanning a handful of
        // seeds must yield more than one distinct starts vector. If shuffling silently breaks this
        // catches it.
        let layouts: HashSet<Vec<(u32, u32)>> = (0..32u32)
            .map(|seed| Map::generate(2, seed).starts)
            .collect();
        assert!(
            layouts.len() > 1,
            "expected at least two distinct start orderings across seeds, got {layouts:?}"
        );
    }

    #[test]
    fn shuffled_starts_get_expansions_from_authored_pool() {
        // All authored expansions from default.json.
        let authored_expansions: &[(u32, u32)] = &[(48, 23), (48, 73), (73, 47), (23, 47)];
        for seed in 0..16u32 {
            let map = Map::generate(4, seed);
            assert_eq!(map.starts.len(), map.expansion_sites.len());
            // Every assigned expansion must come from the authored pool.
            for expansion in &map.expansion_sites {
                assert!(
                    authored_expansions.contains(expansion),
                    "expansion {expansion:?} is not from the authored pool (seed {seed})"
                );
            }
            // No two players share the same expansion.
            let unique: HashSet<_> = map.expansion_sites.iter().collect();
            assert_eq!(
                unique.len(),
                map.expansion_sites.len(),
                "duplicate expansion assigned (seed {seed})"
            );
        }
    }

    #[test]
    fn fair_assignment_avoids_exposed_natural() {
        // Bug case: top-left (10,10) and bottom-left (10,85).
        // Authored expansions for these two are (48,23) and (23,47).
        // (23,47) is the middle-left expansion — nearly equidistant from both players.
        // The fair assignment should give bottom-left something safer, e.g. (48,73).
        let starts: &[(u32, u32)] = &[(10, 10), (10, 85)];
        let candidates: &[(u32, u32)] = &[(48, 23), (48, 73), (73, 47), (23, 47)];
        let assigned = assign_expansions_fair(starts, candidates);
        assert_eq!(assigned.len(), 2);
        // bottom-left player (index 1) must NOT get (23,47) — the exposed middle-left expansion.
        assert_ne!(
            assigned[1],
            (23, 47),
            "bottom-left should not be assigned the exposed middle-left expansion (23,47)"
        );
        // bottom-left should get (48,73) — the bottom-middle expansion, far from the top-left enemy.
        assert_eq!(
            assigned[1],
            (48, 73),
            "bottom-left should be assigned the safe bottom expansion (48,73)"
        );
    }

    #[test]
    fn each_player_gets_one_paired_expansion() {
        for seed in 0..16u32 {
            let map = Map::generate(2, seed);
            assert_eq!(map.starts.len(), 2);
            assert_eq!(map.expansion_sites.len(), 2);
        }
    }

    #[test]
    fn authored_map_rejects_unknown_terrain_characters() {
        let err = Map::from_authored_json(
            1,
            r#"{
              "name": "bad",
              "terrain": ["..", ".x"],
              "baseSites": [{"x": 0, "y": 0}]
            }"#,
            0,
        )
        .expect_err("bad terrain should be rejected");

        assert!(err.contains("unknown terrain character"));
    }

    #[test]
    fn authored_map_rejects_impassable_base_protection_area() {
        // 32×32 map; rock at (8,8) sits inside the protection area of the first base site.
        // A valid second site at (24,24) satisfies the 2-sites-per-player requirement.
        let mut rows = vec![".".repeat(32); 32];
        rows[8].replace_range(8..9, "#");
        let json = format!(
            r#"{{
              "name": "bad-base",
              "terrain": {},
              "baseSites": [{{"x": 8, "y": 8}}, {{"x": 24, "y": 24}}]
            }}"#,
            serde_json::to_string(&rows).unwrap()
        );

        let err = Map::from_authored_json(1, &json, 0)
            .expect_err("blocked base protection area should be rejected");

        assert!(err.contains("impassable terrain"));
    }
}
