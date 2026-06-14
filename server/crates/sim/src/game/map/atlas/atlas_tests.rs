use super::*;
use crate::protocol::terrain as map_terrain;

#[test]
fn atlas_is_deterministic_for_same_map() {
    let map = Map::load("Default", 4, 0x1234_5678).expect("default map should load");

    assert_eq!(map.atlas(), map.atlas());
}

#[test]
fn atlas_generates_required_layers_for_every_bundled_map() {
    for available in Map::list_available() {
        for player_count in 1..=4 {
            let map = Map::load(&available.name, player_count, 0x1020_3040)
                .unwrap_or_else(|err| panic!("{} {player_count}p failed: {err}", available.name));
            let atlas = map.atlas();

            assert_eq!(atlas.movement_layers.len(), MovementClass::ALL.len());
            assert!(!atlas.anchors.is_empty(), "{}", available.name);

            for movement_class in MovementClass::ALL {
                let layer = atlas
                    .layer(movement_class)
                    .unwrap_or_else(|| panic!("missing {movement_class:?} layer"));
                assert_layer_consistent(&map, layer);
            }
        }
    }
}

#[test]
fn anchors_attach_to_passable_regions() {
    let map = Map::load("Low Econ", 4, 0x5566_7788).expect("low econ map should load");
    let atlas = map.atlas();

    for anchor in &atlas.anchors {
        assert!(
            anchor.component_id.is_some(),
            "anchor lacks component: {anchor:?}"
        );
        assert!(
            anchor.region_id.is_some(),
            "anchor lacks region: {anchor:?}"
        );
    }
}

#[test]
fn clearance_is_bounded_by_impassable_tiles_and_edges() {
    let map = map_with_rock_rect(12, 5, 5, 6, 6);
    let atlas = map.atlas();
    let layer = atlas
        .layer(MovementClass::Infantry)
        .expect("infantry layer");

    assert_eq!(layer.clearance_tiles[map.index(5, 5)], 0);
    assert_eq!(layer.clearance_tiles[map.index(4, 5)], 1);
    assert_eq!(layer.clearance_tiles[map.index(0, 0)], 1);
    assert!(layer.clearance_tiles[map.index(2, 2)] > 1);
}

fn assert_layer_consistent(map: &Map, layer: &MovementLayerAtlas) {
    let len = (map.size * map.size) as usize;
    assert_eq!(layer.passable_tiles.len(), len);
    assert_eq!(layer.clearance_tiles.len(), len);
    assert_eq!(layer.component_by_tile.len(), len);
    assert_eq!(layer.region_by_tile.len(), len);
    assert!(!layer.components.is_empty());
    assert!(!layer.regions.is_empty());

    for region in &layer.regions {
        assert_eq!(region.id, layer.regions[region.id].id);
        assert!(layer.components.get(region.component_id).is_some());
        assert!(region.tile_count > 0);
        assert!(region.min_tile.0 <= region.max_tile.0);
        assert!(region.min_tile.1 <= region.max_tile.1);
    }

    for portal in &layer.portals {
        let from = &layer.regions[portal.from_region];
        let to = &layer.regions[portal.to_region];
        assert_eq!(portal.id, layer.portals[portal.id].id);
        assert_eq!(portal.movement_class, layer.movement_class);
        assert_eq!(portal.component_id, from.component_id);
        assert_eq!(portal.component_id, to.component_id);
        assert_ne!(portal.from_region, portal.to_region);
        assert!(portal.width_tiles > 0);
        assert!(map.in_bounds(portal.center_tile.0 as i32, portal.center_tile.1 as i32));
    }

    for y in 0..map.size {
        for x in 0..map.size {
            let idx = map.index(x, y);
            if layer.passable_tiles[idx] {
                assert!(layer.component_by_tile[idx].is_some());
                assert!(layer.region_by_tile[idx].is_some());
                assert!(layer.clearance_tiles[idx] > 0);
            } else {
                assert_eq!(layer.component_by_tile[idx], None);
                assert_eq!(layer.region_by_tile[idx], None);
                assert_eq!(layer.clearance_tiles[idx], 0);
            }
        }
    }
}

fn map_with_rock_rect(size: u32, min_x: u32, min_y: u32, max_x: u32, max_y: u32) -> Map {
    let mut map = Map {
        size,
        terrain: vec![map_terrain::GRASS; (size * size) as usize],
        starts: vec![(1, 1)],
        expansion_sites: vec![(size - 2, size - 2)],
    };
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = map.index(x, y);
            map.terrain[idx] = map_terrain::ROCK;
        }
    }
    map
}
