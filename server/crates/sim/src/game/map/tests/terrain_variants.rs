use super::super::{terrain, Map};

#[test]
fn authored_map_accepts_visual_open_terrain_variants() {
    let mut rows = vec![".".repeat(32); 32];
    rows[8].replace_range(3..13, "0123456789");
    let json = format!(
        r#"{{
          "version": 5,
          "name": "open-variants",
          "width": 32,
          "height": 32,
          "description": "visual open terrain",
          "_design": "n/a",
          "terrain": {},
          "startLocations": [{{"x": 8, "y": 8}}],
          "baseSites": [{{"x": 8, "y": 8, "steelPatches": 12, "oilPatches": 3}}, {{"x": 24, "y": 24, "steelPatches": 12, "oilPatches": 3}}]
        }}"#,
        serde_json::to_string(&rows).unwrap()
    );

    let map = Map::from_authored_json(1, &json, 0).expect("visual variants should be passable");
    for (offset, code) in terrain::ALL[8..].iter().copied().enumerate() {
        assert_eq!(map.terrain_at(3 + offset as u32, 8), code);
        assert!(map.is_passable(3 + offset as i32, 8));
    }
}
