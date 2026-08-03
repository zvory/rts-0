//! Authoritative authored-map validation and static mobility reporting.
//!
//! These pure entry points are transport-independent. The developer CLI calls them directly, and
//! an HTTP/UI adapter can do the same without reimplementing map decoding or simulation pathing.

use serde::Serialize;

use super::{AuthoredMapData, Map};
use crate::config;
use crate::game::entity::EntityKind;
use crate::game::services::pathing::StaticRouteAnalyzer;

const AUTHORED_MAP_ANALYSIS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapTileCoordinate {
    pub x: u32,
    pub y: u32,
}

impl From<(u32, u32)> for MapTileCoordinate {
    fn from((x, y): (u32, u32)) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredBaseSummary {
    pub index: usize,
    pub tile: MapTileCoordinate,
    pub is_start_location: bool,
    pub steel_patches: u32,
    pub oil_patches: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredMapCheck {
    pub schema_version: u32,
    pub valid: bool,
    pub name: String,
    pub width_tiles: u32,
    pub height_tiles: u32,
    pub tile_size_px: u32,
    pub start_locations: Vec<MapTileCoordinate>,
    pub base_sites: Vec<AuthoredBaseSummary>,
    pub doodad_count: usize,
    pub stealth_tile_count: usize,
    pub no_vehicle_tile_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobilityProfileSummary {
    pub id: &'static str,
    pub representative_unit: &'static str,
    pub base_speed_px_per_tick: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredRouteReport {
    pub profile: &'static str,
    pub from_base_index: usize,
    pub to_base_index: usize,
    pub from_tile: MapTileCoordinate,
    pub to_tile: MapTileCoordinate,
    pub reachable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_px: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_travel_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthoredMapReport {
    pub schema_version: u32,
    pub valid: bool,
    pub map: AuthoredMapCheck,
    pub mobility_profiles: Vec<MobilityProfileSummary>,
    pub routes: Vec<AuthoredRouteReport>,
    pub unreachable_route_count: usize,
}

const MOBILITY_PROFILES: [(&str, EntityKind); 2] = [
    ("infantry", EntityKind::Rifleman),
    ("vehicle", EntityKind::ScoutCar),
];

/// Validate an authored-map document with the exact parser and materializer used by live games.
pub fn check_authored_json(json: &str) -> Result<AuthoredMapCheck, String> {
    let materialized = Map::materialize_authored_json(json, 1)?;
    Ok(check_from_materialized(&materialized))
}

/// Validate and report deterministic static routes between every unordered pair of base sites.
///
/// Routes use an empty dynamic entity layer: authored terrain, tree doodads, and no-vehicle tiles
/// participate, while match-time buildings and unit traffic intentionally do not. Travel time is
/// an estimate over the authoritative route and terrain speed multipliers; steering, acceleration,
/// turning, congestion, and match-time abilities are outside this static report.
pub fn analyze_authored_json(json: &str) -> Result<AuthoredMapReport, String> {
    let materialized = Map::materialize_authored_json(json, 1)?;
    let check = check_from_materialized(&materialized);
    let map = map_from_materialized(materialized);
    let mobility_profiles = MOBILITY_PROFILES
        .iter()
        .map(|&(id, kind)| MobilityProfileSummary {
            id,
            representative_unit: kind.stable_id(),
            base_speed_px_per_tick: config::unit_stats(kind)
                .map(|stats| stats.speed)
                .unwrap_or(0.0),
        })
        .collect();
    let mut analyzer = StaticRouteAnalyzer::new(&map);
    let mut routes = Vec::new();
    for from_base_index in 0..map.base_sites.len() {
        for to_base_index in (from_base_index + 1)..map.base_sites.len() {
            let from = map.base_sites[from_base_index];
            let to = map.base_sites[to_base_index];
            for &(profile, kind) in &MOBILITY_PROFILES {
                let result = analyzer.route(kind, from, to);
                routes.push(AuthoredRouteReport {
                    profile,
                    from_base_index,
                    to_base_index,
                    from_tile: from.into(),
                    to_tile: to.into(),
                    reachable: result.reachable,
                    distance_px: result.distance_px,
                    estimated_travel_seconds: result.estimated_travel_seconds,
                    failure_reason: result.failure_reason,
                });
            }
        }
    }
    let unreachable_route_count = routes.iter().filter(|route| !route.reachable).count();
    Ok(AuthoredMapReport {
        schema_version: AUTHORED_MAP_ANALYSIS_SCHEMA_VERSION,
        valid: true,
        map: check,
        mobility_profiles,
        routes,
        unreachable_route_count,
    })
}

fn check_from_materialized(map: &AuthoredMapData) -> AuthoredMapCheck {
    AuthoredMapCheck {
        schema_version: AUTHORED_MAP_ANALYSIS_SCHEMA_VERSION,
        valid: true,
        name: map.name.clone(),
        width_tiles: map.width,
        height_tiles: map.height,
        tile_size_px: config::TILE_SIZE,
        start_locations: map.starts.iter().copied().map(Into::into).collect(),
        base_sites: map
            .base_sites
            .iter()
            .copied()
            .enumerate()
            .map(|(index, tile)| {
                let counts = map
                    .base_resource_counts
                    .get(&tile)
                    .copied()
                    .unwrap_or_default();
                AuthoredBaseSummary {
                    index,
                    tile: tile.into(),
                    is_start_location: map.starts.contains(&tile),
                    steel_patches: counts.steel_patches,
                    oil_patches: counts.oil_patches,
                }
            })
            .collect(),
        doodad_count: map.doodads.len(),
        stealth_tile_count: map.stealth_tiles.len(),
        no_vehicle_tile_count: map.no_vehicle_tiles.len(),
    }
}

fn map_from_materialized(map: AuthoredMapData) -> Map {
    Map {
        width: map.width,
        height: map.height,
        terrain: map.terrain,
        starts: map.starts,
        base_sites: map.base_sites,
        base_resource_counts: map.base_resource_counts,
        doodads: map.doodads,
        stealth_tiles: map.stealth_tiles,
        no_vehicle_tiles: map.no_vehicle_tiles,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;

    fn authored_map(terrain: Vec<String>, doodads: Value, no_vehicle_tiles: Value) -> String {
        let height = terrain.len();
        let width = terrain.first().map(|row| row.len()).unwrap_or_default();
        json!({
            "version": super::super::CURRENT_MAP_VERSION,
            "name": "Analysis fixture",
            "width": width,
            "height": height,
            "description": "fixture",
            "_design": "fixture",
            "terrain": terrain,
            "startLocations": [{"x": 8, "y": 12}, {"x": 31, "y": 12}],
            "baseSites": [
                {"x": 8, "y": 12, "steelPatches": 4, "oilPatches": 1},
                {"x": 31, "y": 12, "steelPatches": 3, "oilPatches": 2}
            ],
            "doodads": doodads,
            "stealthTiles": [],
            "noVehicleTiles": no_vehicle_tiles
        })
        .to_string()
    }

    fn flat_rows(fill: char) -> Vec<String> {
        vec![fill.to_string().repeat(40); 25]
    }

    fn route<'a>(report: &'a AuthoredMapReport, profile: &str) -> &'a AuthoredRouteReport {
        report
            .routes
            .iter()
            .find(|route| route.profile == profile)
            .expect("profile route should exist")
    }

    #[test]
    fn check_uses_authoritative_materializer_and_summarizes_canonical_layers() {
        let json = authored_map(
            flat_rows('.'),
            json!([{"id": 2, "typeId": "tree.oak", "x": 640, "y": 400}]),
            json!([{"x": 20, "y": 12}]),
        );
        let check = check_authored_json(&json).expect("fixture should materialize");
        assert!(check.valid);
        assert_eq!(check.name, "Analysis fixture");
        assert_eq!(check.base_sites.len(), 2);
        assert_eq!(check.doodad_count, 1);
        assert_eq!(check.no_vehicle_tile_count, 1);
        assert_eq!(check.base_sites[1].oil_patches, 2);

        let mut invalid: Value = serde_json::from_str(&json).expect("fixture JSON");
        invalid["doodads"][0]["typeId"] = json!("tree.unknown");
        let error = check_authored_json(&invalid.to_string()).expect_err("unknown doodad fails");
        assert!(error.contains("is not in the server catalog"), "{error}");
    }

    #[test]
    fn report_makes_static_and_vehicle_only_route_failures_explicit() {
        let no_vehicle_wall: Vec<_> = (0..25).map(|y| json!({"x": 20, "y": y})).collect();
        let report = analyze_authored_json(&authored_map(
            flat_rows('.'),
            json!([]),
            Value::Array(no_vehicle_wall),
        ))
        .expect("vehicle exclusion fixture should analyze");
        assert!(route(&report, "infantry").reachable);
        assert!(!route(&report, "vehicle").reachable);
        assert_eq!(route(&report, "vehicle").failure_reason, Some("no_route"));
        assert_eq!(report.unreachable_route_count, 1);

        let mut blocked = flat_rows('.');
        for row in &mut blocked {
            row.replace_range(20..21, "#");
        }
        let blocked_report = analyze_authored_json(&authored_map(blocked, json!([]), json!([])))
            .expect("static wall fixture should analyze");
        assert!(blocked_report.routes.iter().all(|route| !route.reachable));
        assert_eq!(blocked_report.unreachable_route_count, 2);
    }

    #[test]
    fn report_accounts_for_tree_detours_and_road_speed() {
        let flat_json = authored_map(flat_rows('.'), json!([]), json!([]));
        let flat = analyze_authored_json(&flat_json).expect("flat fixture should analyze");
        let flat_infantry = route(&flat, "infantry");

        let tree_json = authored_map(
            flat_rows('.'),
            json!([{"id": 1, "typeId": "tree.oak", "x": 640, "y": 400}]),
            json!([]),
        );
        let tree = analyze_authored_json(&tree_json).expect("tree fixture should analyze");
        assert!(
            route(&tree, "infantry").distance_px > flat_infantry.distance_px,
            "tree trunk on the direct segment should force a longer static route"
        );

        let road = analyze_authored_json(&authored_map(flat_rows('='), json!([]), json!([])))
            .expect("road fixture should analyze");
        let road_infantry = route(&road, "infantry");
        assert_eq!(road_infantry.distance_px, flat_infantry.distance_px);
        assert!(
            road_infantry.estimated_travel_seconds < flat_infantry.estimated_travel_seconds,
            "authoritative road multiplier should reduce estimated travel time"
        );
    }
}
