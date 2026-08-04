use super::super::Map;

#[test]
fn waldubungen_is_selectable_with_standard_resources_at_every_base() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Waldübungen")
        .expect("Waldübungen should be listed");
    assert_eq!((entry.min_players, entry.max_players), (1, 2));

    let map = Map::load("Waldübungen", 2, 0x1234_5678)
        .expect("Waldübungen should load for two active players");
    assert_eq!((map.width, map.height), (192, 126));
    for &base in &map.base_sites {
        let resources = map.resource_counts_at(base);
        assert_eq!(resources.steel_patches, 12, "base {base:?}");
        assert_eq!(resources.oil_patches, 3, "base {base:?}");
    }

    assert!(
        Map::load("Waldübungen", 3, 0x1234_5678).is_err(),
        "Waldübungen should not expose a third start location"
    );
}
