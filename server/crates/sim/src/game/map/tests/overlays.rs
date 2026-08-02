use super::super::*;

fn authored_map_with_overlays(
    stealth_tiles: serde_json::Value,
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
        "stealthTiles": stealth_tiles,
        "noVehicleTiles": no_vehicle_tiles,
    })
    .to_string()
}

#[test]
fn authored_overlays_are_canonicalized_and_hash_as_distinct_layers() {
    let json = authored_map_with_overlays(
        serde_json::json!([{"x": 20, "y": 21}, {"x": 19, "y": 21}]),
        serde_json::json!([{"x": 22, "y": 21}]),
    );
    let map = Map::from_authored_json(1, &json, 0).expect("valid authored overlays");
    assert_eq!(map.stealth_tiles, vec![(19, 21), (20, 21)]);
    assert_eq!(map.no_vehicle_tiles, vec![(22, 21)]);

    let mut stealth_only = map.clone();
    stealth_only.no_vehicle_tiles.clear();
    stealth_only.stealth_tiles = vec![(22, 21)];
    let mut no_vehicle_only = stealth_only.clone();
    no_vehicle_only.stealth_tiles.clear();
    no_vehicle_only.no_vehicle_tiles = vec![(22, 21)];
    assert_ne!(
        stealth_only.materialized_hash(),
        no_vehicle_only.materialized_hash(),
        "the same coordinate in different gameplay layers must not collide",
    );
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
