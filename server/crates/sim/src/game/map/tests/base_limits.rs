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
        assert_eq!(map.size, 150);
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
        .map(|index| format!(r#"{{"x": {}, "y": {}}}"#, 8 + index * 5, 24))
        .collect();
    let json = format!(
        r#"{{
          "version": 3,
          "name": "many-bases",
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
        .map(|index| format!(r#"{{"x": {}, "y": 100}}"#, 8 + index * 5))
        .collect();
    let json = format!(
        r#"{{
          "version": 3,
          "name": "too-many-bases",
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
