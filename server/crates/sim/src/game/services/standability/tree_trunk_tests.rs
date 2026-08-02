use super::*;
use crate::protocol::MapDoodad;

fn flat_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![crate::protocol::terrain::GRASS; (size * size) as usize],
        starts: vec![],
        ..Default::default()
    }
}

#[test]
fn tree_trunks_block_exact_unit_bodies_while_flowers_remain_inert() {
    let mut tree_map = flat_map(12);
    let trunk = tree_map.tile_center(5, 5);
    tree_map.doodads.push(MapDoodad {
        id: 1,
        type_id: "tree.alder".to_string(),
        x: trunk.0 as u32,
        y: trunk.1 as u32,
        color: None,
    });
    let entities = EntityStore::new();
    let tree_occupancy = Occupancy::build(&tree_map, &entities);
    let rifleman_radius = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .radius;
    let same_tile_clear_x =
        trunk.0 + rifleman_radius + crate::game::map::doodads::TREE_TRUNK_RADIUS_PX + 0.1;

    assert!(!unit_static_standable(
        &tree_map,
        &tree_occupancy,
        EntityKind::Rifleman,
        trunk.0,
        trunk.1,
    ));
    assert!(unit_static_standable(
        &tree_map,
        &tree_occupancy,
        EntityKind::Rifleman,
        same_tile_clear_x,
        trunk.1,
    ));
    assert_eq!(
        tree_map.tile_of(same_tile_clear_x, trunk.1),
        tree_map.tile_of(trunk.0, trunk.1),
        "the same tree tile must retain standable sub-tile space",
    );
    assert!(!unit_static_segment_standable(
        &tree_map,
        &tree_occupancy,
        EntityKind::Rifleman,
        tree_map.tile_center(3, 5),
        tree_map.tile_center(7, 5),
    ));

    let mut flower_map = flat_map(12);
    flower_map.doodads.push(MapDoodad {
        id: 1,
        type_id: "wildflower.single".to_string(),
        x: trunk.0 as u32,
        y: trunk.1 as u32,
        color: Some("#e05a91".to_string()),
    });
    let flower_occupancy = Occupancy::build(&flower_map, &entities);
    assert!(unit_static_standable(
        &flower_map,
        &flower_occupancy,
        EntityKind::Rifleman,
        trunk.0,
        trunk.1,
    ));
}
