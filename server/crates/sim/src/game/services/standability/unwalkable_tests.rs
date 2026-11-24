use super::*;

#[test]
fn unwalkable_tiles_reject_bodies_spawns_and_building_footprints() {
    let mut map = Map {
        width: 24,
        height: 24,
        terrain: vec![crate::protocol::terrain::GRASS; 24 * 24],
        elevation: vec![0; 24 * 24],
        ..Default::default()
    };
    map.unwalkable_tiles = vec![(10, 10)];
    let entities = EntityStore::new();
    let occ = Occupancy::build(&map, &entities);
    let center = map.tile_center(10, 10);
    assert!(!unit_static_standable(
        &map,
        &occ,
        EntityKind::Rifleman,
        center.0,
        center.1
    ));
    assert!(!unit_spawn_standable(
        &map,
        &occ,
        &entities,
        EntityKind::Rifleman,
        center.0,
        center.1
    ));
    assert!(!building_site_clear(
        &map,
        &entities,
        EntityKind::Depot,
        9,
        9
    ));
    // A legal center is insufficient when the body overlaps a blocked tile edge.
    let edge_x = 10.0 * config::TILE_SIZE as f32 - 1.0;
    assert!(!unit_static_standable(
        &map,
        &occ,
        EntityKind::Rifleman,
        edge_x,
        center.1
    ));
    let clear = map.tile_center(8, 10);
    assert!(unit_spawn_standable(
        &map,
        &occ,
        &entities,
        EntityKind::Rifleman,
        clear.0,
        clear.1
    ));
}
