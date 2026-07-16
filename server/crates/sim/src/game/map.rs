//! The tile map: terrain grid, authored map loading, and passability. See
//! `docs/design/server-sim.md` (`map.rs`).
//!
//! The live game loads authored maps from the server asset bundle. Map files define terrain,
//! flat start locations and permanent base sites. The simulation assigns players to start
//! locations, while every base site receives its resource cluster in every match.
//!
//! Terrain passability here is purely about *terrain* — building footprints are tracked
//! dynamically by the simulation (a separate occupancy grid in `systems`/`pathfinding`),
//! not baked into the map.

use std::path::PathBuf;

mod authored;
#[cfg(test)]
mod team_assignment_tests;

use crate::config;
use crate::protocol::terrain;
use crate::rules::terrain as terrain_rules;
use serde::{Deserialize, Serialize};

pub use rts_protocol::AvailableMap;

/// The only map schema version this server accepts. Bump when the schema changes incompatibly.
pub const CURRENT_MAP_VERSION: u32 = 3;

const DEFAULT_MAP_JSON: &str = include_str!("../../../../assets/maps/default-handcrafted.json");
const MAPS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/maps");
const DEFAULT_MAP_NAME: &str = "Default";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

type Tile = (u32, u32);
/// Ordered player/team data used internally to assign authored start locations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StartAssignmentPlayer {
    id: u32,
    team_id: u32,
}

/// Radius around a player start location that must remain passable.
pub const BASE_PROTECTION_RADIUS_TILES: i32 = 7;
/// Radius around a permanent base site that does not host a player at launch.
/// Smaller than the start radius because it has no City Centre or worker ring.
pub const BASE_SITE_PROTECTION_RADIUS_TILES: i32 = 4;

/// The terrain grid, selected player starts, and every authored permanent base site.
#[derive(Debug, Clone)]
pub struct Map {
    /// Side length in tiles (square map).
    pub size: u32,
    /// Row-major terrain codes, length `size * size`.
    pub terrain: Vec<u8>,
    /// One start tile `(tile_x, tile_y)` per player, in player-index order.
    pub starts: Vec<(u32, u32)>,
    /// Every authored base location. These always receive resource clusters; selected starts
    /// additionally receive a player's starting buildings and workers.
    pub base_sites: Vec<(u32, u32)>,
}

/// Canonical materialization of an authored-map document before player starts are assigned.
///
/// HTTP/session boundaries use this to bind an untrusted authored document to an equivalent
/// wire-format draft without maintaining a second copy of the authored-map decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredMapData {
    pub name: String,
    pub size: u32,
    pub terrain: Vec<u8>,
    pub starts: Vec<(u32, u32)>,
    pub base_sites: Vec<(u32, u32)>,
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
    /// The `seed` selects and shuffles from fixed authored start locations, so the human/AI
    /// seating in the lobby does not pin them to the same corner every match.
    pub fn generate(player_count: usize, seed: u32) -> Map {
        Self::from_authored_json_with_name(player_count, DEFAULT_MAP_NAME, DEFAULT_MAP_JSON, seed)
            .unwrap_or_else(|err| panic!("invalid hardcoded map asset: {err}"))
    }

    /// Load the deterministic handcrafted map and assign starts to the ordered players.
    pub(crate) fn generate_for_players(players: &[(u32, u32)], seed: u32) -> Map {
        Self::from_authored_json_with_name_for_players(
            players,
            DEFAULT_MAP_NAME,
            DEFAULT_MAP_JSON,
            seed,
        )
        .unwrap_or_else(|err| panic!("invalid hardcoded map asset: {err}"))
    }

    /// Return all available maps in `assets/maps/` as `(name, description)` entries. Only maps
    /// with the current schema version are included; version mismatches are silently skipped.
    /// Errors (unreadable directory or files) are silently skipped so a bad asset cannot crash the
    /// lobby.
    pub fn list_available() -> Vec<AvailableMap> {
        let Some(dir) = bundled_maps_dir() else {
            return vec![default_available_map()];
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![default_available_map()];
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
            if let Some(entry) = available_map_from_json(&stem, &json) {
                out.push(entry);
            }
        }
        if out.is_empty() {
            out.push(default_available_map());
        }
        out
    }

    /// Load a map by display name (the `name` field in the JSON) for `player_count` players.
    /// Returns an error string if the map cannot be found, read, or parsed.
    pub fn load(map_name: &str, player_count: usize, seed: u32) -> Result<Map, String> {
        let (name, json) = Self::authored_json_for_name(map_name)?;
        Self::from_authored_json_with_name(player_count, &name, &json, seed)
    }

    /// Load a map by display name and assign starts to the ordered players.
    pub fn load_for_players(
        map_name: &str,
        players: &[(u32, u32)],
        seed: u32,
    ) -> Result<Map, String> {
        let (name, json) = Self::authored_json_for_name(map_name)?;
        Self::from_authored_json_with_name_for_players(players, &name, &json, seed)
    }

    /// Validate and materialize an untrusted authored-map document with the same parser and
    /// location rules used by bundled maps. Starts remain in authored order; live map loading
    /// assigns them to players separately.
    pub fn materialize_authored_json(
        json: &str,
        player_count: usize,
    ) -> Result<AuthoredMapData, String> {
        authored::materialize(player_count, json)
    }

    pub fn metadata_for_name(map_name: &str) -> Result<MapMetadata, String> {
        let (name, json) = Self::authored_json_for_name(map_name)?;
        Ok(MapMetadata {
            name,
            schema_version: authored::schema_version(&json)?,
            content_hash: stable_content_hash(&json),
        })
    }

    pub(crate) fn materialized_hash(&self) -> String {
        let mut hash = FNV_OFFSET_BASIS;
        hash = fnv_bytes(hash, &self.size.to_le_bytes());
        hash = fnv_bytes(hash, &self.terrain);
        hash = fnv_usize(hash, self.starts.len());
        for &(x, y) in &self.starts {
            hash = fnv_bytes(hash, &x.to_le_bytes());
            hash = fnv_bytes(hash, &y.to_le_bytes());
        }
        hash = fnv_usize(hash, self.base_sites.len());
        for &(x, y) in &self.base_sites {
            hash = fnv_bytes(hash, &x.to_le_bytes());
            hash = fnv_bytes(hash, &y.to_le_bytes());
        }
        format!("{hash:016x}")
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
        authored::load(player_count, json, seed)
    }

    fn from_authored_json_with_name_for_players(
        players: &[(u32, u32)],
        _name: &str,
        json: &str,
        seed: u32,
    ) -> Result<Map, String> {
        let players: Vec<_> = players
            .iter()
            .map(|(id, team_id)| StartAssignmentPlayer {
                id: *id,
                team_id: *team_id,
            })
            .collect();
        authored::load_for_players(&players, json, seed)
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

    /// Whether a tile is passable terrain. Out-of-bounds is impassable. This does NOT
    /// account for building footprints — callers combine this with the dynamic occupancy grid.
    #[inline]
    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        if !self.in_bounds(x, y) {
            return false;
        }
        terrain_rules::is_passable_map_code(self.terrain_at(x as u32, y as u32))
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
    format!("{:016x}", fnv_bytes(FNV_OFFSET_BASIS, content.as_bytes()))
}

fn default_available_map() -> AvailableMap {
    available_map_from_json(DEFAULT_MAP_NAME, DEFAULT_MAP_JSON).unwrap_or_else(|| AvailableMap {
        name: DEFAULT_MAP_NAME.to_string(),
        description: DEFAULT_MAP_NAME.to_string(),
        min_players: 1,
        max_players: 4,
    })
}

fn available_map_from_json(stem: &str, json: &str) -> Option<AvailableMap> {
    let v = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let version = v.get("version").and_then(|v| v.as_u64()).unwrap_or(0);
    if version != CURRENT_MAP_VERSION as u64 {
        return None;
    }
    let name = v
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(stem)
        .to_string();
    let description = v
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or(&name)
        .to_string();
    let (min_players, max_players) = authored::player_count_bounds(json).ok()?;
    (!name.is_empty()).then_some(AvailableMap {
        name,
        description,
        min_players,
        max_players,
    })
}

fn fnv_usize(hash: u64, value: usize) -> u64 {
    fnv_bytes(hash, &(value as u64).to_le_bytes())
}

fn fnv_bytes(mut hash: u64, bytes: &[u8]) -> u64 {
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME);
    }
    hash
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    mod base_limits;
    mod four_player;

    #[test]
    fn hardcoded_map_loads_for_every_supported_player_count() {
        for player_count in 1..=4 {
            let map = Map::generate(player_count, 0x1234_5678);
            assert_eq!(map.size, 126);
            assert_eq!(map.terrain.len(), (map.size * map.size) as usize);
            assert_eq!(map.starts.len(), player_count);
            assert!(!map.base_sites.is_empty());

            for start in &map.starts {
                assert!(map.is_passable(start.0 as i32, start.1 as i32));
            }
            for expansion in &map.base_sites {
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
        assert!(names.contains(&"1v1"), "got: {names:?}");
        assert!(names.contains(&"Low Econ"), "got: {names:?}");
        assert!(names.contains(&"No Terrain"), "got: {names:?}");
        assert!(names.contains(&"1v1 No Terrain"), "got: {names:?}");
        assert!(names.contains(&"4 Player Map"), "got: {names:?}");
        // Every entry must have a non-empty description.
        for entry in &available {
            assert!(
                !entry.description.is_empty(),
                "missing description on {}",
                entry.name
            );
            assert!(
                entry.min_players >= 1 && entry.min_players <= entry.max_players,
                "bad player bounds on {}: {}..={}",
                entry.name,
                entry.min_players,
                entry.max_players
            );
        }

        let map = Map::load("Default", 2, 0x1234_5678)
            .expect("default handcrafted map should load from bundled assets");
        assert_eq!(map.size, 126);
        assert_eq!(map.starts.len(), 2);

        let one_v_one_authored = available
            .iter()
            .find(|entry| entry.name == "1v1")
            .expect("imported 1v1 map should be listed");
        assert_eq!(one_v_one_authored.min_players, 1);
        assert_eq!(one_v_one_authored.max_players, 2);
        let one_v_one_map =
            Map::load("1v1", 2, 0x1234_5678).expect("1v1 should load for two active players");
        assert_eq!(
            one_v_one_map.base_sites.len(),
            10,
            "1v1 must retain all ten permanent resource bases"
        );
        assert!(
            Map::load("1v1", 3, 0x1234_5678).is_err(),
            "1v1 should not expose a third start location"
        );
        for seed in 0..32 {
            let mut starts = Map::load("1v1", 2, seed)
                .expect("1v1 should load for two active players")
                .starts;
            starts.sort_unstable();
            assert_eq!(
                starts,
                vec![(9, 9), (116, 116)],
                "1v1 must only use its two authored start locations for seed {seed}"
            );
        }

        let one_v_one = available
            .iter()
            .find(|entry| entry.name == "1v1 No Terrain")
            .expect("1v1 no-terrain scaffold should be listed");
        assert_eq!(one_v_one.min_players, 1);
        assert_eq!(one_v_one.max_players, 2);
        assert!(
            Map::load("1v1 No Terrain", 2, 0x1234_5678).is_ok(),
            "1v1 No Terrain should load for two active players"
        );
        assert!(
            Map::load("1v1 No Terrain", 3, 0x1234_5678).is_err(),
            "1v1 No Terrain should not expose a third start location"
        );

        for seed in 0..32 {
            let mut starts = Map::load("1v1 No Terrain", 2, seed)
                .expect("1v1 No Terrain should load for two active players")
                .starts;
            starts.sort_unstable();
            assert_eq!(
                starts,
                vec![(25, 25), (100, 100)],
                "1v1 No Terrain must only use its two opposing start locations for seed {seed}"
            );
        }

        let four_player = available
            .iter()
            .find(|entry| entry.name == "4 Player Map")
            .expect("four-player map should be listed");
        assert_eq!(four_player.min_players, 1);
        assert_eq!(four_player.max_players, 4);
        for player_count in 1..=4 {
            let map = Map::load("4 Player Map", player_count, 0x1234_5678)
                .expect("four-player map should load for every supported player count");
            assert_eq!(map.size, 166);
            assert_eq!(map.starts.len(), player_count);
            assert_eq!(map.base_sites.len(), 16);
        }
    }

    #[test]
    fn one_v_one_map_is_rotationally_symmetric() {
        let map = Map::load("1v1", 2, 0x1234_5678).expect("1v1 should load");
        let size = map.size as usize;

        for y in 0..size {
            for x in 0..size {
                let rotated_x = size - 1 - x;
                let rotated_y = size - 1 - y;
                assert_eq!(
                    map.terrain[y * size + x],
                    map.terrain[rotated_y * size + rotated_x],
                    "1v1 terrain differs at ({x},{y}) and its rotation ({rotated_x},{rotated_y})"
                );
            }
        }

        let starts: HashSet<_> = map.starts.iter().copied().collect();
        for &(x, y) in &map.starts {
            assert!(
                starts.contains(&(map.size - 1 - x, map.size - 1 - y)),
                "1v1 start ({x},{y}) has no rotational counterpart"
            );
        }

        let base_sites: HashSet<_> = map.base_sites.iter().copied().collect();
        for &(x, y) in &map.base_sites {
            assert!(
                base_sites.contains(&(map.size - 1 - x, map.size - 1 - y)),
                "1v1 base site ({x},{y}) has no rotational counterpart"
            );
        }
    }

    #[test]
    fn same_seed_produces_identical_start_assignment() {
        let a = Map::generate(4, 0xdead_beef);
        let b = Map::generate(4, 0xdead_beef);

        assert_eq!(a.size, b.size);
        assert_eq!(a.terrain, b.terrain);
        assert_eq!(a.starts, b.starts);
        assert_eq!(a.base_sites, b.base_sites);
    }

    #[test]
    fn different_seeds_produce_different_start_orderings() {
        // With four fixed start locations, scanning a handful of seeds must yield more than one
        // two-player ordering. If shuffling silently breaks this
        // catches it.
        let assignments: HashSet<Vec<(u32, u32)>> = (0..32u32)
            .map(|seed| Map::generate(2, seed).starts)
            .collect();
        assert!(
            assignments.len() > 1,
            "expected at least two distinct start orderings across seeds, got {assignments:?}"
        );
    }

    #[test]
    fn every_authored_base_is_present_for_every_player_count() {
        let expected: HashSet<_> = [
            (13, 12),
            (112, 113),
            (112, 12),
            (13, 113),
            (40, 11),
            (85, 114),
            (114, 40),
            (11, 85),
            (63, 38),
            (63, 87),
            (87, 63),
            (38, 63),
        ]
        .into_iter()
        .collect();
        for seed in 0..16u32 {
            for player_count in 1..=4 {
                let map = Map::generate(player_count, seed);
                assert_eq!(
                    map.base_sites.iter().copied().collect::<HashSet<_>>(),
                    expected
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
              "startLocations": [{"x": 0, "y": 0}],
              "baseSites": [{"x": 0, "y": 0}]
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
              "version": 3,
              "name": "bad",
              "description": "bad map",
              "_design": "n/a",
              "terrain": ["..", ".x"],
              "startLocations": [{"x": 0, "y": 0}],
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
        let mut rows = vec![".".repeat(32); 32];
        rows[8].replace_range(8..9, "#");
        let json = format!(
            r#"{{
              "version": 3,
              "name": "bad-base",
              "description": "bad base map",
              "_design": "n/a",
              "terrain": {},
              "startLocations": [{{"x": 8, "y": 8}}],
              "baseSites": [{{"x": 8, "y": 8}}, {{"x": 24, "y": 24}}]
            }}"#,
            serde_json::to_string(&rows).unwrap()
        );

        let err = Map::from_authored_json(1, &json, 0)
            .expect_err("blocked base protection area should be rejected");

        assert!(err.contains("impassable terrain"));
    }

    #[test]
    fn authored_map_accepts_roads_as_passable_base_terrain() {
        let mut rows = vec![".".repeat(32); 32];
        rows[8].replace_range(8..9, "=");
        let json = format!(
            r#"{{
              "version": 3,
              "name": "road-base",
              "description": "road through a base",
              "_design": "n/a",
              "terrain": {},
              "startLocations": [{{"x": 8, "y": 8}}],
              "baseSites": [{{"x": 8, "y": 8}}, {{"x": 24, "y": 24}}]
            }}"#,
            serde_json::to_string(&rows).unwrap()
        );

        let map = Map::from_authored_json(1, &json, 0).expect("road should be passable");
        assert_eq!(map.terrain_at(8, 8), terrain::ROAD_BARE);
        assert!(map.is_passable(8, 8));
    }
}
