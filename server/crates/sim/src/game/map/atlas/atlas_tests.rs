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
fn atlas_diagnostics_exposes_editor_layers() {
    let map = Map::load("Default", 4, 0x1234_5678).expect("default map should load");
    let diagnostics = map.atlas_diagnostics();
    let len = (map.size * map.size) as usize;
    let movement_classes = diagnostics["movementClasses"]
        .as_array()
        .expect("movement classes");
    let layers = diagnostics["layers"].as_array().expect("layers");
    let anchors = diagnostics["anchors"].as_array().expect("anchors");

    assert_eq!(diagnostics["size"], map.size);
    assert_eq!(movement_classes, &vec!["infantry", "vehicle"]);
    assert_eq!(layers.len(), MovementClass::ALL.len());
    assert!(!anchors.is_empty());

    for layer in layers {
        assert_eq!(
            layer["passableTiles"].as_array().expect("passable").len(),
            len
        );
        assert_eq!(
            layer["clearanceTiles"].as_array().expect("clearance").len(),
            len
        );
        assert_eq!(
            layer["componentByTile"]
                .as_array()
                .expect("components")
                .len(),
            len
        );
        assert_eq!(
            layer["regionByTile"].as_array().expect("regions").len(),
            len
        );
        assert!(!layer["components"]
            .as_array()
            .expect("component list")
            .is_empty());
        assert!(!layer["regions"].as_array().expect("region list").is_empty());
        assert!(anchors
            .iter()
            .any(|anchor| anchor["movementClass"] == layer["movementClass"]));
    }

    assert!(diagnostics.get("movementClasses").is_some());
    assert!(diagnostics["layers"][0].get("componentByTile").is_some());
    assert!(diagnostics["layers"][0].get("clearanceTiles").is_some());
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

#[test]
fn atlas_ignores_passable_islands_unreachable_from_start() {
    let mut map = all_rock_map(12, (1, 1));
    set_grass_rect(&mut map, 1, 1, 3, 3);
    set_grass_rect(&mut map, 8, 8, 9, 9);

    let atlas = map.atlas();
    let layer = atlas
        .layer(MovementClass::Infantry)
        .expect("infantry layer");

    assert_eq!(layer.components.len(), 1);
    assert_eq!(layer.components[0].tile_count, 9);

    let reachable_idx = map.index(2, 2);
    assert!(layer.passable_tiles[reachable_idx]);
    assert_eq!(layer.component_by_tile[reachable_idx], Some(0));
    assert!(layer.region_by_tile[reachable_idx].is_some());
    assert!(layer.clearance_tiles[reachable_idx] > 0);

    let island_idx = map.index(8, 8);
    assert!(!layer.passable_tiles[island_idx]);
    assert_eq!(layer.component_by_tile[island_idx], None);
    assert_eq!(layer.region_by_tile[island_idx], None);
    assert_eq!(layer.clearance_tiles[island_idx], 0);
}

#[test]
fn atlas_regions_use_twelve_tile_buckets() {
    let map = all_grass_map(24, (1, 1));
    let atlas = map.atlas();
    let layer = atlas
        .layer(MovementClass::Infantry)
        .expect("infantry layer");

    assert_eq!(layer.regions.len(), 4);
    for region in &layer.regions {
        assert!(region.max_tile.0 - region.min_tile.0 < REGION_SIZE_TILES);
        assert!(region.max_tile.1 - region.min_tile.1 < REGION_SIZE_TILES);
    }
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
    let mut map = all_grass_map(size, (1, 1));
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = map.index(x, y);
            map.terrain[idx] = map_terrain::ROCK;
        }
    }
    map
}

fn all_grass_map(size: u32, start: Tile) -> Map {
    Map {
        size,
        terrain: vec![map_terrain::GRASS; (size * size) as usize],
        starts: vec![start],
        expansion_sites: vec![(size - 2, size - 2)],
    }
}

fn all_rock_map(size: u32, start: Tile) -> Map {
    Map {
        size,
        terrain: vec![map_terrain::ROCK; (size * size) as usize],
        starts: vec![start],
        expansion_sites: vec![(size - 2, size - 2)],
    }
}

fn set_grass_rect(map: &mut Map, min_x: u32, min_y: u32, max_x: u32, max_y: u32) {
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let idx = map.index(x, y);
            map.terrain[idx] = map_terrain::GRASS;
        }
    }
}
