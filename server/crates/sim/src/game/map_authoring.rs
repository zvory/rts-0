//! Authoritative authored-map validation and static mobility reporting.
//!
//! These pure entry points are transport-independent. The developer CLI calls them directly, and
//! an HTTP/UI adapter can do the same without reimplementing map decoding or simulation pathing.

use serde::Serialize;

use crate::config;
use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::{AuthoredMapData, Map};
use crate::game::services::pathing::StaticRouteAnalyzer;
use crate::game::setup::spawn_authored_map_entities;

const AUTHORED_MAP_ANALYSIS_SCHEMA_VERSION: u32 = 2;
const MAX_ANALYZED_ROUTES: usize = 256;
const REPORT_SEARCH_EXPANSION_BUDGET: usize = 262_144;

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
    pub concealment_tile_count: usize,
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
    pub analyzed: bool,
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
    pub analyzed_route_count: usize,
    pub unreachable_route_count: usize,
    pub unanalyzed_route_count: usize,
    pub truncated: bool,
}

const MOBILITY_PROFILES: [(&str, EntityKind); 2] = [
    ("infantry", EntityKind::Rifleman),
    ("vehicle", EntityKind::ScoutCar),
];

/// Process an authored map into the stable JSON-facing check or static report document.
pub fn process_authored_json(
    json: &str,
    include_route_report: bool,
) -> Result<serde_json::Value, String> {
    if include_route_report {
        analyze_authored_json(json)
            .and_then(|report| serde_json::to_value(report).map_err(|error| error.to_string()))
    } else {
        check_authored_json(json)
            .and_then(|check| serde_json::to_value(check).map_err(|error| error.to_string()))
    }
}

/// Validate an authored-map document with the exact parser and materializer used by live games.
pub(super) fn check_authored_json(json: &str) -> Result<AuthoredMapCheck, String> {
    let materialized = Map::materialize_authored_json(json, 1)?;
    Ok(check_from_materialized(&materialized))
}

/// Validate and report deterministic static routes between every unordered pair of base sites.
///
/// Routes use the authored initial static layer: terrain, trees, tank traps, and no-vehicle tiles
/// participate, while player starts, resources, later buildings, and unit traffic intentionally do
/// not. Travel time is an estimate over the authoritative route and terrain speed multipliers;
/// steering, acceleration, turning, congestion, and match-time abilities are outside this report.
pub(super) fn analyze_authored_json(json: &str) -> Result<AuthoredMapReport, String> {
    let materialized = Map::materialize_authored_json(json, 1)?;
    let check = check_from_materialized(&materialized);
    let map = map_from_materialized(materialized);
    let mut static_entities = EntityStore::new();
    let _ = spawn_authored_map_entities(&map, &mut static_entities);
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
    let mut analyzer =
        StaticRouteAnalyzer::new(&map, &static_entities, REPORT_SEARCH_EXPANSION_BUDGET);
    let mut routes = Vec::new();
    let mut route_attempts = 0usize;
    for from_base_index in 0..map.base_sites.len() {
        for to_base_index in (from_base_index + 1)..map.base_sites.len() {
            let from = map.base_sites[from_base_index];
            let to = map.base_sites[to_base_index];
            for &(profile, kind) in &MOBILITY_PROFILES {
                let result = (route_attempts < MAX_ANALYZED_ROUTES).then(|| {
                    route_attempts += 1;
                    analyzer.route(kind, from, to)
                });
                routes.push(AuthoredRouteReport {
                    profile,
                    from_base_index,
                    to_base_index,
                    from_tile: from.into(),
                    to_tile: to.into(),
                    analyzed: result.is_some_and(|result| result.analyzed),
                    reachable: result.is_some_and(|result| result.reachable),
                    distance_px: result.and_then(|result| result.distance_px),
                    estimated_travel_seconds: result
                        .and_then(|result| result.estimated_travel_seconds),
                    failure_reason: result
                        .and_then(|result| result.failure_reason)
                        .or((result.is_none()).then_some("route_limit_reached")),
                });
            }
        }
    }
    let analyzed_route_count = routes.iter().filter(|route| route.analyzed).count();
    let unreachable_route_count = routes
        .iter()
        .filter(|route| route.analyzed && !route.reachable)
        .count();
    let unanalyzed_route_count = routes.len().saturating_sub(analyzed_route_count);
    Ok(AuthoredMapReport {
        schema_version: AUTHORED_MAP_ANALYSIS_SCHEMA_VERSION,
        valid: true,
        map: check,
        mobility_profiles,
        routes,
        analyzed_route_count,
        unreachable_route_count,
        unanalyzed_route_count,
        truncated: unanalyzed_route_count > 0,
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
        concealment_tile_count: map.concealment_tiles.len(),
        no_vehicle_tile_count: map.no_vehicle_tiles.len(),
    }
}

fn map_from_materialized(map: AuthoredMapData) -> Map {
    Map {
        width: map.width,
        height: map.height,
        terrain: map.terrain,
        elevation: map.elevation,
        sun: map.sun,
        starts: map.starts,
        base_sites: map.base_sites,
        base_resource_counts: map.base_resource_counts,
        doodads: map.doodads,
        concealment_tiles: map.concealment_tiles,
        no_vehicle_tiles: map.no_vehicle_tiles,
        no_building_tiles: map.no_building_tiles,
        no_entrenchment_tiles: map.no_entrenchment_tiles,
        damage_reduction_tiles: map.damage_reduction_tiles,
        slow_movement_tiles: map.slow_movement_tiles,
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
            "version": crate::game::map::CURRENT_MAP_VERSION,
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
            "forestSpans": [],
            "concealmentTiles": [],
            "noVehicleTiles": no_vehicle_tiles,
            "noBuildingTiles": [],
            "noEntrenchmentTiles": []
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
        assert!(route(&report, "infantry").analyzed);
        assert!(!route(&report, "vehicle").reachable);
        assert!(route(&report, "vehicle").analyzed);
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
    fn authored_tank_trap_entities_block_vehicle_routes_but_not_infantry() {
        let tank_trap_wall: Vec<_> = (0..25)
            .map(|y| {
                json!({
                    "id": y + 1,
                    "typeId": "unit.tank_trap",
                    "x": 20 * config::TILE_SIZE + config::TILE_SIZE / 2,
                    "y": y * config::TILE_SIZE + config::TILE_SIZE / 2
                })
            })
            .collect();
        let report = analyze_authored_json(&authored_map(
            flat_rows('.'),
            Value::Array(tank_trap_wall),
            json!([]),
        ))
        .expect("tank-trap fixture should analyze");

        assert!(route(&report, "infantry").reachable);
        assert!(!route(&report, "vehicle").reachable);
        assert_eq!(route(&report, "vehicle").failure_reason, Some("no_route"));
    }

    #[test]
    fn maximum_base_report_is_deterministically_truncated() {
        let terrain = vec![".".repeat(256); 256];
        let base_sites: Vec<_> = (0..32)
            .map(|index| {
                let x = 8 + (index % 8) * 32;
                let y = 8 + (index / 8) * 64;
                json!({"x": x, "y": y, "steelPatches": 4, "oilPatches": 1})
            })
            .collect();
        let no_vehicle_wall: Vec<_> = (0..256).map(|y| json!({"x": 128, "y": y})).collect();
        let json = json!({
            "version": crate::game::map::CURRENT_MAP_VERSION,
            "name": "Maximum report fixture",
            "width": 256,
            "height": 256,
            "description": "fixture",
            "_design": "fixture",
            "terrain": terrain,
            "startLocations": [{"x": 8, "y": 8}],
            "baseSites": base_sites,
            "doodads": [],
            "forestSpans": [],
            "concealmentTiles": [],
            "noVehicleTiles": no_vehicle_wall,
            "noBuildingTiles": [],
            "noEntrenchmentTiles": []
        })
        .to_string();

        let report = analyze_authored_json(&json).expect("maximum fixture should analyze");
        assert_eq!(report.routes.len(), 32 * 31);
        assert!(report.analyzed_route_count <= MAX_ANALYZED_ROUTES);
        assert_eq!(
            report.analyzed_route_count + report.unanalyzed_route_count,
            report.routes.len()
        );
        assert!(report.truncated);
        assert!(report.routes.iter().any(|route| {
            !route.analyzed && route.failure_reason == Some("analysis_budget_exhausted")
        }));
        assert!(report
            .routes
            .iter()
            .skip(MAX_ANALYZED_ROUTES)
            .all(|route| !route.analyzed && route.failure_reason == Some("route_limit_reached")));
    }

    #[test]
    fn report_keeps_infantry_tree_avoidance_local_and_accounts_for_road_speed() {
        let flat_json = authored_map(flat_rows('.'), json!([]), json!([]));
        let flat = analyze_authored_json(&flat_json).expect("flat fixture should analyze");
        let flat_infantry = route(&flat, "infantry");

        let tree_json = authored_map(
            flat_rows('.'),
            json!([{"id": 1, "typeId": "tree.oak", "x": 640, "y": 400}]),
            json!([]),
        );
        let tree = analyze_authored_json(&tree_json).expect("tree fixture should analyze");
        assert_eq!(
            route(&tree, "infantry").distance_px,
            flat_infantry.distance_px,
            "infantry tree avoidance is local steering and must not block the static route"
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

    #[test]
    fn report_travel_time_accounts_for_directional_elevation_speed() {
        let flat_json = authored_map(flat_rows('.'), json!([]), json!([]));
        let flat = analyze_authored_json(&flat_json).expect("flat fixture should analyze");
        let flat_seconds = route(&flat, "infantry").estimated_travel_seconds;

        let mut uphill: Value = serde_json::from_str(&flat_json).expect("fixture JSON");
        uphill["elevation"] = json!(vec![format!("{}{}", "0".repeat(20), "1".repeat(20)); 25]);
        uphill["sun"] = json!({"azimuthDegrees": 315, "elevationDegrees": 30, "warmth": 50});
        let uphill = analyze_authored_json(&uphill.to_string()).expect("uphill fixture");
        assert!(
            route(&uphill, "infantry").estimated_travel_seconds > flat_seconds,
            "uphill movement should increase estimated travel time"
        );

        let mut downhill: Value = serde_json::from_str(&flat_json).expect("fixture JSON");
        downhill["elevation"] = json!(vec![format!("{}{}", "1".repeat(20), "0".repeat(20)); 25]);
        downhill["sun"] = json!({"azimuthDegrees": 315, "elevationDegrees": 30, "warmth": 50});
        let downhill = analyze_authored_json(&downhill.to_string()).expect("downhill fixture");
        assert!(
            route(&downhill, "infantry").estimated_travel_seconds < flat_seconds,
            "downhill movement should reduce estimated travel time"
        );
    }
}
