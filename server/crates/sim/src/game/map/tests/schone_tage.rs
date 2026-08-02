use std::collections::HashSet;

use super::super::Map;

#[test]
fn schone_tage_is_selectable_and_loads_for_two_players() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Schone Tage")
        .expect("Schone Tage should be listed");
    assert_eq!(entry.min_players, 1);
    assert_eq!(entry.max_players, 2);

    let schone_tage = Map::load("Schone Tage", 2, 0x1234_5678)
        .expect("Schone Tage should load for two active players");
    assert_eq!((schone_tage.width, schone_tage.height), (166, 166));
    assert_eq!(schone_tage.base_sites.len(), 8);
    let schone_tage_starts: HashSet<_> = schone_tage.starts.into_iter().collect();
    assert_eq!(schone_tage_starts, HashSet::from([(8, 47), (157, 47)]));
    assert!(
        Map::load("Schone Tage", 3, 0x1234_5678).is_err(),
        "Schone Tage should not expose a third start location"
    );
}
