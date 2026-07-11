use super::entity::{EntityKind, EntityStore};
use super::map::Map;
use super::resource_placement::resource_blocked_building_tiles;
use crate::protocol::terrain;

fn flat_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![],
        base_sites: vec![],
    }
}

#[test]
fn resource_blocked_building_tiles_accounts_for_wide_footprints() {
    let map = flat_map(12);
    let mut entities = EntityStore::new();
    let (node_x, node_y) = map.tile_center(5, 5);
    entities
        .spawn_node(EntityKind::Steel, node_x, node_y)
        .expect("steel node should spawn");

    let blocked = resource_blocked_building_tiles(&map, &entities, EntityKind::Depot, None);

    assert!(
        blocked.contains(&(3, 5)),
        "3x3 depot footprint at (3, 5) intersects the steel node centered on (5, 5)"
    );
}
