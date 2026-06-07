//! The tile map: terrain grid, authored map loading, and passability. See
//! `docs/design/server-sim.md` (`map.rs`).
//!
//! The live game loads authored maps from the server asset bundle. Map files define terrain and
//! ordered base sites; the simulation still derives player starts, expansion sites, starting
//! buildings, workers, and resource clusters from those sites.
//!
//! Terrain passability here is purely about *terrain* — building footprints are tracked
//! dynamically by the simulation (a separate occupancy grid in `systems`/`pathfinding`),
//! not baked into the map.

use std::collections::HashSet;
use std::path::Path;

use crate::config;
use crate::protocol::terrain;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::Deserialize;

const DEFAULT_MAP_JSON: &str = include_str!("../../assets/maps/default-handcrafted.json");
const MAPS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/maps");

type Tile = (u32, u32);
type BasePair = (Tile, Tile);

/// Radius around a player start site (even index) that must remain passable.
pub const BASE_PROTECTION_RADIUS_TILES: i32 = 7;
/// Radius around a natural expansion site (odd index) that must remain passable.
/// Smaller than the start radius because naturals have no City Centre or worker ring.
pub const EXPANSION_PROTECTION_RADIUS_TILES: i32 = 4;

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

    /// Return the display names of all maps in `assets/maps/`. The name is read from the JSON
    /// `name` field; the filename stem is used as a fallback. Errors (unreadable directory or
    /// files) are silently skipped so a bad asset file can't crash the lobby.
    pub fn list_available() -> Vec<String> {
        let dir = Path::new(MAPS_DIR);
        let mut names: Vec<String> = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return names;
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .map(|e| e.path())
            .collect();
        paths.sort();
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let name = std::fs::read_to_string(&path)
                .ok()
                .and_then(|json| {
                    serde_json::from_str::<serde_json::Value>(&json)
                        .ok()
                        .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
                })
                .unwrap_or(stem);
            if !name.is_empty() {
                names.push(name);
            }
        }
        names
    }

    /// Load a map by display name (the `name` field in the JSON) for `player_count` players.
    /// Returns an error string if the map cannot be found, read, or parsed.
    pub fn load(map_name: &str, player_count: usize, seed: u32) -> Result<Map, String> {
        // First try to match by `name` field, then by filename stem.
        let dir = Path::new(MAPS_DIR);
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Err(format!("cannot read maps directory: {MAPS_DIR}"));
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .map(|e| e.path())
            .collect();
        paths.sort();

        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let json = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let json_name = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(str::to_string));
            let matches = json_name.as_deref() == Some(map_name) || stem == map_name;
            if matches {
                return Self::from_authored_json(player_count, &json, seed);
            }
        }
        Err(format!("map not found: {map_name:?}"))
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
        // Even indices are player starts; odd indices are neutral expansion bases.
        // Shuffle the pairs so the lobby seat order does not pin players to the same corner every
        // match. For 2-player games, keep the randomized starts but re-pick the natural assignment
        // from the authored expansion pool so adjacent starts get symmetric naturals instead of one
        // shared middle natural and one side natural.
        let total_pairs = base_sites.len() / 2;
        let authored_pairs: Vec<BasePair> = (0..total_pairs)
            .map(|i| (base_sites[2 * i], base_sites[2 * i + 1]))
            .collect();
        let mut pairs = authored_pairs.clone();
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        pairs.shuffle(&mut rng);
        let selected_pairs: Vec<_> = pairs.into_iter().take(player_count).collect();
        let starts: Vec<_> = selected_pairs.iter().map(|(start, _)| *start).collect();
        let expansion_sites = if player_count == 2 {
            select_symmetric_two_player_expansions([starts[0], starts[1]], &authored_pairs)
        } else {
            selected_pairs
                .iter()
                .map(|(_, expansion)| *expansion)
                .collect()
        };

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

fn select_symmetric_two_player_expansions(
    starts: [Tile; 2],
    authored_pairs: &[BasePair],
) -> Vec<Tile> {
    let authored_expansions = [
        authored_expansion_for_start(starts[0], authored_pairs),
        authored_expansion_for_start(starts[1], authored_pairs),
    ];
    let mut best: Option<(TwoPlayerExpansionScore, [(u32, u32); 2])> = None;

    for (left_index, &left_expansion) in authored_pairs
        .iter()
        .map(|(_, expansion)| expansion)
        .enumerate()
    {
        for (right_index, &right_expansion) in authored_pairs
            .iter()
            .map(|(_, expansion)| expansion)
            .enumerate()
        {
            if left_index == right_index {
                continue;
            }
            let expansions = [left_expansion, right_expansion];
            let score =
                score_two_player_expansion_assignment(starts, expansions, authored_expansions);
            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score.is_better_than(best_score))
            {
                best = Some((score, expansions));
            }
        }
    }

    best.map(|(_, expansions)| expansions.to_vec())
        .unwrap_or_else(|| {
            starts
                .iter()
                .filter_map(|&start| authored_expansion_for_start(start, authored_pairs))
                .collect()
        })
}

fn authored_expansion_for_start(start: Tile, authored_pairs: &[BasePair]) -> Option<Tile> {
    authored_pairs
        .iter()
        .find_map(|&(pair_start, expansion)| (pair_start == start).then_some(expansion))
}

#[derive(Debug, Clone, Copy)]
struct TwoPlayerExpansionScore {
    local_symmetry_error: f32,
    distance_error: f32,
    authored_mismatches: u8,
    total_distance: f32,
}

impl TwoPlayerExpansionScore {
    fn is_better_than(self, current: &Self) -> bool {
        if let Some(is_better) =
            compare_score_value(self.local_symmetry_error, current.local_symmetry_error)
        {
            return is_better;
        }
        if let Some(is_better) = compare_score_value(self.distance_error, current.distance_error) {
            return is_better;
        }
        if self.authored_mismatches != current.authored_mismatches {
            return self.authored_mismatches < current.authored_mismatches;
        }
        if let Some(is_better) = compare_score_value(self.total_distance, current.total_distance) {
            return is_better;
        }
        false
    }
}

fn score_two_player_expansion_assignment(
    starts: [Tile; 2],
    expansions: [Tile; 2],
    authored_expansions: [Option<Tile>; 2],
) -> TwoPlayerExpansionScore {
    let left = local_expansion_coordinates(starts[0], starts[1], expansions[0]);
    let right = local_expansion_coordinates(starts[1], starts[0], expansions[1]);
    let local_symmetry_error =
        (left.forward - right.forward).abs() + (left.lateral.abs() - right.lateral.abs()).abs();
    let distance_error = (left.distance - right.distance).abs();
    let total_distance = left.distance + right.distance;
    let authored_mismatches = authored_expansions
        .iter()
        .zip(expansions.iter())
        .filter(|(authored, expansion)| match authored {
            Some(authored) => authored != *expansion,
            None => false,
        })
        .count() as u8;

    TwoPlayerExpansionScore {
        local_symmetry_error,
        distance_error,
        authored_mismatches,
        total_distance,
    }
}

#[derive(Debug, Clone, Copy)]
struct LocalExpansionCoordinates {
    forward: f32,
    lateral: f32,
    distance: f32,
}

fn local_expansion_coordinates(
    start: Tile,
    enemy_start: Tile,
    expansion: Tile,
) -> LocalExpansionCoordinates {
    let to_enemy_x = enemy_start.0 as f32 - start.0 as f32;
    let to_enemy_y = enemy_start.1 as f32 - start.1 as f32;
    let enemy_distance = (to_enemy_x * to_enemy_x + to_enemy_y * to_enemy_y).sqrt();
    if enemy_distance <= f32::EPSILON {
        return LocalExpansionCoordinates {
            forward: 0.0,
            lateral: 0.0,
            distance: 0.0,
        };
    }

    let unit_x = to_enemy_x / enemy_distance;
    let unit_y = to_enemy_y / enemy_distance;
    let expansion_x = expansion.0 as f32 - start.0 as f32;
    let expansion_y = expansion.1 as f32 - start.1 as f32;

    LocalExpansionCoordinates {
        forward: expansion_x * unit_x + expansion_y * unit_y,
        lateral: expansion_x * -unit_y + expansion_y * unit_x,
        distance: (expansion_x * expansion_x + expansion_y * expansion_y).sqrt(),
    }
}

const SCORE_EPSILON_TILES: f32 = 0.0001;

fn compare_score_value(candidate: f32, current: f32) -> Option<bool> {
    if candidate + SCORE_EPSILON_TILES < current {
        Some(true)
    } else if current + SCORE_EPSILON_TILES < candidate {
        Some(false)
    } else {
        None
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
        let radius = if i % 2 == 1 {
            EXPANSION_PROTECTION_RADIUS_TILES
        } else {
            BASE_PROTECTION_RADIUS_TILES
        };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardcoded_map_loads_for_every_supported_player_count() {
        for player_count in 1..=4 {
            let map = Map::generate(player_count, 0x1234_5678);
            assert_eq!(map.size, 126);
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
    fn bundled_map_catalog_loads_available_maps_by_name() {
        let available = Map::list_available();
        assert!(available.contains(&"default-handcrafted".to_string()));
        assert!(available.contains(&"no-terrain".to_string()));

        let map = Map::load("default-handcrafted", 2, 0x1234_5678)
            .expect("default handcrafted map should load from bundled assets");
        assert_eq!(map.size, 126);
        assert_eq!(map.starts.len(), 2);
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
        // All authored expansions from default-handcrafted.json.
        let authored_expansions: &[(u32, u32)] = &[(63, 38), (63, 88), (88, 62), (38, 62)];
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
    fn full_player_count_keeps_their_paired_natural_expansions() {
        let authored_pairs: HashSet<_> = default_authored_pairs().into_iter().collect();

        for seed in 0..16u32 {
            let map = Map::generate(4, seed);
            for pair in map
                .starts
                .iter()
                .copied()
                .zip(map.expansion_sites.iter().copied())
            {
                assert!(
                    authored_pairs.contains(&pair),
                    "start/expansion pair {pair:?} is not an authored natural pair (seed {seed})"
                );
            }
        }
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
    fn adjacent_two_player_starts_get_symmetric_natural_expansions() {
        let authored_pairs = default_authored_pairs();

        assert_eq!(
            select_symmetric_two_player_expansions([(25, 25), (100, 25)], &authored_pairs),
            vec![(38, 62), (88, 62)],
            "top-edge starts should both expand toward matching side naturals"
        );
        assert_eq!(
            select_symmetric_two_player_expansions([(25, 100), (100, 100)], &authored_pairs),
            vec![(38, 62), (88, 62)],
            "bottom-edge starts should both expand toward matching side naturals"
        );
        assert_eq!(
            select_symmetric_two_player_expansions([(25, 25), (25, 100)], &authored_pairs),
            vec![(63, 38), (63, 88)],
            "left-edge starts should both expand toward matching vertical naturals"
        );
        assert_eq!(
            select_symmetric_two_player_expansions([(100, 25), (100, 100)], &authored_pairs),
            vec![(63, 38), (63, 88)],
            "right-edge starts should both expand toward matching vertical naturals"
        );
    }

    #[test]
    fn generated_two_player_layouts_use_symmetric_natural_assignments() {
        for seed in 0..64u32 {
            let map = Map::generate(2, seed);
            let score = score_two_player_expansion_assignment(
                [map.starts[0], map.starts[1]],
                [map.expansion_sites[0], map.expansion_sites[1]],
                [None, None],
            );

            assert!(
                score.local_symmetry_error <= 2.0,
                "seed {seed} assigned asymmetric naturals: starts {:?}, expansions {:?}, score {:?}",
                map.starts,
                map.expansion_sites,
                score
            );
            assert!(
                score.distance_error <= 2.0,
                "seed {seed} assigned unequal natural distances: starts {:?}, expansions {:?}, score {:?}",
                map.starts,
                map.expansion_sites,
                score
            );
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

    fn default_authored_pairs() -> [BasePair; 4] {
        [
            ((25, 25), (63, 38)),
            ((100, 100), (63, 88)),
            ((100, 25), (88, 62)),
            ((25, 100), (38, 62)),
        ]
    }
}
