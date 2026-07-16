use std::collections::HashSet;

use super::super::Map;

const EXPECTED_BASE_SITES: [(u32, u32); 16] = [
    (18, 18),
    (147, 18),
    (147, 147),
    (18, 147),
    (18, 40),
    (125, 18),
    (147, 125),
    (40, 147),
    (56, 18),
    (147, 56),
    (109, 147),
    (18, 109),
    (53, 55),
    (110, 53),
    (112, 110),
    (55, 112),
];
const EXPECTED_STARTS: [(u32, u32); 4] = [(18, 18), (18, 147), (147, 18), (147, 147)];

#[test]
fn four_player_map_retains_every_resource_site() {
    let expected_base_sites: HashSet<_> = EXPECTED_BASE_SITES.into_iter().collect();

    for player_count in 1..=4 {
        let mut map = Map::load("4 Player Map", player_count, 0x1234_5678)
            .expect("four-player map should load for every supported player count");
        assert_eq!(
            map.base_sites.iter().copied().collect::<HashSet<_>>(),
            expected_base_sites,
            "four-player map must retain all permanent resource sites for player_count={player_count}"
        );
        if player_count == 4 {
            map.starts.sort_unstable();
            assert_eq!(map.starts, EXPECTED_STARTS);
        }
    }
}

#[test]
fn four_player_map_is_fourfold_rotationally_symmetric() {
    let map = Map::load("4 Player Map", 4, 0x1234_5678).expect("four-player map should load");
    let size = map.size as usize;

    for y in 0..size {
        for x in 0..size {
            let rotated_x = size - 1 - y;
            let rotated_y = x;
            assert_eq!(
                map.terrain[y * size + x],
                map.terrain[rotated_y * size + rotated_x],
                "four-player terrain differs at ({x},{y}) and its 90-degree rotation ({rotated_x},{rotated_y})"
            );
        }
    }

    let starts: HashSet<_> = map.starts.iter().copied().collect();
    let base_sites: HashSet<_> = map.base_sites.iter().copied().collect();
    for (locations, kind) in [(&starts, "start"), (&base_sites, "base site")] {
        for &location in locations {
            let rotated = (map.size - 1 - location.1, location.0);
            assert!(
                locations.contains(&rotated),
                "four-player {kind} ({},{}) has no 90-degree rotational counterpart",
                location.0,
                location.1,
            );
        }
    }
}
