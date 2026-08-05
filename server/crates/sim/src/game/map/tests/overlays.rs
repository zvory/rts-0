use super::super::*;

fn authored_map_with_overlays(
    concealment_tiles: serde_json::Value,
    no_vehicle_tiles: serde_json::Value,
) -> String {
    serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "overlay-test",
        "width": 32,
        "height": 32,
        "description": "sparse gameplay overlay test",
        "_design": "test",
        "terrain": vec![".".repeat(32); 32],
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [],
        "forestSpans": [],
        "concealmentTiles": concealment_tiles,
        "noVehicleTiles": no_vehicle_tiles,
        "noBuildingTiles": [],
        "damageReductionTiles": [],
        "slowMovementTiles": [],
    })
    .to_string()
}

#[test]
fn reduction_and_slow_layers_are_independent_and_may_overlap_every_other_overlay() {
    let mut authored: serde_json::Value = serde_json::from_str(&authored_map_with_overlays(
        serde_json::json!([{"x": 18, "y": 18}]),
        serde_json::json!([{"x": 18, "y": 18}]),
    ))
    .expect("test map JSON");
    authored["damageReductionTiles"] = serde_json::json!([
        {"x": 18, "y": 18}, {"x": 19, "y": 18}
    ]);
    authored["slowMovementTiles"] = serde_json::json!([
        {"x": 18, "y": 18}, {"x": 20, "y": 18}
    ]);
    let map = Map::from_authored_json(1, &authored.to_string(), 0)
        .expect("five independent authored overlays");

    assert_eq!(map.damage_reduction_tiles, vec![(18, 18), (19, 18)]);
    assert_eq!(map.slow_movement_tiles, vec![(18, 18), (20, 18)]);
    let center = map.tile_center(18, 18);
    assert_eq!(map.damage_after_reduction_tile(center.0, center.1, 100), 75);
    assert_eq!(map.slow_movement_multiplier_at(center.0, center.1), 0.75);
    assert_ne!(map.materialized_hash(), {
        let mut without_reduction = map.clone();
        without_reduction.damage_reduction_tiles.clear();
        without_reduction.materialized_hash()
    });
}

#[test]
fn authored_overlays_are_canonicalized_and_hash_as_distinct_layers() {
    let json = authored_map_with_overlays(
        serde_json::json!([{"x": 20, "y": 21}, {"x": 19, "y": 21}]),
        serde_json::json!([{"x": 22, "y": 21}]),
    );
    let map = Map::from_authored_json(1, &json, 0).expect("valid authored overlays");
    assert_eq!(map.concealment_tiles, vec![(19, 21), (20, 21)]);
    assert_eq!(map.no_vehicle_tiles, vec![(22, 21)]);

    let mut concealment_only = map.clone();
    concealment_only.no_vehicle_tiles.clear();
    concealment_only.concealment_tiles = vec![(22, 21)];
    let mut no_vehicle_only = concealment_only.clone();
    no_vehicle_only.concealment_tiles.clear();
    no_vehicle_only.no_vehicle_tiles = vec![(22, 21)];
    assert_ne!(
        concealment_only.materialized_hash(),
        no_vehicle_only.materialized_hash(),
        "the same coordinate in different gameplay layers must not collide",
    );
}

#[test]
fn overlay_canonicalization_does_not_reorder_authored_starts() {
    let mut authored: serde_json::Value = serde_json::from_str(&authored_map_with_overlays(
        serde_json::json!([]),
        serde_json::json!([]),
    ))
    .expect("test map JSON");
    authored["startLocations"] = serde_json::json!([
        {"x": 24, "y": 8},
        {"x": 8, "y": 8}
    ]);
    authored["baseSites"] = serde_json::json!([
        {"x": 24, "y": 8, "steelPatches": 12, "oilPatches": 3},
        {"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}
    ]);

    let materialized =
        Map::materialize_authored_json(&authored.to_string(), 2).expect("valid authored starts");
    assert_eq!(materialized.starts, vec![(24, 8), (8, 8)]);
}

#[test]
fn authored_overlays_reject_duplicates_and_out_of_bounds_tiles() {
    for (tiles, expected) in [
        (
            serde_json::json!([{"x": 20, "y": 21}, {"x": 20, "y": 21}]),
            "duplicates",
        ),
        (serde_json::json!([{"x": 32, "y": 21}]), "outside"),
    ] {
        let json = authored_map_with_overlays(tiles, serde_json::json!([]));
        let error = Map::materialize_authored_json(&json, 1)
            .expect_err("invalid overlay coordinates must be rejected");
        assert!(error.contains(expected), "error was: {error}");
    }
}

#[test]
fn compact_forest_spans_materialize_into_all_gameplay_layers() {
    let mut authored: serde_json::Value = serde_json::from_str(&authored_map_with_overlays(
        serde_json::json!([{"x": 20, "y": 21}]),
        serde_json::json!([]),
    ))
    .expect("test map JSON");
    authored["forestSpans"] = serde_json::json!([[22, 18, 20], [23, 19, 19]]);
    let materialized = Map::materialize_authored_json(&authored.to_string(), 1)
        .expect("valid compact forest spans");

    let forest = vec![(18, 22), (19, 22), (19, 23), (20, 22)];
    assert_eq!(materialized.no_vehicle_tiles, forest);
    assert_eq!(materialized.no_building_tiles, forest);
    assert_eq!(materialized.damage_reduction_tiles, forest);
    assert_eq!(materialized.slow_movement_tiles, forest);
    assert_eq!(
        materialized.concealment_tiles,
        vec![(18, 22), (19, 22), (19, 23), (20, 21), (20, 22)]
    );
}

#[test]
fn shipped_forest_maps_exclude_buildings_on_their_full_semantic_forest_mask() {
    for name in ["Crossroads"] {
        let map = Map::load(name, 1, 0).expect("bundled forest map should load");
        assert!(!map.no_building_tiles.is_empty(), "{name}");
        assert_eq!(map.no_building_tiles, map.no_vehicle_tiles, "{name}");
    }
}

#[test]
fn compact_forest_spans_reject_overlap_and_bad_bounds() {
    for (spans, expected) in [
        (serde_json::json!([[22, 18, 20], [22, 20, 21]]), "overlaps"),
        (serde_json::json!([[22, 20, 18]]), "reversed"),
        (serde_json::json!([[32, 18, 20]]), "outside"),
    ] {
        let mut authored: serde_json::Value = serde_json::from_str(&authored_map_with_overlays(
            serde_json::json!([]),
            serde_json::json!([]),
        ))
        .expect("test map JSON");
        authored["forestSpans"] = spans;
        let error = Map::materialize_authored_json(&authored.to_string(), 1)
            .expect_err("invalid compact forest spans must be rejected");
        assert!(error.contains(expected), "error was: {error}");
    }
}

#[test]
fn current_authored_schema_requires_forest_spans() {
    let mut authored: serde_json::Value = serde_json::from_str(&authored_map_with_overlays(
        serde_json::json!([]),
        serde_json::json!([]),
    ))
    .expect("test map JSON");
    authored
        .as_object_mut()
        .expect("authored map object")
        .remove("forestSpans");

    let error = Map::materialize_authored_json(&authored.to_string(), 1)
        .expect_err("schema-v9 maps must declare forestSpans");
    assert!(
        error.contains("forestSpans must be an array"),
        "error was: {error}"
    );
}
