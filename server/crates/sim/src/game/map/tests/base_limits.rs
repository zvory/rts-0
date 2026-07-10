use super::super::Map;

#[test]
fn authored_map_supports_four_bases_per_player() {
    let rows = vec![".".repeat(56); 56];
    let json = format!(
        r#"{{
          "version": 2,
          "name": "four-base",
          "description": "four-base map",
          "_design": "n/a",
          "terrain": {},
          "sites": [
            {{"id": "main_a", "kind": "main", "x": 8, "y": 8}},
            {{"id": "nat_a", "kind": "natural", "x": 24, "y": 8}},
            {{"id": "nat_b", "kind": "natural", "x": 24, "y": 24}},
            {{"id": "nat_c", "kind": "natural", "x": 40, "y": 24}}
          ],
          "layouts": [
            {{"id": "one", "playerCount": 1, "slots": [{{"main": "main_a", "naturals": ["nat_a", "nat_b", "nat_c"]}}]}}
          ]
        }}"#,
        serde_json::to_string(&rows).unwrap()
    );

    let map = Map::from_authored_json(1, &json, 0).expect("four-base map should load");

    assert_eq!(map.starts, vec![(8, 8)]);
    assert_eq!(map.expansion_sites, vec![(24, 8), (24, 24), (40, 24)]);
}

#[test]
fn authored_map_rejects_more_than_four_bases_per_player() {
    let rows = vec![".".repeat(64); 64];
    let json = format!(
        r#"{{
          "version": 2,
          "name": "five-base",
          "description": "five-base map",
          "_design": "n/a",
          "terrain": {},
          "sites": [
            {{"id": "main_a", "kind": "main", "x": 8, "y": 8}},
            {{"id": "nat_a", "kind": "natural", "x": 24, "y": 8}},
            {{"id": "nat_b", "kind": "natural", "x": 24, "y": 24}},
            {{"id": "nat_c", "kind": "natural", "x": 40, "y": 24}},
            {{"id": "nat_d", "kind": "natural", "x": 40, "y": 40}}
          ],
          "layouts": [
            {{"id": "one", "playerCount": 1, "slots": [{{"main": "main_a", "naturals": ["nat_a", "nat_b", "nat_c", "nat_d"]}}]}}
          ]
        }}"#,
        serde_json::to_string(&rows).unwrap()
    );

    let err = Map::from_authored_json(1, &json, 0).expect_err("five-base map should be rejected");

    assert!(err.contains("at most 3 naturals"), "error was: {err}");
}
