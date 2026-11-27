use super::*;
#[test]
fn automatic_extractors_share_quantized_node_tiles_sequentially_not_simultaneously() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let depot = entities
        .spawn_building(1, EntityKind::ResourceDepot, 1712.0, 464.0, true)
        .unwrap();
    // Actual Classic seed7 natural nodes59/60: distinct32px-separated patch centers
    // from a rotated resource grid, both reserving the authoritative tile(49,16).
    let first = (1575.8413, 513.52563);
    let second = (1590.0337, 542.2063);
    assert_eq!(
        map.tile_of(first.0, first.1),
        map.tile_of(second.0, second.1)
    );
    let first_node = entities
        .spawn_node(EntityKind::Steel, first.0, first.1)
        .unwrap();
    entities
        .spawn_node(EntityKind::Steel, second.0, second.1)
        .unwrap();
    let old = entities
        .spawn_building(1, EntityKind::SteelMine, first.0, first.1, true)
        .unwrap();
    let alternative = (1712.0, 640.0);
    entities
        .spawn_node(EntityKind::Steel, alternative.0, alternative.1)
        .unwrap();
    assert_eq!(
        extractors::target(&map, &entities, depot, EntityKind::SteelMine),
        Some(alternative)
    );
    extractors::advance_automatic(&map, &mut entities, depot, "kriegsia", 1);
    let scaffold = extractors::scaffold_id(&entities, depot, EntityKind::SteelMine).unwrap();
    assert_eq!(
        (
            entities.get(scaffold).unwrap().pos_x(),
            entities.get(scaffold).unwrap().pos_y()
        ),
        alternative
    );
    // No two existing buildings reserve the same tile, including incomplete scaffolds.
    let mut occupied = std::collections::BTreeSet::new();
    for building in entities.iter().filter(|e| e.is_building()) {
        for tile in crate::game::services::occupancy::building_footprint(&map, building) {
            assert!(occupied.insert(tile));
        }
    }
    entities.remove(scaffold);
    entities
        .get_mut(first_node)
        .unwrap()
        .harvest_resources(u32::MAX);
    entities.remove(old);
    assert_eq!(
        extractors::target(&map, &entities, depot, EntityKind::SteelMine),
        Some(second)
    );
}

#[test]
fn blocked_extractor_front_does_not_pay_and_retries_when_footprint_is_freed() {
    let map = flat_map(32);
    let mut entities = EntityStore::new();
    let depot = entities
        .spawn_building(1, EntityKind::ResourceDepot, 128.0, 128.0, true)
        .unwrap();
    let target = (272.0, 144.0);
    entities
        .spawn_node(EntityKind::Oil, target.0, target.1)
        .unwrap();
    let blocker = entities
        .spawn_building(1, EntityKind::Barracks, 288.0, 160.0, true)
        .unwrap();
    entities.get_mut(depot).unwrap().push_production(ProdItem {
        unit: EntityKind::PumpJack,
        progress: 0,
        total: 1,
        paid: false,
    });
    let mut players = vec![player(1)];
    players[0].set_resources(1000, 1000);
    let cost = rules::economy::resource_cost(EntityKind::PumpJack);
    manual::pay_front(&map, &mut entities, &mut players, depot);
    assert_eq!(players[0].steel, 1000);
    assert_eq!(players[0].oil, 1000);
    assert!(!entities.get(depot).unwrap().prod_queue()[0].paid);
    extractors::ensure_scaffold(&map, &mut entities, depot);
    assert!(extractors::scaffold_id(&entities, depot, EntityKind::PumpJack).is_none());
    entities.remove(blocker);
    manual::pay_front(&map, &mut entities, &mut players, depot);
    assert_eq!(players[0].steel, 1000 - cost.steel);
    assert_eq!(players[0].oil, 1000 - cost.oil);
    assert!(entities.get(depot).unwrap().prod_queue()[0].paid);
    assert!(extractors::ensure_scaffold(&map, &mut entities, depot));
    let scaffold = extractors::scaffold_id(&entities, depot, EntityKind::PumpJack).unwrap();
    assert_eq!(
        (
            entities.get(scaffold).unwrap().pos_x(),
            entities.get(scaffold).unwrap().pos_y()
        ),
        target
    );
}
