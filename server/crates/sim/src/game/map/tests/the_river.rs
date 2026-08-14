use super::super::Map;
use crate::protocol::terrain;

#[test]
fn the_river_is_selectable_and_loads_for_a_two_player_match() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "The River")
        .expect("The River should be listed");
    assert_eq!(entry.min_players, 1);
    assert_eq!(entry.max_players, 2);

    let mut map = Map::load("The River", 2, 0x1234_5678)
        .expect("The River should load for two active players");
    assert_eq!((map.width, map.height), (126, 126));
    assert_eq!(map.terrain.len(), 126 * 126);
    assert_eq!(
        map.terrain.iter().filter(|&&tile| tile == terrain::WATER).count(),
        1_374
    );
    map.starts.sort_unstable();
    assert_eq!(map.starts, [(9, 9), (116, 116)]);
    assert_eq!(
        map.base_sites,
        [
            (9, 9),
            (116, 116),
            (18, 34),
            (107, 91),
            (63, 110),
            (62, 15),
        ]
    );
    assert_eq!(map.doodads.len(), 16);
    assert_eq!(map.concealment_tiles.len(), 38);
    assert_eq!(map.no_vehicle_tiles.len(), 38);
    assert_eq!(map.no_building_tiles.len(), 38);
    assert!(
        Map::load("The River", 3, 0x1234_5678).is_err(),
        "The River should not expose a third start location"
    );
}
