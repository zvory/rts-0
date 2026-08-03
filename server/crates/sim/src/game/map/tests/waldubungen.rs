use super::super::Map;

#[test]
fn waldubungen_is_selectable_with_equal_starting_resources() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Waldübungen")
        .expect("Waldübungen should be listed");
    assert_eq!((entry.min_players, entry.max_players), (1, 2));

    let mut map = Map::load("Waldübungen", 2, 0x1234_5678)
        .expect("Waldübungen should load for two active players");
    assert_eq!((map.width, map.height), (192, 126));
    map.starts.sort_unstable();
    assert_eq!(map.starts, [(42, 9), (149, 116)]);
    for &start in &map.starts {
        let resources = map.resource_counts_at(start);
        assert_eq!(resources.steel_patches, 12, "start {start:?}");
        assert_eq!(resources.oil_patches, 3, "start {start:?}");
    }
    assert!(
        Map::load("Waldübungen", 3, 0x1234_5678).is_err(),
        "Waldübungen should not expose a third start location"
    );
}
