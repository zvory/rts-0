use super::super::Map;

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

    assert!(err.contains("baseSites must contain 1 to 32"), "error was: {err}");
}
