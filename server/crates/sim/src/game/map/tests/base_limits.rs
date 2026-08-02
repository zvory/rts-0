use super::super::Map;

#[test]
fn three_player_map_is_selectable_and_loads_for_each_supported_player_count() {
    let available = Map::list_available();
    let three_player_map = available
        .iter()
        .find(|entry| entry.name == "3 Player Map")
        .expect("3-player map must be listed");
    assert_eq!(three_player_map.min_players, 1);
    assert_eq!(three_player_map.max_players, 3);

    let expected_three_player_starts = vec![(22, 43), (73, 136), (128, 45)];
    for player_count in 1..=3 {
        let mut map = Map::load("3 Player Map", player_count, 0x1234_5678)
            .expect("three-player map should load for every supported player count");
        assert_eq!(map.width, 150);
        assert_eq!(map.starts.len(), player_count);
        assert_eq!(map.base_sites.len(), 12);
        if player_count == 3 {
            map.starts.sort_unstable();
            assert_eq!(map.starts, expected_three_player_starts);
        }
    }
    assert!(
        Map::load("3 Player Map", 4, 0x1234_5678).is_err(),
        "three-player map should not expose a fourth start location"
    );
}

#[test]
fn authored_map_supports_many_unconditional_base_sites() {
    let rows = vec![".".repeat(80); 80];
    let base_sites: Vec<_> = (0..12)
        .map(|index| {
            format!(
                r#"{{"x": {}, "y": {}, "steelPatches": 12, "oilPatches": 3}}"#,
                8 + index * 5,
                24
            )
        })
        .collect();
    let json = format!(
        r#"{{
          "version": 5,
          "name": "many-bases",
          "width": 80,
          "height": 80,
          "description": "many permanent bases",
          "_design": "n/a",
          "terrain": {},
          "startLocations": [{{"x": 8, "y": 24}}],
          "baseSites": [{}]
        }}"#,
        serde_json::to_string(&rows).unwrap(),
        base_sites.join(",")
    );

    let map = Map::from_authored_json(1, &json, 0).expect("map should load");

    assert_eq!(map.starts.len(), 1);
    assert_eq!(map.base_sites.len(), 12);
}

#[test]
fn authored_map_rejects_more_than_bounded_base_sites() {
    let rows = vec![".".repeat(200); 200];
    let base_sites: Vec<_> = (0..33)
        .map(|index| {
            format!(
                r#"{{"x": {}, "y": 100, "steelPatches": 12, "oilPatches": 3}}"#,
                8 + index * 5
            )
        })
        .collect();
    let json = format!(
        r#"{{
          "version": 5,
          "name": "too-many-bases",
          "width": 200,
          "height": 200,
          "description": "too many bases",
          "_design": "n/a",
          "terrain": {},
          "startLocations": [{{"x": 8, "y": 100}}],
          "baseSites": [{}]
        }}"#,
        serde_json::to_string(&rows).unwrap(),
        base_sites.join(",")
    );

    let err = Map::from_authored_json(1, &json, 0).expect_err("bounded base count must fail");

    assert!(
        err.contains("baseSites must contain 1 to 32"),
        "error was: {err}"
    );
}

#[test]
fn authored_map_accepts_zero_and_maximum_per_base_resource_counts() {
    let rows = vec![".".repeat(40); 40];
    let json = format!(
        r#"{{
          "version": 5,
          "name": "resource-bounds",
          "width": 40,
          "height": 40,
          "description": "per-base resource bounds",
          "_design": "n/a",
          "terrain": {},
          "startLocations": [{{"x": 8, "y": 8}}],
          "baseSites": [
            {{"x": 8, "y": 8, "steelPatches": 0, "oilPatches": 0}},
            {{"x": 31, "y": 31, "steelPatches": 36, "oilPatches": 9}}
          ]
        }}"#,
        serde_json::to_string(&rows).unwrap(),
    );

    let map = Map::from_authored_json(1, &json, 0).expect("resource bounds should be accepted");
    assert_eq!(map.resource_counts_at((8, 8)).steel_patches, 0);
    assert_eq!(map.resource_counts_at((8, 8)).oil_patches, 0);
    assert_eq!(map.resource_counts_at((31, 31)).steel_patches, 36);
    assert_eq!(map.resource_counts_at((31, 31)).oil_patches, 9);
}

#[test]
fn authored_map_rejects_per_base_resource_counts_above_the_limits() {
    let rows = vec![".".repeat(32); 32];
    for (field, value, expected) in [
        ("steelPatches", 37, "steelPatches must be between 0 and 36"),
        ("oilPatches", 10, "oilPatches must be between 0 and 9"),
    ] {
        let mut site = serde_json::json!({
            "x": 8,
            "y": 8,
            "steelPatches": 12,
            "oilPatches": 3
        });
        site[field] = value.into();
        let json = serde_json::json!({
            "version": 5,
            "name": "bad-resource-count",
            "width": 32,
            "height": 32,
            "description": "invalid per-base resource count",
            "_design": "n/a",
            "terrain": rows.clone(),
            "startLocations": [{ "x": 8, "y": 8 }],
            "baseSites": [site]
        })
        .to_string();
        let err = Map::from_authored_json(1, &json, 0)
            .expect_err("resource count above the schema limit must fail");
        assert!(err.contains(expected), "error was: {err}");
    }
}
