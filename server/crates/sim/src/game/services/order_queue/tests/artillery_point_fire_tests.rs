use super::*;

#[test]
fn queued_packed_artillery_point_fire_sets_up_on_promotion() {
    let map = flat_map(64);
    let mut entities = EntityStore::new();
    let pos = (320.0, 320.0);
    let target = (pos.0 + config::TILE_SIZE as f32 * 30.0, pos.1);
    let artillery = entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    entities
        .get_mut(artillery)
        .expect("artillery should exist")
        .append_queued_order(OrderIntent::point_fire(target.0, target.1));

    promote(&map, &mut entities);

    let unit = entities.get(artillery).expect("artillery should exist");
    assert!(matches!(unit.weapon_setup(), WeaponSetup::Packed));
    assert!(matches!(unit.order(), Order::ArtilleryPointFire(_)));
    assert!(
        unit.emplacement_facing().unwrap_or_default().abs() < 0.001,
        "queued packed point fire should set up toward the stored target"
    );
}
