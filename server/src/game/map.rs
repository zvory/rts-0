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
    /// The `seed` parameter is intentionally ignored while the hardcoded map is active; it remains
    /// in the API so replay/lobby callers do not need to change before lobby map selection exists.
    pub fn generate(player_count: usize, _seed: u32) -> Map {
        Self::from_authored_json(player_count, DEFAULT_MAP_JSON)
            .unwrap_or_else(|err| panic!("invalid hardcoded map asset: {err}"))
    }

    fn from_authored_json(player_count: usize, json: &str) -> Result<Map, String> {
        let authored: AuthoredMap =
            serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
        let (size, terrain) = parse_terrain(&authored.terrain)?;
        let base_sites = parse_base_sites(size, &authored.base_sites)?;

        if player_count == 0 {
            return Err("player_count must be at least 1".to_string());
        }
        if player_count > base_sites.len() {
            return Err(format!(
                "map has {} base sites, but {player_count} players were requested",
                base_sites.len()
            ));
        }

        validate_base_clearance(size, &terrain, &base_sites)?;

        let starts = base_sites.iter().take(player_count).copied().collect();
        let expansion_sites = base_sites.iter().skip(player_count).copied().collect();

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
    fn hardcoded_map_is_deterministic_across_seeds() {
        let a = Map::generate(4, 1);
        let b = Map::generate(4, 2);

        assert_eq!(a.size, b.size);
        assert_eq!(a.terrain, b.terrain);
        assert_eq!(a.starts, b.starts);
        assert_eq!(a.expansion_sites, b.expansion_sites);
    }

    #[test]
    fn two_player_match_uses_opposite_authored_starts() {
        let map = Map::generate(2, 0);
        assert_eq!(map.starts, vec![(10, 10), (85, 85)]);
        assert!(map.expansion_sites.contains(&(85, 10)));
        assert!(map.expansion_sites.contains(&(10, 85)));
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
        )
        .expect_err("bad terrain should be rejected");

        assert!(err.contains("unknown terrain character"));
    }

    #[test]
    fn authored_map_rejects_impassable_base_protection_area() {
        let mut rows = vec!["................".to_string(); 16];
        rows[8].replace_range(8..9, "#");
        let json = format!(
            r#"{{
              "name": "bad-base",
              "terrain": {},
              "baseSites": [{{"x": 8, "y": 8}}]
            }}"#,
            serde_json::to_string(&rows).unwrap()
        );

        let err = Map::from_authored_json(1, &json)
            .expect_err("blocked base protection area should be rejected");

        assert!(err.contains("impassable terrain"));
    }
}
