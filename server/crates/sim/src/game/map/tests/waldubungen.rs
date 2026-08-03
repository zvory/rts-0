use super::super::Map;

#[test]
fn waldubungen_is_selectable_with_its_authored_start_resources() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Waldübungen")
        .expect("Waldübungen should be listed");
    assert_eq!((entry.min_players, entry.max_players), (1, 2));

    let map = Map::load("Waldübungen", 2, 0x1234_5678)
        .expect("Waldübungen should load for two active players");
    assert_eq!((map.width, map.height), (192, 126));

    let northwest_start = map.resource_counts_at((42, 9));
    assert_eq!((northwest_start.steel_patches, northwest_start.oil_patches), (4, 4));
    let southeast_start = map.resource_counts_at((149, 116));
    assert_eq!((southeast_start.steel_patches, southeast_start.oil_patches), (12, 3));

    assert!(
        Map::load("Waldübungen", 3, 0x1234_5678).is_err(),
        "Waldübungen should not expose a third start location"
    );
}
