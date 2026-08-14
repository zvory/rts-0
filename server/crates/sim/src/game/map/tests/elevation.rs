use super::super::{Map, CURRENT_MAP_VERSION};
use crate::protocol::MapSun;

#[test]
fn authored_relief_requires_valid_sun_and_round_trips_it() {
    let terrain = vec![".".repeat(32); 32];
    let mut elevation = vec!["0".repeat(32); 32];
    elevation[16].replace_range(16..17, "5");
    let mut authored = serde_json::json!({
        "version": CURRENT_MAP_VERSION,
        "name": "lit-relief",
        "width": 32,
        "height": 32,
        "description": "elevation contract",
        "_design": "n/a",
        "terrain": terrain,
        "elevation": elevation,
        "sun": { "azimuthDegrees": 315, "elevationDegrees": 12, "warmth": 75 },
        "startLocations": [{"x": 8, "y": 8}],
        "baseSites": [{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}],
        "doodads": [],
        "forestSpans": [],
        "noBuildingTiles": [],
        "noEntrenchmentTiles": []
    });

    let map = Map::from_authored_json(1, &authored.to_string(), 0)
        .expect("varying elevation with valid sun should materialize");
    assert_eq!(map.elevation[16 * 32 + 16], 5);
    assert_eq!(
        map.sun.expect("sun should survive materialization").warmth,
        75
    );

    authored.as_object_mut().unwrap().remove("sun");
    let error = Map::from_authored_json(1, &authored.to_string(), 0)
        .expect_err("varying elevation without sun must be rejected");
    assert!(error.contains("must specify sun"));

    authored["elevation"] = serde_json::json!(vec!["0".repeat(32); 32]);
    authored["sun"] = serde_json::json!({
        "azimuthDegrees": 315,
        "elevationDegrees": 12,
        "warmth": 75
    });
    let map = Map::from_authored_json(1, &authored.to_string(), 0)
        .expect("flat elevation may opt into authored sunlight");
    assert_eq!(
        map.sun
            .expect("flat authored sun should survive")
            .azimuth_degrees,
        315
    );
}

#[test]
fn materialized_hash_preserves_flat_legacy_identity_and_tracks_relief() {
    let legacy = Map {
        width: 2,
        height: 2,
        terrain: vec![0; 4],
        ..Default::default()
    };
    let mut normalized_flat = legacy.clone();
    normalized_flat.elevation = vec![0; 4];
    assert_eq!(
        legacy.materialized_hash(),
        normalized_flat.materialized_hash(),
        "normalizing an omitted legacy elevation layer must not invalidate its checkpoint hash"
    );

    let mut relief = normalized_flat;
    relief.elevation[3] = 1;
    relief.sun = Some(MapSun {
        azimuth_degrees: 315,
        elevation_degrees: 12,
        warmth: 75,
    });
    assert_ne!(
        legacy.materialized_hash(),
        relief.materialized_hash(),
        "authored relief and lighting must participate in map identity"
    );

    let mut uniform_high_ground = legacy.clone();
    uniform_high_ground.elevation = vec![8; 4];
    assert_ne!(
        legacy.materialized_hash(),
        uniform_high_ground.materialized_hash(),
        "uniform nonzero elevation affects sight and must participate in map identity"
    );
}
