use std::collections::HashSet;

use super::super::*;
use crate::protocol::{MAP_DOODAD_TYPE_IDS, MAP_TILE_SIZE_PX};

#[test]
fn doodad_world_bound_tile_size_matches_simulation_tiles() {
    assert_eq!(MAP_TILE_SIZE_PX, config::TILE_SIZE);
}

#[test]
fn doodad_preview_preserves_one_v_one_layout_and_exercises_the_full_catalog() {
    let seed = 0x1234_5678;
    let original = Map::load("1v1", 2, seed).expect("1v1 should load");
    let preview = Map::load("1v1 Doodad Preview", 2, seed).expect("preview should load");
    assert_eq!(preview.terrain, original.terrain);
    assert_eq!(preview.starts, original.starts);
    assert_eq!(preview.base_sites, original.base_sites);
    assert_eq!(preview.base_resource_counts, original.base_resource_counts);
    assert_eq!(preview.doodads.len(), 156);

    let types = preview
        .doodads
        .iter()
        .map(|doodad| doodad.type_id.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(types, MAP_DOODAD_TYPE_IDS.into_iter().collect());
    for doodad in &preview.doodads {
        let tile = (doodad.x / config::TILE_SIZE, doodad.y / config::TILE_SIZE);
        assert_eq!(
            preview.terrain_at(tile.0, tile.1),
            terrain::GRASS,
            "preview doodad {} must remain off roads, rock, and water",
            doodad.id
        );
        assert!(
            preview.base_sites.iter().all(|&(x, y)| {
                let dx = i64::from(x) - i64::from(tile.0);
                let dy = i64::from(y) - i64::from(tile.1);
                dx * dx + dy * dy > 36
            }),
            "preview doodad {} is too close to a protected base",
            doodad.id
        );
    }
}

#[test]
fn authored_doodads_are_validated_canonicalized_and_hashed() {
    let rows = vec![".".repeat(32); 32];
    let document = serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "doodad-test",
        "description": "static decoration test",
        "_design": "test",
        "terrain": rows,
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [
            {"id": 7, "typeId": "wildflower.cluster", "x": 700, "y": 701, "color": "#e05a91"},
            {"id": 2, "typeId": "tree.alder", "x": 400, "y": 500}
        ]
    });
    let json = serde_json::to_string(&document).expect("map JSON");
    let materialized = Map::materialize_authored_json(&json, 1).expect("valid doodads");
    assert_eq!(
        materialized
            .doodads
            .iter()
            .map(|doodad| doodad.id)
            .collect::<Vec<_>>(),
        vec![2, 7]
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
        "description": "bad static decoration",
        "_design": "test",
        "terrain": rows,
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [{"id": 1, "typeId": "tree.oak", "x": 400, "y": 500, "rotation": 1}]
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
