use std::collections::HashSet;

use super::super::Map;

#[test]
fn crossroads_is_selectable_and_retains_its_authored_sites() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Crossroads")
        .expect("Crossroads should be listed");
    assert_eq!(entry.min_players, 1);
    assert_eq!(entry.max_players, 2);

    let mut map = Map::load("Crossroads", 2, 0x1234_5678)
        .expect("Crossroads should load for two active players");
    assert_eq!((map.width, map.height), (126, 126));
    map.starts.sort_unstable();
    assert_eq!(map.starts, [(47, 8), (117, 78)]);
    assert_eq!(
        map.base_sites.iter().copied().collect::<HashSet<_>>(),
        HashSet::from([
            (47, 8),
            (117, 78),
            (9, 27),
            (98, 116),
            (9, 73),
            (52, 116),
            (67, 27),
            (98, 58),
            (13, 112),
            (66, 59),
        ])
    );
    assert!(
        Map::load("Crossroads", 3, 0x1234_5678).is_err(),
        "Crossroads should not expose a third start location"
    );
}
