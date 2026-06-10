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
use std::path::PathBuf;

use crate::config;
use crate::protocol::terrain;
use rand::rngs::SmallRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

pub use rts_protocol::AvailableMap;

/// The only map schema version this server accepts. Bump when the schema changes incompatibly.
pub const CURRENT_MAP_VERSION: u32 = 1;

const DEFAULT_MAP_JSON: &str = include_str!("../../../../assets/maps/default-handcrafted.json");
const MAPS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/maps");
const DEFAULT_MAP_NAME: &str = "Default";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

type Tile = (u32, u32);
type BasePair = (Tile, Tile);

/// Radius around a player start site (even index) that must remain passable.
pub const BASE_PROTECTION_RADIUS_TILES: i32 = 7;
/// Radius around a natural expansion site (odd index) that must remain passable.
/// Smaller than the start radius because naturals have no City Centre or worker ring.
pub const EXPANSION_PROTECTION_RADIUS_TILES: i32 = 4;

/// The terrain grid plus the selected start and expansion tiles.
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MapMetadata {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

impl Map {
    /// Load the deterministic handcrafted map for `player_count` players.
    ///
    /// The `seed` is used to shuffle which authored base pair each player draws, so the
    /// human/AI seating in the lobby does not pin them to the same corner every match.
    pub fn generate(player_count: usize, seed: u32) -> Map {
        Self::from_authored_json_with_name(player_count, DEFAULT_MAP_NAME, DEFAULT_MAP_JSON, seed)
            .unwrap_or_else(|err| panic!("invalid hardcoded map asset: {err}"))
    }

    /// Return all available maps in `assets/maps/` as `(name, description)` entries. Only maps
    /// with the current schema version are included; version mismatches are silently skipped.
    /// Errors (unreadable directory or files) are silently skipped so a bad asset cannot crash the
    /// lobby.
    pub fn list_available() -> Vec<AvailableMap> {
        let Some(dir) = bundled_maps_dir() else {
            return vec![AvailableMap {
                name: DEFAULT_MAP_NAME.to_string(),
                description: DEFAULT_MAP_NAME.to_string(),
            }];
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![AvailableMap {
                name: DEFAULT_MAP_NAME.to_string(),
                description: DEFAULT_MAP_NAME.to_string(),
            }];
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .map(|e| e.path())
            .collect();
        paths.sort();
        let mut out: Vec<AvailableMap> = Vec::new();
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let Some(json) = std::fs::read_to_string(&path).ok() else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) else {
                continue;
            };
            // Skip maps that do not declare the current schema version.
            let version = v.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
            if version != CURRENT_MAP_VERSION as u64 {
                continue;
            }
            let name = v
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or(&stem)
                .to_string();
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or(&name)
                .to_string();
            if !name.is_empty() {
                out.push(AvailableMap { name, description });
            }
        }
        if out.is_empty() {
            out.push(AvailableMap {
                name: DEFAULT_MAP_NAME.to_string(),
                description: DEFAULT_MAP_NAME.to_string(),
            });
        }
        out
    }

    /// Load a map by display name (the `name` field in the JSON) for `player_count` players.
    /// Returns an error string if the map cannot be found, read, or parsed.
    pub fn load(map_name: &str, player_count: usize, seed: u32) -> Result<Map, String> {
        let (name, json) = Self::authored_json_for_name(map_name)?;
        Self::from_authored_json_with_name(player_count, &name, &json, seed)
    }

    pub fn metadata_for_name(map_name: &str) -> Result<MapMetadata, String> {
        let (name, json) = Self::authored_json_for_name(map_name)?;
        let authored: AuthoredMap =
            serde_json::from_str(&json).map_err(|err| format!("map JSON parse error: {err}"))?;
        Ok(MapMetadata {
            name,
            schema_version: authored.version,
            content_hash: stable_content_hash(&json),
        })
    }

    fn authored_json_for_name(map_name: &str) -> Result<(String, String), String> {
        // First try to match by `name` field, then by filename stem.
        if let Some(dir) = bundled_maps_dir() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                return Err(format!("cannot read maps directory: {}", dir.display()));
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
                    return Ok((json_name.unwrap_or(stem), json));
                }
            }
        }
        if map_name == DEFAULT_MAP_NAME {
            return Ok((DEFAULT_MAP_NAME.to_string(), DEFAULT_MAP_JSON.to_string()));
        }
        Err(format!("map not found: {map_name:?}"))
    }

    #[cfg(test)]
    fn from_authored_json(player_count: usize, json: &str, seed: u32) -> Result<Map, String> {
        Self::from_authored_json_with_name(player_count, DEFAULT_MAP_NAME, json, seed)
    }

    fn from_authored_json_with_name(
        player_count: usize,
        _name: &str,
        json: &str,
        seed: u32,
    ) -> Result<Map, String> {
        let authored: AuthoredMap =
            serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
        if authored.version != CURRENT_MAP_VERSION {
            return Err(format!(
                "map schema version {} is not supported; server requires version {CURRENT_MAP_VERSION}",
                authored.version
            ));
        }
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
        // match. Once a start is selected, its authored natural stays attached to it.
        let total_pairs = base_sites.len() / 2;
        let authored_pairs: Vec<BasePair> = (0..total_pairs)
            .map(|i| (base_sites[2 * i], base_sites[2 * i + 1]))
            .collect();
        let mut pairs = authored_pairs.clone();
        let mut rng = SmallRng::seed_from_u64(seed as u64);
        pairs.shuffle(&mut rng);
        let selected_pairs: Vec<_> = pairs.into_iter().take(player_count).collect();
        let starts: Vec<_> = selected_pairs.iter().map(|(start, _)| *start).collect();
        let expansion_sites = selected_pairs
            .iter()
            .map(|(_, expansion)| *expansion)
            .collect();

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

fn stable_content_hash(content: &str) -> String {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in content.as_bytes() {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn bundled_maps_dir() -> Option<PathBuf> {
    maps_dir_candidates().into_iter().find(|path| path.is_dir())
}

fn maps_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    candidates.push(PathBuf::from(MAPS_DIR));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("server/assets/maps"));
        candidates.push(cwd.join("assets/maps"));
    }
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors() {
            candidates.push(ancestor.join("server/assets/maps"));
            candidates.push(ancestor.join("assets/maps"));
        }
    }
    candidates
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredMap {
    version: u32,
    #[allow(dead_code)]
    name: String,
    /// Human-readable text shown in the lobby map selector.
    #[allow(dead_code)]
    description: String,
    /// Private invariants memo used by agents when porting a map to a new schema version.
    #[allow(dead_code)]
    #[serde(rename = "_design")]
    design: String,
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
        assert!(
            !available.is_empty(),
            "lobby map catalog must expose at least one selectable map"
        );
        let names: Vec<&str> = available.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Default"), "got: {names:?}");
        assert!(names.contains(&"No Terrain"), "got: {names:?}");
        // Every entry must have a non-empty description.
        for entry in &available {
            assert!(
                !entry.description.is_empty(),
                "missing description on {}",
                entry.name
            );
        }

        let map = Map::load("Default", 2, 0x1234_5678)
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
        let authored_pairs: HashSet<_> = default_authored_pairs().into_iter().collect();

        for seed in 0..16u32 {
            let map = Map::generate(2, seed);
            assert_eq!(map.starts.len(), 2);
            assert_eq!(map.expansion_sites.len(), 2);
            for pair in map
                .starts
                .iter()
                .copied()
                .zip(map.expansion_sites.iter().copied())
            {
                assert!(
                    authored_pairs.contains(&pair),
                    "two-player start/expansion pair {pair:?} is not an authored natural pair (seed {seed})"
                );
            }
        }
    }

    #[test]
    fn authored_map_rejects_wrong_version() {
        let err = Map::from_authored_json(
            1,
            r#"{
              "version": 99,
              "name": "future",
              "description": "a future map",
              "_design": "n/a",
              "terrain": [".."],
              "baseSites": [{"x": 0, "y": 0}, {"x": 1, "y": 0}]
            }"#,
            0,
        )
        .expect_err("wrong version should be rejected");

        assert!(err.contains("not supported"), "error was: {err}");
    }

    #[test]
    fn authored_map_rejects_unknown_terrain_characters() {
        let err = Map::from_authored_json(
            1,
            r#"{
              "version": 1,
              "name": "bad",
              "description": "bad map",
              "_design": "n/a",
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
              "version": 1,
              "name": "bad-base",
              "description": "bad base map",
              "_design": "n/a",
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
