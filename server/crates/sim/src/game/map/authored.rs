use std::collections::HashSet;

use serde::Deserialize;

mod assignment;

use super::{
    Map, StartAssignmentPlayer, BASE_PROTECTION_RADIUS_TILES, BASE_SITE_PROTECTION_RADIUS_TILES,
    CURRENT_MAP_VERSION,
};
use crate::protocol::terrain;

/// Bound authored locations before any game entities are allocated from them. The game currently
/// supports four active players, while a map can contain many more permanent resource bases.
const MAX_START_LOCATIONS: usize = 4;
const MAX_BASE_SITES: usize = 32;

pub(super) fn schema_version(json: &str) -> Result<u32, String> {
    let authored: AuthoredMap =
        serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
    Ok(authored.version)
}

pub(super) fn player_count_bounds(json: &str) -> Result<(u32, u32), String> {
    let authored: AuthoredMap =
        serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
    if authored.version != CURRENT_MAP_VERSION {
        return Err(format!(
            "map schema version {} is not supported; server requires version {CURRENT_MAP_VERSION}",
            authored.version
        ));
    }
    let starts = authored.start_locations.len();
    if starts == 0 || starts > MAX_START_LOCATIONS {
        return Err(format!(
            "startLocations must contain 1 to {MAX_START_LOCATIONS} locations"
        ));
    }
    Ok((1, starts as u32))
}

pub(super) fn load(player_count: usize, json: &str, seed: u32) -> Result<Map, String> {
    let players: Vec<_> = (1..=player_count)
        .map(|id| StartAssignmentPlayer {
            id: id as u32,
            team_id: id as u32,
        })
        .collect();
    load_for_players(&players, json, seed)
}

pub(super) fn load_for_players(
    players: &[StartAssignmentPlayer],
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
    let start_locations = parse_locations(size, &authored.start_locations, "startLocations")?;
    let base_sites = parse_locations(size, &authored.base_sites, "baseSites")?;

    if players.is_empty() {
        return Err("player_count must be at least 1".to_string());
    }
    if start_locations.is_empty() || start_locations.len() > MAX_START_LOCATIONS {
        return Err(format!(
            "startLocations must contain 1 to {MAX_START_LOCATIONS} locations"
        ));
    }
    if base_sites.is_empty() || base_sites.len() > MAX_BASE_SITES {
        return Err(format!(
            "baseSites must contain 1 to {MAX_BASE_SITES} locations"
        ));
    }
    if players.len() > start_locations.len() {
        return Err(format!(
            "map has {} start locations but needs {} players",
            start_locations.len(),
            players.len()
        ));
    }

    let base_set: HashSet<_> = base_sites.iter().copied().collect();
    for start in &start_locations {
        if !base_set.contains(start) {
            return Err(format!(
                "start location ({},{}) is not also a permanent base site",
                start.0, start.1
            ));
        }
    }
    validate_base_clearance(size, &terrain, &start_locations, &base_sites)?;
    let starts = assignment::assign_start_locations(&start_locations, players, seed)?;

    Ok(Map {
        size,
        terrain,
        starts,
        base_sites,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthoredMap {
    version: u32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    #[serde(rename = "_design")]
    design: String,
    terrain: Vec<String>,
    start_locations: Vec<AuthoredLocation>,
    base_sites: Vec<AuthoredLocation>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredLocation {
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
                '=' => terrain::ROAD_BARE,
                '-' => terrain::ROAD_HORIZONTAL,
                '|' => terrain::ROAD_VERTICAL,
                '\\' => terrain::ROAD_DIAGONAL_NW_SE,
                '/' => terrain::ROAD_DIAGONAL_NE_SW,
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

fn parse_locations(
    size: u32,
    authored: &[AuthoredLocation],
    field: &str,
) -> Result<Vec<(u32, u32)>, String> {
    let mut locations = Vec::with_capacity(authored.len());
    let mut seen = HashSet::with_capacity(authored.len());
    for (index, location) in authored.iter().enumerate() {
        if location.x >= size || location.y >= size {
            return Err(format!(
                "{field}[{index}] = ({},{}) is outside the {size}x{size} map",
                location.x, location.y
            ));
        }
        if !seen.insert((location.x, location.y)) {
            return Err(format!(
                "{field}[{index}] duplicates an earlier location at ({},{})",
                location.x, location.y
            ));
        }
        locations.push((location.x, location.y));
    }
    Ok(locations)
}

fn validate_base_clearance(
    size: u32,
    terrain_grid: &[u8],
    start_locations: &[(u32, u32)],
    base_sites: &[(u32, u32)],
) -> Result<(), String> {
    let starts: HashSet<_> = start_locations.iter().copied().collect();
    for &(sx, sy) in base_sites {
        let radius = if starts.contains(&(sx, sy)) {
            BASE_PROTECTION_RADIUS_TILES
        } else {
            BASE_SITE_PROTECTION_RADIUS_TILES
        };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let tx = sx as i32 + dx;
                let ty = sy as i32 + dy;
                if tx < 0 || ty < 0 || tx >= size as i32 || ty >= size as i32 {
                    return Err(format!(
                        "base site ({sx},{sy}) is too close to the map edge"
                    ));
                }
                let idx = (ty as u32 * size + tx as u32) as usize;
                if !crate::rules::terrain::is_passable_map_code(terrain_grid[idx]) {
                    return Err(format!(
                        "base site ({sx},{sy}) has impassable terrain in its protected area at ({tx},{ty})"
                    ));
                }
            }
        }
    }
    Ok(())
}
