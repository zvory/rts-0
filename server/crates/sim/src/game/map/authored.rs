use std::collections::HashSet;

use serde::Deserialize;

mod assignment;
mod forest_spans;

use forest_spans::{merge_overlay_locations, parse_forest_spans};

use super::{
    supported_map_version, AuthoredMapData, BaseResourceCounts, Map, StartAssignmentPlayer,
    BASE_PROTECTION_RADIUS_TILES, BASE_SITE_PROTECTION_RADIUS_TILES, CURRENT_MAP_VERSION,
};
use crate::protocol::terrain;
use rts_protocol::{MapSun, MAX_OIL_PATCHES_PER_BASE, MAX_STEEL_PATCHES_PER_BASE};

/// Bound authored locations before any game entities are allocated from them. The game currently
/// supports four active players, while a map can contain many more permanent resource bases.
const MAX_START_LOCATIONS: usize = 4;
const MAX_BASE_SITES: usize = 32;
const MAX_MAP_DIMENSION_TILES: usize = 256;

pub(super) fn schema_version(json: &str) -> Result<u32, String> {
    let authored: AuthoredMap =
        serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
    Ok(authored.version)
}

pub(super) fn player_count_bounds(json: &str) -> Result<(u32, u32), String> {
    let authored: AuthoredMap =
        serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
    if !supported_map_version(authored.version) {
        return Err(format!(
            "map schema version {} is not supported; server requires version {CURRENT_MAP_VERSION}",
            authored.version
        ));
    }
    if authored.forest_spans.is_none() {
        return Err("map forestSpans must be an array".to_string());
    }
    if authored.no_building_tiles.is_none() {
        return Err("map noBuildingTiles must be an array".to_string());
    }
    if authored.no_entrenchment_tiles.is_none() {
        return Err("map noEntrenchmentTiles must be an array".to_string());
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
    let materialized = materialize(players.len(), json)?;
    let starts = assignment::assign_start_locations(&materialized.starts, players, seed)?;

    Ok(Map {
        width: materialized.width,
        height: materialized.height,
        terrain: materialized.terrain,
        elevation: materialized.elevation,
        sun: materialized.sun,
        starts,
        base_sites: materialized.base_sites,
        base_resource_counts: materialized.base_resource_counts,
        doodads: materialized.doodads,
        concealment_tiles: materialized.concealment_tiles,
        no_vehicle_tiles: materialized.no_vehicle_tiles,
        no_building_tiles: materialized.no_building_tiles,
        no_entrenchment_tiles: materialized.no_entrenchment_tiles,
        damage_reduction_tiles: materialized.damage_reduction_tiles,
        slow_movement_tiles: materialized.slow_movement_tiles,
    })
}

pub(super) fn materialize(player_count: usize, json: &str) -> Result<AuthoredMapData, String> {
    let authored: AuthoredMap =
        serde_json::from_str(json).map_err(|err| format!("map JSON parse error: {err}"))?;
    if !supported_map_version(authored.version) {
        return Err(format!(
            "map schema version {} is not supported; server requires version {CURRENT_MAP_VERSION}",
            authored.version
        ));
    }
    let (width, height, terrain) = parse_terrain(&authored.terrain)?;
    if authored.width != width || authored.height != height {
        return Err(format!(
            "map width/height must match the {width}x{height} terrain grid"
        ));
    }
    let elevation = parse_elevation(&authored.elevation, width, height)?;
    super::validate_elevation_sun(&elevation, authored.sun)?;
    let start_locations =
        parse_locations(width, height, &authored.start_locations, "startLocations")?;
    let base_sites = parse_base_sites(width, height, &authored.base_sites)?;
    let base_locations: Vec<_> = base_sites.iter().map(|site| (site.x, site.y)).collect();

    if player_count == 0 {
        return Err("player_count must be at least 1".to_string());
    }
    if start_locations.is_empty() || start_locations.len() > MAX_START_LOCATIONS {
        return Err(format!(
            "startLocations must contain 1 to {MAX_START_LOCATIONS} locations"
        ));
    }
    if base_locations.is_empty() || base_locations.len() > MAX_BASE_SITES {
        return Err(format!(
            "baseSites must contain 1 to {MAX_BASE_SITES} locations"
        ));
    }
    if player_count > start_locations.len() {
        return Err(format!(
            "map has {} start locations but needs {} players",
            start_locations.len(),
            player_count
        ));
    }

    let base_set: HashSet<_> = base_locations.iter().copied().collect();
    for start in &start_locations {
        if !base_set.contains(start) {
            return Err(format!(
                "start location ({},{}) is not also a permanent base site",
                start.0, start.1
            ));
        }
    }
    validate_base_clearance(width, height, &terrain, &start_locations, &base_locations)?;
    let base_resource_counts = base_sites
        .into_iter()
        .map(|site| {
            (
                (site.x, site.y),
                BaseResourceCounts {
                    steel_patches: site.steel_patches,
                    oil_patches: site.oil_patches,
                },
            )
        })
        .collect();
    let doodads = super::doodads::canonicalize(width, height, authored.doodads)?;
    let forest_spans = authored
        .forest_spans
        .as_deref()
        .ok_or_else(|| "map forestSpans must be an array".to_string())?;
    let forest_tiles = parse_forest_spans(width, height, forest_spans)?;
    let materialize_overlay = |locations: &[AuthoredLocation], field| {
        parse_overlay_locations(width, height, locations, field)
            .map(|locations| merge_overlay_locations(locations, &forest_tiles))
    };
    let concealment_tiles = materialize_overlay(&authored.concealment_tiles, "concealmentTiles")?;
    let no_vehicle_tiles = materialize_overlay(&authored.no_vehicle_tiles, "noVehicleTiles")?;
    let no_building_tiles = materialize_overlay(
        authored
            .no_building_tiles
            .as_deref()
            .ok_or_else(|| "map noBuildingTiles must be an array".to_string())?,
        "noBuildingTiles",
    )?;
    // Forest remains the established five-effect composite. No-entrenchment is independent and
    // is authored explicitly (including the automatic records generated beneath road terrain).
    let no_entrenchment_tiles = parse_overlay_locations(
        width,
        height,
        authored
            .no_entrenchment_tiles
            .as_deref()
            .ok_or_else(|| "map noEntrenchmentTiles must be an array".to_string())?,
        "noEntrenchmentTiles",
    )?;
    let damage_reduction_tiles =
        materialize_overlay(&authored.damage_reduction_tiles, "damageReductionTiles")?;
    let slow_movement_tiles =
        materialize_overlay(&authored.slow_movement_tiles, "slowMovementTiles")?;
    Ok(AuthoredMapData {
        name: authored.name,
        width,
        height,
        terrain,
        elevation,
        sun: authored.sun,
        starts: start_locations,
        base_sites: base_locations,
        base_resource_counts,
        doodads,
        concealment_tiles,
        no_vehicle_tiles,
        no_building_tiles,
        no_entrenchment_tiles,
        damage_reduction_tiles,
        slow_movement_tiles,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthoredMap {
    version: u32,
    name: String,
    width: u32,
    height: u32,
    #[allow(dead_code)]
    description: String,
    #[allow(dead_code)]
    #[serde(rename = "_design")]
    design: String,
    terrain: Vec<String>,
    #[serde(default)]
    elevation: Vec<String>,
    #[serde(default)]
    sun: Option<MapSun>,
    start_locations: Vec<AuthoredLocation>,
    base_sites: Vec<AuthoredBaseSite>,
    #[serde(default)]
    doodads: Vec<crate::protocol::MapDoodad>,
    forest_spans: Option<Vec<[u32; 3]>>,
    #[serde(default)]
    concealment_tiles: Vec<AuthoredLocation>,
    #[serde(default)]
    no_vehicle_tiles: Vec<AuthoredLocation>,
    no_building_tiles: Option<Vec<AuthoredLocation>>,
    no_entrenchment_tiles: Option<Vec<AuthoredLocation>>,
    #[serde(default)]
    damage_reduction_tiles: Vec<AuthoredLocation>,
    #[serde(default)]
    slow_movement_tiles: Vec<AuthoredLocation>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredLocation {
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AuthoredBaseSite {
    x: u32,
    y: u32,
    steel_patches: u32,
    oil_patches: u32,
}

fn parse_terrain(rows: &[String]) -> Result<(u32, u32, Vec<u8>), String> {
    if rows.is_empty() {
        return Err("terrain must contain at least one row".to_string());
    }

    let width = rows[0].chars().count();
    if width == 0 {
        return Err("terrain rows must not be empty".to_string());
    }
    if width > MAX_MAP_DIMENSION_TILES || rows.len() > MAX_MAP_DIMENSION_TILES {
        return Err(format!(
            "terrain width and height must each be at most {MAX_MAP_DIMENSION_TILES} tiles"
        ));
    }
    let width_u32 =
        u32::try_from(width).map_err(|_| "terrain width does not fit in u32".to_string())?;
    let height_u32 =
        u32::try_from(rows.len()).map_err(|_| "terrain height does not fit in u32".to_string())?;
    let capacity = width.checked_mul(rows.len()).ok_or_else(|| {
        "terrain width multiplied by height overflows addressable memory".to_string()
    })?;
    let mut out = Vec::with_capacity(capacity);
    for (y, row) in rows.iter().enumerate() {
        let width = row.chars().count();
        if width != width_u32 as usize {
            return Err(format!(
                "terrain row {y} has width {width}; expected {width_u32}"
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
                '0'..='9' => terrain::GRAVEL_A + (ch as u8 - b'0'),
                _ => {
                    return Err(format!(
                        "unknown terrain character '{ch}' at tile ({x},{y})"
                    ))
                }
            };
            out.push(code);
        }
    }

    Ok((width_u32, height_u32, out))
}

fn parse_elevation(rows: &[String], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let tile_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| {
            "elevation width multiplied by height overflows addressable memory".to_string()
        })?;
    if rows.is_empty() {
        return Ok(vec![0; tile_count]);
    }
    if rows.len() != height as usize {
        return Err(format!(
            "elevation has {} rows; expected {height}",
            rows.len()
        ));
    }

    let mut out = Vec::with_capacity(tile_count);
    for (y, row) in rows.iter().enumerate() {
        let row_width = row.chars().count();
        if row_width != width as usize {
            return Err(format!(
                "elevation row {y} has width {row_width}; expected {width}"
            ));
        }
        for (x, ch) in row.chars().enumerate() {
            let level = ch
                .to_digit(10)
                .ok_or_else(|| format!("unknown elevation character '{ch}' at tile ({x},{y})"))?
                as u8;
            out.push(level);
        }
    }
    Ok(out)
}

fn parse_locations(
    width: u32,
    height: u32,
    authored: &[AuthoredLocation],
    field: &str,
) -> Result<Vec<(u32, u32)>, String> {
    let mut locations = Vec::with_capacity(authored.len());
    let mut seen = HashSet::with_capacity(authored.len());
    for (index, location) in authored.iter().enumerate() {
        if location.x >= width || location.y >= height {
            return Err(format!(
                "{field}[{index}] = ({},{}) is outside the {width}x{height} map",
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

fn parse_overlay_locations(
    width: u32,
    height: u32,
    authored: &[AuthoredLocation],
    field: &str,
) -> Result<Vec<(u32, u32)>, String> {
    let mut locations = parse_locations(width, height, authored, field)?;
    locations.sort_unstable();
    Ok(locations)
}

fn parse_base_sites(
    width: u32,
    height: u32,
    authored: &[AuthoredBaseSite],
) -> Result<Vec<AuthoredBaseSite>, String> {
    let mut sites = Vec::with_capacity(authored.len());
    let mut seen = HashSet::with_capacity(authored.len());
    for (index, site) in authored.iter().copied().enumerate() {
        if site.x >= width || site.y >= height {
            return Err(format!(
                "baseSites[{index}] = ({},{}) is outside the {width}x{height} map",
                site.x, site.y
            ));
        }
        if !seen.insert((site.x, site.y)) {
            return Err(format!(
                "baseSites[{index}] duplicates an earlier location at ({},{})",
                site.x, site.y
            ));
        }
        if site.steel_patches > MAX_STEEL_PATCHES_PER_BASE {
            return Err(format!(
                "baseSites[{index}].steelPatches must be between 0 and {MAX_STEEL_PATCHES_PER_BASE}"
            ));
        }
        if site.oil_patches > MAX_OIL_PATCHES_PER_BASE {
            return Err(format!(
                "baseSites[{index}].oilPatches must be between 0 and {MAX_OIL_PATCHES_PER_BASE}"
            ));
        }
        sites.push(site);
    }
    Ok(sites)
}

fn validate_base_clearance(
    width: u32,
    height: u32,
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
                if tx < 0 || ty < 0 || tx >= width as i32 || ty >= height as i32 {
                    return Err(format!(
                        "base site ({sx},{sy}) is too close to the map edge"
                    ));
                }
                let idx = (ty as u32 * width + tx as u32) as usize;
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
