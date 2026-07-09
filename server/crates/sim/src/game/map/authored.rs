use std::collections::{HashMap, HashSet};

use serde::Deserialize;

mod assignment;

use super::{
    BaseSlot, Map, StartAssignmentPlayer, BASE_PROTECTION_RADIUS_TILES, CURRENT_MAP_VERSION,
    EXPANSION_PROTECTION_RADIUS_TILES,
};
use crate::protocol::terrain;

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

    let mut min_players = u32::MAX;
    let mut max_players = 0;
    for layout in &authored.layouts {
        if layout.player_count == 0 {
            continue;
        }
        min_players = min_players.min(layout.player_count);
        max_players = max_players.max(layout.player_count);
    }
    if max_players == 0 {
        return Err("layouts must contain at least one positive playerCount".to_string());
    }
    Ok((min_players, max_players))
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
    let sites = parse_sites(size, &authored.sites)?;

    if players.is_empty() {
        return Err("player_count must be at least 1".to_string());
    }
    let player_count = players.len();

    validate_base_clearance(size, &terrain, &sites)?;
    if authored.layouts.is_empty() {
        return Err("layouts must contain at least one spawn layout".to_string());
    }
    for layout in &authored.layouts {
        parse_layout_pairs(layout, &sites)?;
    }

    let matching_layouts: Vec<_> = authored
        .layouts
        .iter()
        .filter(|layout| layout.player_count as usize == player_count)
        .collect();
    if matching_layouts.is_empty() {
        return Err(format!(
            "map has no spawn layout for {player_count} players"
        ));
    }

    let slots = assignment::assign_layout_slots(&matching_layouts, &sites, players, seed)?;
    let starts: Vec<_> = slots.iter().map(|(start, _)| *start).collect();
    let expansion_sites = slots
        .iter()
        .flat_map(|(_, expansions)| expansions.iter().copied())
        .collect();

    Ok(Map {
        size,
        terrain,
        starts,
        expansion_sites,
    })
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
    sites: Vec<AuthoredSite>,
    layouts: Vec<AuthoredLayout>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredSite {
    id: String,
    kind: AuthoredSiteKind,
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum AuthoredSiteKind {
    Main,
    Natural,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredLayout {
    id: String,
    player_count: u32,
    slots: Vec<AuthoredLayoutSlot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthoredLayoutSlot {
    main: String,
    #[serde(default)]
    natural: Option<String>,
    #[serde(default)]
    naturals: Vec<String>,
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

fn parse_sites(
    size: u32,
    authored: &[AuthoredSite],
) -> Result<HashMap<String, AuthoredSite>, String> {
    if authored.is_empty() {
        return Err("sites must contain at least one site".to_string());
    }

    let mut out = HashMap::with_capacity(authored.len());
    let mut seen_coords = HashSet::with_capacity(authored.len());
    for (i, site) in authored.iter().enumerate() {
        if site.id.trim().is_empty() {
            return Err(format!("sites[{i}] has an empty id"));
        }
        if site.x >= size || site.y >= size {
            return Err(format!(
                "sites[{i}] = ({},{}) is outside the {size}x{size} map",
                site.x, site.y
            ));
        }
        if !seen_coords.insert((site.x, site.y)) {
            return Err(format!(
                "sites[{i}] duplicates an earlier site at ({},{})",
                site.x, site.y
            ));
        }
        if out.insert(site.id.clone(), site.clone()).is_some() {
            return Err(format!("sites[{i}] duplicates an earlier id {:?}", site.id));
        }
    }
    Ok(out)
}

fn parse_layout_pairs(
    layout: &AuthoredLayout,
    sites: &HashMap<String, AuthoredSite>,
) -> Result<Vec<BaseSlot>, String> {
    if layout.player_count == 0 {
        return Err(format!("layout {:?} has playerCount 0", layout.id));
    }
    if layout.slots.len() != layout.player_count as usize {
        return Err(format!(
            "layout {:?} has {} slots but playerCount is {}",
            layout.id,
            layout.slots.len(),
            layout.player_count
        ));
    }

    let mut seen_mains = HashSet::with_capacity(layout.slots.len());
    let mut seen_naturals = HashSet::with_capacity(layout.slots.len());
    let mut slots = Vec::with_capacity(layout.slots.len());
    for (i, slot) in layout.slots.iter().enumerate() {
        let main = sites.get(&slot.main).ok_or_else(|| {
            format!(
                "layout {:?} slot {i} references missing main {:?}",
                layout.id, slot.main
            )
        })?;
        if main.kind != AuthoredSiteKind::Main {
            return Err(format!(
                "layout {:?} slot {i} main {:?} is not a main site",
                layout.id, slot.main
            ));
        }

        if !seen_mains.insert(slot.main.as_str()) {
            return Err(format!(
                "layout {:?} assigns main {:?} more than once",
                layout.id, slot.main
            ));
        }

        let natural_ids = slot_natural_ids(slot).map_err(|err| {
            format!(
                "layout {:?} slot {i} has invalid naturals: {err}",
                layout.id
            )
        })?;
        let mut expansions = Vec::with_capacity(natural_ids.len());
        for natural_id in natural_ids {
            let natural = sites.get(natural_id).ok_or_else(|| {
                format!(
                    "layout {:?} slot {i} references missing natural {:?}",
                    layout.id, natural_id
                )
            })?;
            if natural.kind != AuthoredSiteKind::Natural {
                return Err(format!(
                    "layout {:?} slot {i} natural {:?} is not a natural site",
                    layout.id, natural_id
                ));
            }
            if !seen_naturals.insert(natural_id) {
                return Err(format!(
                    "layout {:?} assigns natural {:?} more than once",
                    layout.id, natural_id
                ));
            }
            expansions.push((natural.x, natural.y));
        }
        slots.push(((main.x, main.y), expansions));
    }
    Ok(slots)
}

fn slot_natural_ids(slot: &AuthoredLayoutSlot) -> Result<Vec<&str>, String> {
    let mut out = Vec::new();
    if let Some(natural) = slot.natural.as_deref() {
        if !natural.trim().is_empty() {
            out.push(natural);
        }
    }
    for natural in &slot.naturals {
        if !natural.trim().is_empty() {
            out.push(natural.as_str());
        }
    }
    if out.is_empty() {
        return Err("at least one natural is required".to_string());
    }

    let mut seen = HashSet::with_capacity(out.len());
    for natural in &out {
        if !seen.insert(*natural) {
            return Err(format!("natural {natural:?} is listed more than once"));
        }
    }
    Ok(out)
}

fn validate_base_clearance(
    size: u32,
    terrain_grid: &[u8],
    sites: &HashMap<String, AuthoredSite>,
) -> Result<(), String> {
    for site in sites.values() {
        let (sx, sy) = (site.x, site.y);
        let radius = if site.kind == AuthoredSiteKind::Main {
            BASE_PROTECTION_RADIUS_TILES
        } else {
            EXPANSION_PROTECTION_RADIUS_TILES
        };
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let tx = sx as i32 + dx;
                let ty = sy as i32 + dy;
                if tx < 0 || ty < 0 || tx >= size as i32 || ty >= size as i32 {
                    return Err(format!(
                        "site {:?} at ({sx},{sy}) is too close to the map edge",
                        site.id
                    ));
                }
                let idx = (ty as u32 * size + tx as u32) as usize;
                if terrain_grid[idx] != terrain::GRASS {
                    return Err(format!(
                        "site {:?} at ({sx},{sy}) has impassable terrain in its protected area at ({tx},{ty})",
                        site.id
                    ));
                }
            }
        }
    }
    Ok(())
}
