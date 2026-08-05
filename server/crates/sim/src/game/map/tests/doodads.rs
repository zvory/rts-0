use super::super::*;
use crate::protocol::MAP_TILE_SIZE_PX;

#[test]
fn doodad_world_bound_tile_size_matches_simulation_tiles() {
    assert_eq!(MAP_TILE_SIZE_PX, config::TILE_SIZE);
}

#[test]
fn authored_doodads_are_validated_canonicalized_and_hashed() {
    let rows = vec![".".repeat(32); 32];
    let document = serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "doodad-test",
        "width": 32,
        "height": 32,
        "description": "static decoration test",
        "_design": "test",
        "terrain": rows,
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [
            {"id": 7, "typeId": "wildflower.cluster", "x": 700, "y": 701, "color": "#e05a91"},
            {"id": 2, "typeId": "tree.alder", "x": 400, "y": 500},
            {"id": 9, "typeId": "unit.tank_trap", "x": 80, "y": 80}
        ],
        "forestSpans": [],
        "noBuildingTiles": []
    });
    let json = serde_json::to_string(&document).expect("map JSON");
    let materialized = Map::materialize_authored_json(&json, 1).expect("valid doodads");
    assert_eq!(
        materialized
            .doodads
            .iter()
            .map(|doodad| doodad.id)
            .collect::<Vec<_>>(),
        vec![2, 7, 9]
    );

    let map = Map::from_authored_json(1, &json, 0).expect("valid authored map");
    assert_eq!(map.doodads, materialized.doodads);
    let mut changed = map.clone();
    changed.doodads[1].color = Some("#e05a92".to_string());
    assert_ne!(map.materialized_hash(), changed.materialized_hash());
}

#[test]
fn authored_doodads_reject_unknown_fields_and_invalid_catalog_data() {
    let rows = vec![".".repeat(32); 32];
    let base = serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "bad-doodad",
        "width": 32,
        "height": 32,
        "description": "bad static decoration",
        "_design": "test",
        "terrain": rows,
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [{"id": 1, "typeId": "tree.oak", "x": 400, "y": 500, "rotation": 1}],
        "forestSpans": [],
        "noBuildingTiles": []
    });
    let err = Map::materialize_authored_json(&base.to_string(), 1)
        .expect_err("unknown doodad field must fail");
    assert!(err.contains("unknown field"), "error was: {err}");

    for (doodad, expected) in [
        (
            serde_json::json!({"id": 0, "typeId": "tree.oak", "x": 400, "y": 500}),
            "nonzero",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "tree.maple", "x": 400, "y": 500}),
            "server catalog",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "tree.birch", "x": 400, "y": 500}),
            "server catalog",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "tree.aspen", "x": 400, "y": 500}),
            "server catalog",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "tree.oak", "x": 400, "y": 500, "color": "#ffffff"}),
            "only allowed for wildflowers",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "wildflower.single", "x": 400, "y": 500, "color": "#FF00aa"}),
            "canonical lowercase",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "tree.pine", "x": 1024, "y": 500}),
            "outside",
        ),
        (
            serde_json::json!({"id": 1, "typeId": "unit.tank_trap", "x": 81, "y": 80}),
            "centered on a map tile",
        ),
    ] {
        let mut document = base.clone();
        document["doodads"] = serde_json::json!([doodad]);
        let err = Map::materialize_authored_json(&document.to_string(), 1)
            .expect_err("invalid doodad must fail");
        assert!(
            err.contains(expected),
            "expected {expected:?}, error was: {err}"
        );
    }
}

#[test]
fn authored_doodads_respect_rectangular_world_bounds() {
    let document = serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "rectangular-doodad-test",
        "width": 32,
        "height": 16,
        "description": "rectangular static decoration test",
        "_design": "test",
        "terrain": vec![".".repeat(32); 16],
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [{"id": 1, "typeId": "tree.oak", "x": 100, "y": 512}],
        "forestSpans": [],
        "noBuildingTiles": []
    });

    let error = Map::materialize_authored_json(&document.to_string(), 1)
        .expect_err("doodad below the rectangular map must fail");

    assert!(error.contains("1024x512px map"), "error was: {error}");
}
