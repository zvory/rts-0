use std::collections::HashSet;

use super::super::Map;

const EXPECTED_BASE_SITES: [(u32, u32); 12] = [
    (40, 40),
    (155, 155),
    (98, 20),
    (97, 175),
    (20, 98),
    (175, 97),
    (75, 76),
    (120, 119),
    (120, 76),
    (75, 119),
    (155, 40),
    (40, 155),
];

#[test]
fn open_basin_is_selectable_and_retains_its_authored_sites() {
    let available = Map::list_available();
    let entry = available
        .iter()
        .find(|entry| entry.name == "Open Basin")
        .expect("Open Basin should be listed");
    assert_eq!(entry.min_players, 1);
    assert_eq!(entry.max_players, 2);

    let mut map = Map::load("Open Basin", 2, 0x1234_5678)
        .expect("Open Basin should load for two active players");
    assert_eq!((map.width, map.height), (196, 196));
    map.starts.sort_unstable();
    assert_eq!(map.starts, [(40, 40), (155, 155)]);
    assert_eq!(
        map.base_sites.iter().copied().collect::<HashSet<_>>(),
        EXPECTED_BASE_SITES.into_iter().collect()
    );
    assert!(
        Map::load("Open Basin", 3, 0x1234_5678).is_err(),
        "Open Basin should not expose a third start location"
    );
}

#[test]
fn open_basin_is_rotationally_symmetric() {
    let map = Map::load("Open Basin", 2, 0x1234_5678)
        .expect("Open Basin should load for symmetry checks");

    for y in 0..map.height as usize {
        for x in 0..map.width as usize {
            let rotated_x = map.width as usize - 1 - x;
            let rotated_y = map.height as usize - 1 - y;
            assert_eq!(
                map.terrain[y * map.width as usize + x],
                map.terrain[rotated_y * map.width as usize + rotated_x],
                "Open Basin terrain differs at ({x},{y}) and its 180-degree rotation ({rotated_x},{rotated_y})"
            );
        }
    }

    let starts: HashSet<_> = map.starts.iter().copied().collect();
    let base_sites: HashSet<_> = map.base_sites.iter().copied().collect();
    for (locations, kind) in [(&starts, "start"), (&base_sites, "base site")] {
        for &(x, y) in locations {
            let rotated = (map.width - 1 - x, map.height - 1 - y);
            assert!(
                locations.contains(&rotated),
                "Open Basin {kind} ({x},{y}) has no 180-degree rotational counterpart"
            );
        }
    }
}
